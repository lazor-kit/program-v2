use assertions::sol_assert_bytes_eq;
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use crate::{
    auth::{
        ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator,
    },
    error::AuthError,
    state::{
        authority::AuthorityAccountHeader, config::ConfigAccount, session::SessionAccount,
        AccountDiscriminator,
    },
};

/// Closes a session account and refunds the rent to the caller.
///
/// Authentication rules:
/// - Contract Admin: Can close ONLY expired sessions.
/// - Wallet Admin/Owner: Can close both active AND expired sessions.
/// - Anyone else: Rejected.
///
/// Accounts:
/// 0. `[signer, writable]` Payer (receives refund)
/// 1. `[]` Wallet PDA
/// 2. `[writable]` Session PDA
/// 3. `[]` Config PDA
/// 4. `[optional]` Authorizer PDA (if wallet admin/owner)
/// 5. `[optional, signer]` Authorizer Signer (Ed25519)
/// 6. `[optional]` Sysvar Instructions (Secp256r1)
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Note: Protocol fee is not charged for cleanup actions.
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
    let session_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let config_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    let len = accounts.len();
    if len < 6 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let treasury_shard = &accounts[len - 2];
    let system_program = &accounts[len - 1];

    let config_data = unsafe { config_pda.borrow_data_unchecked() };
    if config_data.len() < std::mem::size_of::<ConfigAccount>() {
        return Err(ProgramError::UninitializedAccount);
    }
    let config = unsafe { std::ptr::read_unaligned(config_data.as_ptr() as *const ConfigAccount) };

    crate::utils::collect_protocol_fee(
        program_id,
        payer,
        &config,
        treasury_shard,
        system_program,
        false, // not a wallet creation
    )?;

    // 1. Validate Session PDA
    if session_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    let session_data = unsafe { session_pda.borrow_mut_data_unchecked() };
    if session_data.len() < std::mem::size_of::<SessionAccount>() {
        return Err(ProgramError::InvalidAccountData);
    }
    // Safe alignment check isn't strictly necessary with byte copy or if we assume layout
    let session =
        unsafe { std::ptr::read_unaligned(session_data.as_ptr() as *const SessionAccount) };

    if session.discriminator != AccountDiscriminator::Session as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if session.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidArgument);
    }
    // Re-derive to be absolutely sure
    let (derived_session_key, _bump) = find_program_address(
        &[
            b"session",
            wallet_pda.key().as_ref(),
            session.session_key.as_ref(),
        ],
        program_id,
    );
    if !sol_assert_bytes_eq(session_pda.key().as_ref(), derived_session_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }

    // 3. Check expiration
    let current_slot = Clock::get()?.slot;
    let is_expired = current_slot > session.expires_at;

    // 4. Authorization
    let mut is_authorized = false;

    // Is the caller the contract admin?
    if *payer.key() == config.admin {
        if is_expired {
            is_authorized = true;
        } else {
            // Admin cannot close active sessions
            return Err(AuthError::PermissionDenied.into());
        }
    }

    // Is there an authorizer PDA provided?
    if !is_authorized {
        let auth_pda = account_info_iter.next();
        if let Some(auth_pda) = auth_pda {
            // Verify authority belongs to the wallet
            if auth_pda.owner() != program_id {
                return Err(ProgramError::IllegalOwner);
            }
            let auth_data = unsafe { auth_pda.borrow_mut_data_unchecked() };
            if auth_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
                return Err(ProgramError::InvalidAccountData);
            }
            let auth_header = unsafe {
                std::ptr::read_unaligned(auth_data.as_ptr() as *const AuthorityAccountHeader)
            };
            if auth_header.discriminator != AccountDiscriminator::Authority as u8 {
                return Err(ProgramError::InvalidAccountData);
            }
            if auth_header.wallet != *wallet_pda.key() {
                return Err(ProgramError::InvalidArgument);
            }
            if auth_header.role > 1 {
                // Must be Owner (0) or Admin (1)
                return Err(AuthError::PermissionDenied.into());
            }

            // Authenticate the authority via signatures
            // Binding payload to the session PDA to prevent replay swap attacks
            let mut payload = Vec::with_capacity(32);
            payload.extend_from_slice(session_pda.key().as_ref());

            if auth_header.authority_type == 0 {
                // Ed25519
                Ed25519Authenticator.authenticate(accounts, auth_data, &[], &payload, &[8])?;
            } else if auth_header.authority_type == 1 {
                // Secp256r1
                Secp256r1Authenticator.authenticate(accounts, auth_data, &[], &payload, &[8])?;
            } else {
                return Err(AuthError::InvalidAuthenticationKind.into());
            }

            is_authorized = true;
        }
    }

    if !is_authorized {
        return Err(AuthError::PermissionDenied.into());
    }

    // 5. Transfer session lamports to the payer
    let session_lamports = session_pda.lamports();
    let payer_lamports = payer.lamports();

    unsafe {
        *payer.borrow_mut_lamports_unchecked() = payer_lamports
            .checked_add(session_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *session_pda.borrow_mut_lamports_unchecked() = 0;
    }

    // 6. Zero out the session data
    session_data.fill(0);

    Ok(())
}
