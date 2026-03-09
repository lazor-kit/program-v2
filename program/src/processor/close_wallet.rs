use assertions::sol_assert_bytes_eq;
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    ProgramResult,
};

use crate::{
    auth::{
        ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator,
    },
    error::AuthError,
    state::{authority::AuthorityAccountHeader, wallet::WalletAccount, AccountDiscriminator},
};

/// Closes the Wallet and Vault PDAs, sending all remaining lamports to a designated destination.
///
/// This is a highly destructive action and can ONLY be performed by the Owner (Role 0).
/// Note: Any remaining Authority/Session PDAs will be orphaned on-chain.
///
/// Accounts:
/// 0. `[signer]` Payer (pays transaction fee)
/// 1. `[writable]` Wallet PDA (to close)
/// 2. `[writable]` Vault PDA (to drain)
/// 3. `[]` Owner Authority PDA (must be role == 0)
/// 4. `[writable]` Destination account (receives all drained lamports)
/// 5. `[optional, signer]` Owner Signer (Ed25519)
/// 6. `[optional]` Sysvar Instructions (Secp256r1)
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Note: Protocol fee is usually taken at the start. Since this is a final cleanup action,
    // we may choose not to charge a fee, or the entrypoint logic has already charged it.
    // Assuming entrypoint handles protocol fee. Wait! We decided to NOT charge fees for close actions.
    if !instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let account_info_iter = &mut accounts.iter();
    let payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let vault_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let owner_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let destination = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // 1. Validate Wallet PDA
    let len = accounts.len();
    if len < 10 {
        // 5 fixed + up to 2 optional + config + treasury + sys prog
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let config_pda = &accounts[len - 3];
    let treasury_shard = &accounts[len - 2];
    let system_program = &accounts[len - 1];

    let config_data = unsafe { config_pda.borrow_data_unchecked() };
    if config_data.len() < std::mem::size_of::<crate::state::config::ConfigAccount>() {
        return Err(ProgramError::UninitializedAccount);
    }
    let config = unsafe {
        std::ptr::read_unaligned(config_data.as_ptr() as *const crate::state::config::ConfigAccount)
    };

    crate::utils::collect_protocol_fee(
        program_id,
        payer, // Using payer
        &config,
        treasury_shard,
        system_program,
        false, // not a wallet creation
    )?;

    if wallet_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let wallet_data = unsafe { wallet_pda.borrow_mut_data_unchecked() };
    if wallet_data.len() < std::mem::size_of::<WalletAccount>() {
        return Err(ProgramError::InvalidAccountData);
    }
    let wallet_info =
        unsafe { std::ptr::read_unaligned(wallet_data.as_ptr() as *const WalletAccount) };
    if wallet_info.discriminator != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    // 2. Validate Vault PDA
    let (derived_vault_key, _vault_bump) =
        find_program_address(&[b"vault", wallet_pda.key().as_ref()], program_id);
    if !sol_assert_bytes_eq(vault_pda.key().as_ref(), derived_vault_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    // 3. Validate Owner Authority PDA
    if owner_auth_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let auth_data = unsafe { owner_auth_pda.borrow_mut_data_unchecked() };
    if auth_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
        return Err(ProgramError::InvalidAccountData);
    }
    let auth_header =
        unsafe { std::ptr::read_unaligned(auth_data.as_ptr() as *const AuthorityAccountHeader) };
    if auth_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if auth_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidArgument);
    }
    if auth_header.role != 0 {
        return Err(AuthError::PermissionDenied.into()); // MUST be Owner
    }

    // 4. Authenticate the Owner
    // Bind payload to the Destination address to prevent attackers from swapping the destination
    let mut payload = Vec::with_capacity(32);
    payload.extend_from_slice(destination.key().as_ref());

    if auth_header.authority_type == 0 {
        // Ed25519
        Ed25519Authenticator.authenticate(accounts, auth_data, &[], &payload, &[9])?;
    } else if auth_header.authority_type == 1 {
        // Secp256r1
        Secp256r1Authenticator.authenticate(accounts, auth_data, &[], &payload, &[9])?;
    } else {
        return Err(AuthError::InvalidAuthenticationKind.into());
    }

    // 5. Drain Vault PDA to Destination
    let vault_lamports = vault_pda.lamports();
    let dest_lamports = destination.lamports();

    unsafe {
        *destination.borrow_mut_lamports_unchecked() = dest_lamports
            .checked_add(vault_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *vault_pda.borrow_mut_lamports_unchecked() = 0;
    }
    let vault_data = unsafe { vault_pda.borrow_mut_data_unchecked() };
    if !vault_data.is_empty() {
        vault_data.fill(0);
    }

    // 6. Drain Wallet PDA to Destination
    let wallet_lamports = wallet_pda.lamports();
    // Re-read dest lamports since we just updated it
    let current_dest_lamports = destination.lamports();

    unsafe {
        *destination.borrow_mut_lamports_unchecked() = current_dest_lamports
            .checked_add(wallet_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *wallet_pda.borrow_mut_lamports_unchecked() = 0;
    }
    wallet_data.fill(0);

    Ok(())
}
