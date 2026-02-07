use crate::{
    auth::{
        ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator,
    },
    compact::parse_compact_instructions,
    error::AuthError,
    state::{authority::AuthorityAccountHeader, AccountDiscriminator},
};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Account, AccountMeta, Instruction, Seed, Signer},
    program::invoke_signed_unchecked,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

/// Process the Execute instruction  
/// Processes the `Execute` instruction.
///
/// Executes a batch of condensed "Compact Instructions" on behalf of the wallet.
///
/// # Logic:
/// 1. **Authentication**: Verifies that the signer is a valid `Authority` or `Session` for this wallet.
/// 2. **Session Checks**: If authenticated via Session, enforces slot expiry.
/// 3. **Decompression**: Expands `CompactInstructions` (index-based references) into full Solana instructions.
/// 4. **Execution**: Invokes the Instructions via CPI, signing with the Vault PDA.
///
/// # Accounts:
/// 1. `[signer]` Payer.
/// 2. `[]` Wallet PDA.
/// 3. `[signer]` Authority or Session PDA.
/// 4. `[signer]` Vault PDA (Signer for CPI).
/// 5. `...` Inner accounts referenced by instructions.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Parse accounts
    let account_info_iter = &mut accounts.iter();
    let _payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let authority_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let vault_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // Remaining accounts are for inner instructions
    let inner_accounts_start = 4;
    let _inner_accounts = &accounts[inner_accounts_start..];

    // Verify ownership
    if wallet_pda.owner() != program_id || authority_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
    // Validate Wallet Discriminator (Issue #7)
    let wallet_data = unsafe { wallet_pda.borrow_data_unchecked() };
    if wallet_data.is_empty() || wallet_data[0] != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    if !authority_pda.is_writable() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Read authority header
    // Safe copy header
    let authority_data = unsafe { authority_pda.borrow_mut_data_unchecked() };
    let mut header_bytes = [0u8; std::mem::size_of::<AuthorityAccountHeader>()];
    header_bytes.copy_from_slice(&authority_data[..std::mem::size_of::<AuthorityAccountHeader>()]);
    let authority_header =
        unsafe { std::mem::transmute::<&[u8; 48], &AuthorityAccountHeader>(&header_bytes) };

    // Parse compact instructions
    let compact_instructions = parse_compact_instructions(instruction_data)?;

    // Serialize compact instructions to get their byte length
    let compact_bytes = crate::compact::serialize_compact_instructions(&compact_instructions);
    let compact_len = compact_bytes.len();

    // Authenticate based on discriminator
    match authority_header.discriminator {
        2 => {
            // Authority
            if authority_header.wallet != *wallet_pda.key() {
                return Err(ProgramError::InvalidAccountData);
            }
            match authority_header.authority_type {
                0 => {
                    // Ed25519: Verify signer (authority_payload ignored)
                    Ed25519Authenticator.authenticate(accounts, authority_data, &[], &[], &[4])?;
                },
                1 => {
                    // Secp256r1 (WebAuthn)
                    // Issue #11: Include accounts hash to prevent account reordering attacks
                    // signed_payload is compact_instructions bytes + accounts hash for Execute
                    let data_payload = &instruction_data[..compact_len];
                    let authority_payload = &instruction_data[compact_len..];

                    // Compute hash of all account pubkeys referenced by compact instructions
                    // This binds the signature to the exact accounts, preventing reordering
                    let accounts_hash = compute_accounts_hash(accounts, &compact_instructions)?;

                    // Extended payload: compact_instructions + accounts_hash
                    let mut extended_payload = Vec::with_capacity(compact_len + 32);
                    extended_payload.extend_from_slice(data_payload);
                    extended_payload.extend_from_slice(&accounts_hash);

                    Secp256r1Authenticator.authenticate(
                        accounts,
                        authority_data,
                        authority_payload,
                        &extended_payload,
                        &[4],
                    )?;
                },
                _ => return Err(AuthError::InvalidAuthenticationKind.into()),
            }
        },
        3 => {
            // Session
            let session_data = unsafe { authority_pda.borrow_mut_data_unchecked() };
            if session_data.len() < std::mem::size_of::<crate::state::session::SessionAccount>() {
                return Err(ProgramError::InvalidAccountData);
            }
            if (session_data.as_ptr() as usize) % 8 != 0 {
                return Err(ProgramError::InvalidAccountData);
            }
            // SAFETY: Alignment and size checked.
            let session = unsafe {
                &*(session_data.as_ptr() as *const crate::state::session::SessionAccount)
            };

            let clock = Clock::get()?;
            let current_slot = clock.slot;

            // Verify Wallet
            if session.wallet != *wallet_pda.key() {
                return Err(ProgramError::InvalidAccountData);
            }

            // Verify Expiry
            if current_slot > session.expires_at {
                return Err(AuthError::SessionExpired.into());
            }

            // Verify Signer matches Session Key
            let mut signer_matched = false;
            for acc in accounts {
                if acc.is_signer() && *acc.key() == session.session_key {
                    signer_matched = true;
                    break;
                }
            }
            if !signer_matched {
                return Err(ProgramError::MissingRequiredSignature);
            }
        },
        _ => return Err(ProgramError::InvalidAccountData),
    }

    // Get vault bump for signing
    let (vault_key, vault_bump) =
        find_program_address(&[b"vault", wallet_pda.key().as_ref()], program_id);

    // Verify vault PDA.
    // CRITICAL: Ensure we are signing with the correct Vault derived from this Wallet.
    if vault_pda.key() != &vault_key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Execute each compact instruction
    for compact_ix in &compact_instructions {
        let decompressed = compact_ix.decompress(accounts)?;

        // Build AccountMeta array for instruction
        let account_metas: Vec<AccountMeta> = decompressed
            .accounts
            .iter()
            .map(|acc| AccountMeta {
                pubkey: acc.key(),
                is_signer: acc.is_signer() || acc.key() == vault_pda.key(),
                is_writable: acc.is_writable(),
            })
            .collect();

        // Prevent self-reentrancy (Issue #10)
        // Reject CPI calls back into this program to avoid unexpected state mutations
        if decompressed.program_id.as_ref() == program_id.as_ref() {
            return Err(AuthError::SelfReentrancyNotAllowed.into());
        }

        // Create instruction
        let ix = Instruction {
            program_id: decompressed.program_id,
            accounts: &account_metas,
            data: &decompressed.data,
        };

        // Create seeds for vault signing (pinocchio style)
        let vault_bump_arr = [vault_bump];
        let seeds = [
            Seed::from(b"vault"),
            Seed::from(wallet_pda.key().as_ref()),
            Seed::from(&vault_bump_arr),
        ];
        let signer: Signer = (&seeds).into();

        // Convert AccountInfo to Account for invoke_signed_unchecked
        let cpi_accounts: Vec<Account> = decompressed
            .accounts
            .iter()
            .map(|acc| Account::from(*acc))
            .collect();

        // Invoke with vault as signer
        // Use unchecked invocation to support dynamic account list (slice)
        unsafe {
            invoke_signed_unchecked(&ix, &cpi_accounts, &[signer]);
        }
    }

    Ok(())
}

/// Compute SHA256 hash of all account pubkeys referenced by compact instructions (Issue #11)
///
/// This binds the signature to the exact accounts in their exact order,
/// preventing account reordering attacks where an attacker could swap
/// recipient addresses while keeping the signature valid.
///
/// # Arguments
/// * `accounts` - Slice of all account infos in the transaction
/// * `compact_instructions` - Parsed compact instructions containing account indices
///
/// # Returns
/// * 32-byte SHA256 hash of all referenced pubkeys
fn compute_accounts_hash(
    accounts: &[AccountInfo],
    compact_instructions: &[crate::compact::CompactInstruction],
) -> Result<[u8; 32], ProgramError> {
    // Collect all account pubkeys in order of reference
    let mut pubkeys_data = Vec::new();

    for ix in compact_instructions {
        // Include program_id
        let program_idx = ix.program_id_index as usize;
        if program_idx >= accounts.len() {
            return Err(ProgramError::InvalidInstructionData);
        }
        pubkeys_data.extend_from_slice(accounts[program_idx].key().as_ref());

        // Include all account pubkeys
        for &acc_idx in &ix.accounts {
            let idx = acc_idx as usize;
            if idx >= accounts.len() {
                return Err(ProgramError::InvalidInstructionData);
            }
            pubkeys_data.extend_from_slice(accounts[idx].key().as_ref());
        }
    }

    // Compute SHA256 hash
    #[allow(unused_assignments)]
    let mut hash = [0u8; 32];
    #[cfg(target_os = "solana")]
    unsafe {
        pinocchio::syscalls::sol_sha256(
            [pubkeys_data.as_slice()].as_ptr() as *const u8,
            1,
            hash.as_mut_ptr(),
        );
    }
    #[cfg(not(target_os = "solana"))]
    {
        // For tests, use a dummy hash
        hash = [0xAA; 32];
        let _ = pubkeys_data; // suppress warning
    }

    Ok(hash)
}
