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
    if !instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    pinocchio::msg!("CW: Enter");

    let account_info_iter = &mut accounts.iter();
    pinocchio::msg!("CW: Acc LEN: {}", accounts.len());
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

    pinocchio::msg!("CW: Payer: signer: {:?}", payer.is_signer());

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    if destination.key() == vault_pda.key() || destination.key() == wallet_pda.key() {
        pinocchio::msg!("CW: Invalid Dest");
        return Err(ProgramError::InvalidArgument);
    }

    pinocchio::msg!("CW: Auth Type Check");

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

    pinocchio::msg!(
        "CW: Auth check. authority_type: {}",
        auth_header.authority_type
    );

    if auth_header.authority_type == 0 {
        // Ed25519
        pinocchio::msg!("CW: Calling Ed255");
        Ed25519Authenticator.authenticate(accounts, auth_data, &[], &payload, &[9])?;
    } else if auth_header.authority_type == 1 {
        // Secp256r1
        pinocchio::msg!("CW: Calling Secp");
        Secp256r1Authenticator.authenticate(accounts, auth_data, &[], &payload, &[9])?;
    } else {
        return Err(AuthError::InvalidAuthenticationKind.into());
    }

    pinocchio::msg!("CW: Drain Vault");
    // 5. Drain Vault PDA to Destination
    let vault_lamports = vault_pda.lamports();
    if vault_lamports > 0 {
        // Must use CPI to transfer because the Vault is owned by SystemProgram
        let mut system_program = None;
        for acc in accounts {
            if acc.key().as_ref() == &[0; 32] {
                system_program = Some(acc);
                break;
            }
        }
        let sys_prog = system_program.ok_or(ProgramError::NotEnoughAccountKeys)?;

        // Create instruction
        let mut ix_data = [0u8; 12];
        ix_data[0..4].copy_from_slice(&2u32.to_le_bytes()); // Transfer instruction index
        ix_data[4..12].copy_from_slice(&vault_lamports.to_le_bytes());

        let account_metas = [
            pinocchio::instruction::AccountMeta {
                pubkey: vault_pda.key(),
                is_signer: true,
                is_writable: true,
            },
            pinocchio::instruction::AccountMeta {
                pubkey: destination.key(),
                is_signer: false,
                is_writable: true,
            },
        ];

        let ix = pinocchio::instruction::Instruction {
            program_id: sys_prog.key(),
            accounts: &account_metas,
            data: &ix_data,
        };

        let vault_bump_pda =
            find_program_address(&[b"vault", wallet_pda.key().as_ref()], program_id).1;
        let vault_bump_arr = [vault_bump_pda];
        let seeds = [
            pinocchio::instruction::Seed::from(b"vault"),
            pinocchio::instruction::Seed::from(wallet_pda.key().as_ref()),
            pinocchio::instruction::Seed::from(&vault_bump_arr),
        ];
        let signer: pinocchio::instruction::Signer = (&seeds).into();

        let cpi_accounts = [
            pinocchio::instruction::Account::from(vault_pda),
            pinocchio::instruction::Account::from(destination),
        ];

        unsafe {
            pinocchio::program::invoke_signed_unchecked(&ix, &cpi_accounts, &[signer]);
        }
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

    pinocchio::msg!("CW: Done");
    Ok(())
}
