use crate::{
    auth::{ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator},
    error::AuthError,
    state::{
        authority::AuthorityAccountHeader,
        session::SessionAccount,
        AccountDiscriminator,
    },
};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};

/// Process the RevokeSession instruction.
///
/// Closes a session account early (before expiry), refunding rent to a specified destination.
/// Only Owner or Admin can revoke.
///
/// # Accounts:
/// 1. `[signer]` Payer
/// 2. `[]` Wallet PDA
/// 3. `[writable]` Admin/Owner Authority PDA (counter incremented for Secp256r1)
/// 4. `[writable]` Session PDA (closed)
/// 5. `[writable]` Refund destination
/// 6. `[optional]` Auth extra (Ed25519: signer keypair | Secp256r1: sysvar_instructions)
///
/// # Instruction Data (after discriminator):
///   Secp256r1: [auth_payload(variable)]
///   Ed25519:   empty (auth is via signer)
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let authority_payload = instruction_data;

    let payer = accounts
        .first()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = accounts
        .get(1)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let admin_auth_pda = accounts
        .get(2)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let session_pda = accounts
        .get(3)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let refund_dest = accounts
        .get(4)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // Validate payer is signer
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify ownership of program accounts
    if wallet_pda.owner() != program_id
        || admin_auth_pda.owner() != program_id
        || session_pda.owner() != program_id
    {
        return Err(ProgramError::IllegalOwner);
    }

    // Validate Wallet discriminator
    let wallet_data = unsafe { wallet_pda.borrow_data_unchecked() };
    if wallet_data.is_empty() || wallet_data[0] != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    // Authority PDA must be writable (counter increment for Secp256r1)
    if !admin_auth_pda.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Read authority header
    let admin_data = unsafe { admin_auth_pda.borrow_mut_data_unchecked() };
    if admin_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
        return Err(ProgramError::InvalidAccountData);
    }
    let admin_header = unsafe {
        std::ptr::read_unaligned(admin_data.as_ptr() as *const AuthorityAccountHeader)
    };

    if admin_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if admin_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Only Owner (0) or Admin (1) can revoke sessions
    if admin_header.role > 1 {
        return Err(AuthError::PermissionDenied.into());
    }

    // Build data_payload binding signature to specific session + refund destination
    let mut data_payload = Vec::with_capacity(64);
    data_payload.extend_from_slice(session_pda.key().as_ref());
    data_payload.extend_from_slice(refund_dest.key().as_ref());

    // Authenticate
    match admin_header.authority_type {
        0 => {
            Ed25519Authenticator.authenticate(
                accounts, admin_data, &[], &data_payload, &[9], program_id,
            )?;
        }
        1 => {
            Secp256r1Authenticator.authenticate(
                accounts, admin_data, authority_payload, &data_payload, &[9], program_id,
            )?;
        }
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    }

    // Validate session account
    let session_data = unsafe { session_pda.borrow_mut_data_unchecked() };
    if session_data.len() < std::mem::size_of::<SessionAccount>() {
        return Err(ProgramError::InvalidAccountData);
    }
    let session = unsafe {
        std::ptr::read_unaligned(session_data.as_ptr() as *const SessionAccount)
    };

    if session.discriminator != AccountDiscriminator::Session as u8 {
        return Err(AuthError::InvalidSessionAccount.into());
    }

    // Session must belong to this wallet
    if session.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Close the session account — zero data and drain lamports
    session_data.fill(0);

    let session_lamports = session_pda.lamports();
    let refund_lamports = unsafe { *refund_dest.borrow_mut_lamports_unchecked() };
    unsafe {
        *refund_dest.borrow_mut_lamports_unchecked() = refund_lamports
            .checked_add(session_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *session_pda.borrow_mut_lamports_unchecked() = 0;
    }

    Ok(())
}
