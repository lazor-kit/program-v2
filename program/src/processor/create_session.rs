use assertions::{check_zero_data, sol_assert_bytes_eq};
use no_padding::NoPadding;
use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

use crate::{
    auth::{
        ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator,
    },
    error::AuthError,
    state::{
        action::validate_actions_buffer,
        authority::AuthorityAccountHeader,
        session::{SessionAccount, SESSION_HEADER_SIZE},
        AccountDiscriminator,
    },
};

/// Arguments for the `CreateSession` instruction.
///
/// Layout:
/// - `session_key`: The public key of the ephemeral session signer (32 bytes).
/// - `expires_at`: The absolute slot height when this session expires (8 bytes).
/// - `actions_len`: Length of the actions buffer in bytes (2 bytes, u16 LE). 0 = no actions.
/// - `actions`: Raw actions buffer (variable, `actions_len` bytes).
///
/// Total fixed: 42 bytes minimum. Backwards compatible: old clients sending 40 bytes
/// will have actions_len=0 (no actions).
#[repr(C, align(8))]
#[derive(NoPadding)]
pub struct CreateSessionArgs {
    pub session_key: [u8; 32],
    pub expires_at: u64,
}

/// Parsed session creation arguments including optional actions.
pub struct ParsedCreateSessionArgs {
    pub session_key: [u8; 32],
    pub expires_at: u64,
    /// Raw actions buffer bytes (empty if no actions).
    pub actions_bytes: Vec<u8>,
    /// Byte offset where the actions section ends in instruction_data.
    /// Everything after this is auth_payload for Secp256r1.
    pub args_end_offset: usize,
}

impl ParsedCreateSessionArgs {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < 40 {
            return Err(ProgramError::InvalidInstructionData);
        }

        let mut session_key = [0u8; 32];
        session_key.copy_from_slice(&data[..32]);
        let expires_at = u64::from_le_bytes(data[32..40].try_into().unwrap());

        // Check for actions buffer
        if data.len() >= 42 {
            let actions_len = u16::from_le_bytes(data[40..42].try_into().unwrap()) as usize;

            // Cap actions buffer size to prevent BPF heap exhaustion.
            // 16 actions * max ~128 bytes each = 2048 is generous.
            // The BPF heap is 32KB; allocating 64KB (u16 max) would OOM.
            const MAX_ACTIONS_BUFFER_SIZE: usize = 2048;
            if actions_len > MAX_ACTIONS_BUFFER_SIZE {
                return Err(ProgramError::InvalidInstructionData);
            }

            if actions_len > 0 {
                let actions_start = 42;
                let actions_end = actions_start + actions_len;

                if data.len() < actions_end {
                    return Err(ProgramError::InvalidInstructionData);
                }

                let actions_bytes = data[actions_start..actions_end].to_vec();

                // Validate actions buffer at creation time
                validate_actions_buffer(&actions_bytes)?;

                return Ok(Self {
                    session_key,
                    expires_at,
                    actions_bytes,
                    args_end_offset: actions_end,
                });
            }

            // actions_len == 0
            return Ok(Self {
                session_key,
                expires_at,
                actions_bytes: Vec::new(),
                args_end_offset: 42,
            });
        }

        // Legacy format: exactly 40 bytes, no actions
        Ok(Self {
            session_key,
            expires_at,
            actions_bytes: Vec::new(),
            args_end_offset: 40,
        })
    }
}

/// Processes the `CreateSession` instruction.
///
/// Creates a temporary `Session` account that facilitates limited-scope execution (Spender role).
/// Optional actions (permissions) can be attached to restrict what the session can do.
///
/// # Logic:
/// 1. Verifies the authorizing authority (must be Owner or Admin).
/// 2. Validates optional actions buffer.
/// 3. Derives a fresh Session PDA from `["session", wallet, session_key]`.
/// 4. Allocates and initializes the Session account with expiry and actions.
///
/// # Accounts:
/// 1. `[signer, writable]` Payer: Pays for rent.
/// 2. `[]` Wallet PDA.
/// 3. `[signer, writable]` Authorizer: Authority approving this session creation.
/// 4. `[writable]` Session PDA: The new session account.
/// 5. `[]` System Program.
/// 6. `[]` Rent Sysvar.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let args = ParsedCreateSessionArgs::from_bytes(instruction_data)?;

    let account_info_iter = &mut accounts.iter();
    let payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let authorizer_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let session_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let system_program = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let rent_sysvar = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // Get rent from sysvar (fixes audit issue #5 - hardcoded rent calculations)
    let rent = Rent::from_account_info(rent_sysvar)?;

    // Validate system_program is the correct System Program (audit N2)
    if !assertions::sol_assert_bytes_eq(
        system_program.key().as_ref(),
        &crate::utils::SYSTEM_PROGRAM_ID,
        32,
    ) {
        return Err(ProgramError::IncorrectProgramId);
    }

    if wallet_pda.owner() != program_id || authorizer_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Validate Wallet Discriminator (Issue #7)
    let wallet_data = unsafe { wallet_pda.borrow_data_unchecked() };
    if wallet_data.is_empty() || wallet_data[0] != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    let auth_data = unsafe { authorizer_pda.borrow_mut_data_unchecked() };

    // Safe Copy of Header using read_unaligned
    if auth_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
        return Err(ProgramError::InvalidAccountData);
    }
    let auth_header =
        unsafe { std::ptr::read_unaligned(auth_data.as_ptr() as *const AuthorityAccountHeader) };

    if auth_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if auth_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }
    // Only Admin (1) or Owner (0) can create sessions.
    // Spender (2) cannot create sessions.
    if auth_header.role != 0 && auth_header.role != 1 {
        return Err(AuthError::PermissionDenied.into());
    }

    // Validate expires_at: must be in the future and within max session duration
    {
        let clock = Clock::get()?;
        let current_slot = clock.slot;
        if args.expires_at <= current_slot {
            return Err(AuthError::InvalidSessionDuration.into());
        }
        // Max session duration: ~30 days at ~2.5 slots/sec = 6,480,000 slots
        const MAX_SESSION_SLOTS: u64 = 6_480_000;
        if args.expires_at > current_slot.saturating_add(MAX_SESSION_SLOTS) {
            return Err(AuthError::InvalidSessionDuration.into());
        }
    }

    // Authenticate Authorizer
    // instruction_data layout: [args(40)][actions_len(2)][actions(N)][auth_payload...]
    // args.args_end_offset points to the end of the args+actions section.
    let data_payload = &instruction_data[..args.args_end_offset];
    let authority_payload = if instruction_data.len() > args.args_end_offset {
        &instruction_data[args.args_end_offset..]
    } else {
        &[]
    };

    // Ed25519 signed payload — includes payer + session_key + actions.
    // Note: Ed25519Authenticator only checks that the authority keypair is a tx signer,
    // so this payload is not cryptographically verified. The protection is that only the
    // keypair holder can sign the transaction. For Secp256r1, the data_payload IS verified.
    let mut ed25519_payload = Vec::with_capacity(64 + args.actions_bytes.len());
    ed25519_payload.extend_from_slice(payer.key().as_ref());
    ed25519_payload.extend_from_slice(&args.session_key);
    ed25519_payload.extend_from_slice(&args.actions_bytes);

    match auth_header.authority_type {
        0 => {
            // Ed25519: Include payer + session_key + actions in signed payload
            Ed25519Authenticator.authenticate(
                accounts,
                auth_data,
                &[],
                &ed25519_payload,
                &[5],
                program_id,
            )?;
        }
        1 => {
            // Secp256r1: Include payer in data_payload
            let mut extended_data_payload = Vec::with_capacity(data_payload.len() + 32);
            extended_data_payload.extend_from_slice(data_payload);
            extended_data_payload.extend_from_slice(payer.key().as_ref());

            Secp256r1Authenticator.authenticate(
                accounts,
                auth_data,
                authority_payload,
                &extended_data_payload,
                &[5],
                program_id,
            )?;
        }
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    }

    // Derive Session PDA
    let (session_key, bump) = find_program_address(
        &[b"session", wallet_pda.key().as_ref(), &args.session_key],
        program_id,
    );
    if !sol_assert_bytes_eq(session_pda.key().as_ref(), session_key.as_ref(), 32) {
        return Err(ProgramError::InvalidSeeds);
    }
    check_zero_data(session_pda, ProgramError::AccountAlreadyInitialized)?;

    // Create Session Account — variable size if actions are present
    let space = SESSION_HEADER_SIZE + args.actions_bytes.len();
    let session_rent = rent.minimum_balance(space);

    let bump_arr = [bump];
    let seeds = [
        Seed::from(b"session"),
        Seed::from(wallet_pda.key().as_ref()),
        Seed::from(&args.session_key),
        Seed::from(&bump_arr),
    ];

    crate::utils::initialize_pda_account(
        payer,
        session_pda,
        system_program,
        space,
        session_rent,
        program_id,
        &seeds,
    )?;

    // Initialize Session State
    let data = unsafe { session_pda.borrow_mut_data_unchecked() };
    let session = SessionAccount {
        discriminator: AccountDiscriminator::Session as u8,
        bump,
        version: crate::state::CURRENT_ACCOUNT_VERSION,
        _padding: [0; 5],
        wallet: *wallet_pda.key(),
        session_key: Pubkey::from(args.session_key),
        expires_at: args.expires_at,
    };

    // Write fixed header
    let session_bytes = unsafe {
        std::slice::from_raw_parts(
            &session as *const SessionAccount as *const u8,
            std::mem::size_of::<SessionAccount>(),
        )
    };
    data[..SESSION_HEADER_SIZE].copy_from_slice(session_bytes);

    // Write actions buffer (if any)
    if !args.actions_bytes.is_empty() {
        data[SESSION_HEADER_SIZE..SESSION_HEADER_SIZE + args.actions_bytes.len()]
            .copy_from_slice(&args.actions_bytes);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_session_args_from_bytes() {
        let mut data = Vec::new();
        let session_key = [7u8; 32];
        let expires_at = 12345678u64;
        data.extend_from_slice(&session_key);
        data.extend_from_slice(&expires_at.to_le_bytes());

        let args = ParsedCreateSessionArgs::from_bytes(&data).unwrap();
        assert_eq!(args.session_key, session_key);
        assert_eq!(args.expires_at, expires_at);
        assert!(args.actions_bytes.is_empty());
        assert_eq!(args.args_end_offset, 40);
    }

    #[test]
    fn test_create_session_args_too_short() {
        let data = vec![0u8; 39];
        assert!(ParsedCreateSessionArgs::from_bytes(&data).is_err());
    }

    #[test]
    fn test_create_session_args_with_no_actions() {
        let mut data = Vec::new();
        data.extend_from_slice(&[7u8; 32]); // session_key
        data.extend_from_slice(&12345678u64.to_le_bytes()); // expires_at
        data.extend_from_slice(&0u16.to_le_bytes()); // actions_len = 0

        let args = ParsedCreateSessionArgs::from_bytes(&data).unwrap();
        assert!(args.actions_bytes.is_empty());
        assert_eq!(args.args_end_offset, 42);
    }

    #[test]
    fn test_create_session_args_with_actions() {
        let mut data = Vec::new();
        data.extend_from_slice(&[7u8; 32]); // session_key
        data.extend_from_slice(&12345678u64.to_le_bytes()); // expires_at

        // Build a SolMaxPerTx action: header(11) + data(8) = 19 bytes
        let mut actions = Vec::new();
        actions.push(3u8); // type = SolMaxPerTx
        actions.extend_from_slice(&8u16.to_le_bytes()); // data_len
        actions.extend_from_slice(&0u64.to_le_bytes()); // expires_at
        actions.extend_from_slice(&500_000u64.to_le_bytes()); // max

        data.extend_from_slice(&(actions.len() as u16).to_le_bytes()); // actions_len
        data.extend_from_slice(&actions);

        let args = ParsedCreateSessionArgs::from_bytes(&data).unwrap();
        assert_eq!(args.actions_bytes.len(), 19);
        assert_eq!(args.args_end_offset, 42 + 19);
    }

    #[test]
    fn test_create_session_args_with_invalid_actions() {
        let mut data = Vec::new();
        data.extend_from_slice(&[7u8; 32]); // session_key
        data.extend_from_slice(&12345678u64.to_le_bytes()); // expires_at

        // Invalid action type
        let mut actions = Vec::new();
        actions.push(99u8); // bad type
        actions.extend_from_slice(&8u16.to_le_bytes());
        actions.extend_from_slice(&0u64.to_le_bytes());
        actions.extend_from_slice(&500_000u64.to_le_bytes());

        data.extend_from_slice(&(actions.len() as u16).to_le_bytes());
        data.extend_from_slice(&actions);

        assert!(ParsedCreateSessionArgs::from_bytes(&data).is_err());
    }

    #[test]
    fn test_create_session_args_with_trailing_auth_payload() {
        let mut data = Vec::new();
        data.extend_from_slice(&[7u8; 32]); // session_key
        data.extend_from_slice(&12345678u64.to_le_bytes()); // expires_at

        // SolMaxPerTx action
        let mut actions = Vec::new();
        actions.push(3u8);
        actions.extend_from_slice(&8u16.to_le_bytes());
        actions.extend_from_slice(&0u64.to_le_bytes());
        actions.extend_from_slice(&500_000u64.to_le_bytes());

        data.extend_from_slice(&(actions.len() as u16).to_le_bytes());
        data.extend_from_slice(&actions);

        // Simulate trailing auth payload
        data.extend_from_slice(&[0xAA; 50]);

        let args = ParsedCreateSessionArgs::from_bytes(&data).unwrap();
        assert_eq!(args.actions_bytes.len(), 19);
        assert_eq!(args.args_end_offset, 42 + 19);
        // Trailing 50 bytes would be auth_payload — not parsed here
    }

    #[test]
    fn test_actions_len_exceeds_cap_rejected() {
        let mut data = Vec::new();
        data.extend_from_slice(&[7u8; 32]); // session_key
        data.extend_from_slice(&12345678u64.to_le_bytes()); // expires_at

        // actions_len = 3000 > MAX_ACTIONS_BUFFER_SIZE (2048)
        data.extend_from_slice(&3000u16.to_le_bytes());
        // Pad enough bytes so the length check doesn't fail first
        data.extend_from_slice(&vec![0u8; 3000]);

        let result = ParsedCreateSessionArgs::from_bytes(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_actions_len_at_cap_allowed() {
        let mut data = Vec::new();
        data.extend_from_slice(&[7u8; 32]); // session_key
        data.extend_from_slice(&12345678u64.to_le_bytes()); // expires_at

        // Build a valid action buffer that's under 2048
        // 16 ProgramWhitelist actions = 16 * (11 header + 32 data) = 16 * 43 = 688 bytes
        let mut actions = Vec::new();
        for i in 0..16u8 {
            let mut prog = [0u8; 32];
            prog[0] = i;
            actions.push(10u8); // ProgramWhitelist type
            actions.extend_from_slice(&32u16.to_le_bytes()); // data_len
            actions.extend_from_slice(&0u64.to_le_bytes()); // expires_at
            actions.extend_from_slice(&prog);
        }
        assert!(actions.len() <= 2048);

        data.extend_from_slice(&(actions.len() as u16).to_le_bytes());
        data.extend_from_slice(&actions);

        let result = ParsedCreateSessionArgs::from_bytes(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().actions_bytes.len(), actions.len());
    }
}
