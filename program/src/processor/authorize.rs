use crate::{
    auth::{secp256r1::Secp256r1Authenticator, traits::Authenticator},
    error::AuthError,
    state::{
        authority::AuthorityAccountHeader,
        deferred::DeferredExecAccount,
        AccountDiscriminator, CURRENT_ACCOUNT_VERSION,
    },
    utils::initialize_pda_account,
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

/// Minimum expiry window in slots (~4 seconds at 400ms/slot)
const MIN_EXPIRY_SLOTS: u16 = 10;

/// Maximum expiry window in slots (~1 hour at 400ms/slot)
const MAX_EXPIRY_SLOTS: u16 = 9_000;

/// Process the Authorize instruction (deferred execution tx1).
///
/// Verifies Secp256r1 signature over instruction/account hashes, then creates
/// a DeferredExec PDA storing the authorization for later execution.
///
/// # Accounts:
/// 1. `[signer, writable]` Payer
/// 2. `[]` Wallet PDA
/// 3. `[writable]` Authority PDA (counter increment)
/// 4. `[writable]` DeferredExec PDA (created)
/// 5. `[]` System Program
/// 6. `[]` Rent Sysvar
/// 7. `[]` Sysvar Instructions (for Secp256r1 precompile introspection)
///
/// # Instruction Data (after discriminator):
///   [instructions_hash(32)][accounts_hash(32)][expiry_offset(2)][auth_payload(variable)]
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Parse accounts
    let payer = accounts
        .first()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = accounts
        .get(1)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let authority_pda = accounts
        .get(2)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let deferred_pda = accounts
        .get(3)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = accounts
        .get(4)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let _rent_sysvar = accounts
        .get(5)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // Validate payer is signer
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify ownership
    if wallet_pda.owner() != program_id || authority_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Validate Wallet discriminator
    let wallet_data = unsafe { wallet_pda.borrow_data_unchecked() };
    if wallet_data.is_empty() || wallet_data[0] != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    if !authority_pda.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Parse instruction data: [instructions_hash(32)][accounts_hash(32)][expiry_offset(2)][auth_payload(...)]
    if instruction_data.len() < 66 {
        // 32 + 32 + 2 minimum
        return Err(ProgramError::InvalidInstructionData);
    }

    let instructions_hash: [u8; 32] = instruction_data[0..32].try_into().unwrap();
    let accounts_hash: [u8; 32] = instruction_data[32..64].try_into().unwrap();
    let expiry_offset =
        u16::from_le_bytes(instruction_data[64..66].try_into().unwrap());
    let auth_payload = &instruction_data[66..];

    // Validate expiry window
    if expiry_offset < MIN_EXPIRY_SLOTS || expiry_offset > MAX_EXPIRY_SLOTS {
        return Err(AuthError::InvalidExpiryWindow.into());
    }

    // Read authority header
    let authority_data = unsafe { authority_pda.borrow_mut_data_unchecked() };
    if authority_data.is_empty() || authority_data[0] != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if authority_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
        return Err(ProgramError::InvalidAccountData);
    }

    let authority_header = unsafe {
        std::ptr::read_unaligned(authority_data.as_ptr() as *const AuthorityAccountHeader)
    };

    if authority_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Only Secp256r1 needs deferred execution
    if authority_header.authority_type != 1 {
        return Err(AuthError::InvalidAuthenticationKind.into());
    }

    // Only Owner or Admin can authorize (not Spender)
    if authority_header.role > 1 {
        return Err(AuthError::PermissionDenied.into());
    }

    // The signed_payload for Authorize is: instructions_hash || accounts_hash || expiry_offset
    let mut signed_payload = Vec::with_capacity(66);
    signed_payload.extend_from_slice(&instructions_hash);
    signed_payload.extend_from_slice(&accounts_hash);
    signed_payload.extend_from_slice(&expiry_offset.to_le_bytes());

    // Authenticate — this verifies the Secp256r1 signature and increments the counter
    Secp256r1Authenticator.authenticate(
        accounts,
        authority_data,
        auth_payload,
        &signed_payload,
        &[6], // discriminator for Authorize
        program_id,
    )?;

    // Read the counter value that was just committed
    let updated_header = unsafe {
        std::ptr::read_unaligned(authority_data.as_ptr() as *const AuthorityAccountHeader)
    };
    let counter_for_seed = updated_header.counter;

    // Compute expiry
    let clock = Clock::get()?;
    let expires_at = clock.slot + expiry_offset as u64;

    // Derive DeferredExec PDA
    let counter_bytes = counter_for_seed.to_le_bytes();
    let seeds: &[&[u8]] = &[
        b"deferred",
        wallet_pda.key().as_ref(),
        authority_pda.key().as_ref(),
        &counter_bytes,
    ];
    let (expected_pda, bump) = find_program_address(seeds, program_id);

    if deferred_pda.key() != &expected_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // Compute rent
    let rent = Rent::get()?;
    let space = std::mem::size_of::<DeferredExecAccount>();
    let rent_lamports = rent.minimum_balance(space);

    // Create DeferredExec PDA
    let bump_arr = [bump];
    let pda_seeds: &[Seed] = &[
        Seed::from(b"deferred"),
        Seed::from(wallet_pda.key().as_ref()),
        Seed::from(authority_pda.key().as_ref()),
        Seed::from(&counter_bytes),
        Seed::from(&bump_arr),
    ];

    initialize_pda_account(
        payer,
        deferred_pda,
        system_program,
        space,
        rent_lamports,
        program_id,
        pda_seeds,
    )?;

    // Write DeferredExec data
    let deferred = DeferredExecAccount {
        discriminator: AccountDiscriminator::DeferredExec as u8,
        version: CURRENT_ACCOUNT_VERSION,
        bump,
        _padding: [0u8; 5],
        instructions_hash,
        accounts_hash,
        wallet: *wallet_pda.key(),
        authority: *authority_pda.key(),
        payer: *payer.key(),
        expires_at,
    };

    let deferred_data = unsafe { deferred_pda.borrow_mut_data_unchecked() };
    unsafe {
        std::ptr::write_unaligned(
            deferred_data.as_mut_ptr() as *mut DeferredExecAccount,
            deferred,
        );
    }

    Ok(())
}
