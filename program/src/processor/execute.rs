use crate::{
    auth::{
        ed25519::Ed25519Authenticator, secp256r1::Secp256r1Authenticator, traits::Authenticator,
    },
    compact::{parse_compact_instructions_ref_with_len, CompactInstructionRef},
    error::AuthError,
    processor::execute_actions::{
        evaluate_post_actions, evaluate_pre_actions, snapshot_token_authorities,
        snapshot_token_balances, verify_token_authorities_unchanged,
    },
    state::{authority::AuthorityAccountHeader, session::has_actions, AccountDiscriminator},
    utils::get_stack_height,
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

/// Process the Execute instruction.
///
/// Executes a batch of condensed "Compact Instructions" on behalf of the wallet.
///
/// # Logic:
/// 1. **Authentication**: Verifies that the signer is a valid `Authority` or `Session` for this wallet.
/// 2. **Session Checks**: If authenticated via Session, enforces slot expiry and action permissions.
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
    let authority_data = unsafe { authority_pda.borrow_mut_data_unchecked() };

    // Authenticate based on discriminator
    let discriminator = if !authority_data.is_empty() {
        authority_data[0]
    } else {
        return Err(ProgramError::InvalidAccountData);
    };

    // Parse compact instructions and get their consumed byte length. The
    // length is used to split `instruction_data` into the compact-instructions
    // prefix (the data_payload bound into the Secp256r1 signature) and the
    // auth payload suffix. Tracking the parse cursor avoids re-serializing
    // just to measure length.
    let (compact_instructions, compact_len) =
        parse_compact_instructions_ref_with_len(instruction_data)?;

    // Track whether this is a session-based execution and the current slot
    let mut is_session = false;
    let mut session_slot: u64 = 0;

    match discriminator {
        2 => {
            // Authority
            if authority_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
                return Err(ProgramError::InvalidAccountData);
            }
            let authority_header = unsafe {
                std::ptr::read_unaligned(authority_data.as_ptr() as *const AuthorityAccountHeader)
            };

            if authority_header.discriminator != AccountDiscriminator::Authority as u8 {
                return Err(ProgramError::InvalidAccountData);
            }

            if authority_header.wallet != *wallet_pda.key() {
                return Err(ProgramError::InvalidAccountData);
            }
            match authority_header.authority_type {
                0 => {
                    // Ed25519
                    Ed25519Authenticator.authenticate(
                        accounts,
                        authority_data,
                        &[],
                        &[],
                        &[4],
                        program_id,
                    )?;
                }
                1 => {
                    // Secp256r1 (WebAuthn)
                    let data_payload = &instruction_data[..compact_len];
                    let authority_payload = &instruction_data[compact_len..];
                    let accounts_hash =
                        compute_accounts_hash(accounts, &compact_instructions)?;
                    let mut extended_payload = Vec::with_capacity(compact_len + 32);
                    extended_payload.extend_from_slice(data_payload);
                    extended_payload.extend_from_slice(&accounts_hash);

                    Secp256r1Authenticator.authenticate(
                        accounts,
                        authority_data,
                        authority_payload,
                        &extended_payload,
                        &[4],
                        program_id,
                    )?;
                }
                _ => return Err(AuthError::InvalidAuthenticationKind.into()),
            }
        }
        3 => {
            // Session — reuse the existing `authority_data` borrow; no re-borrow needed.

            // L5: anti-CPI guard, mirroring the Secp256r1 authenticator check.
            // A session-authenticated Execute is only valid as a top-level instruction
            // (stack_height == 1). Rejecting CPI entry prevents any future bugs where
            // a wrapper program could chain through Execute with forged account context.
            if get_stack_height() > 1 {
                return Err(AuthError::PermissionDenied.into());
            }

            if authority_data.len()
                < std::mem::size_of::<crate::state::session::SessionAccount>()
            {
                return Err(ProgramError::InvalidAccountData);
            }

            let session = unsafe {
                std::ptr::read_unaligned(
                    authority_data.as_ptr() as *const crate::state::session::SessionAccount,
                )
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

            // Pre-CPI action checks (program whitelist/blacklist)
            if has_actions(authority_data) {
                evaluate_pre_actions(
                    authority_data,
                    &compact_instructions,
                    accounts,
                    current_slot,
                )?;
            }

            is_session = true;
            session_slot = current_slot;
        }
        _ => return Err(ProgramError::InvalidAccountData),
    }

    // Get vault bump for signing
    let (vault_key, vault_bump) =
        find_program_address(&[b"vault", wallet_pda.key().as_ref()], program_id);

    // Verify vault PDA.
    if vault_pda.key() != &vault_key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Snapshot balances before CPI (for session action enforcement)
    let vault_lamports_before = if is_session { vault_pda.lamports() } else { 0 };
    let token_snapshots_before = if is_session {
        // Reuse the existing `authority_data` borrow — no additional borrow of authority_pda.
        snapshot_token_balances(authority_data, accounts, vault_pda.key())?
    } else {
        Vec::new()
    };

    // ── Session invariants (defense against System::Assign / SetAuthority escapes) ──
    // A session that whitelists System Program (a common pattern for SOL transfers)
    // could otherwise craft `System::Assign(vault, attacker)` — the lamport-based
    // limits see no outflow, but ownership of the vault silently transfers to the
    // attacker, who then drains it in a follow-up tx. Same class of attack via
    // SPL Token's `SetAuthority` / `Approve` on vault-owned token accounts.
    //
    // Snapshot the vault's metadata + every listed-mint vault-owned token account's
    // authority fields BEFORE the CPI loop; verify unchanged AFTER.
    let session_has_actions = is_session && has_actions(authority_data);
    let vault_owner_before = if session_has_actions {
        Some(*vault_pda.owner())
    } else {
        None
    };
    let vault_data_len_before = if session_has_actions {
        Some(unsafe { vault_pda.borrow_data_unchecked().len() })
    } else {
        None
    };
    let token_authority_snapshots = if session_has_actions {
        snapshot_token_authorities(authority_data, accounts, vault_pda.key())?
    } else {
        Vec::new()
    };

    // Track gross SOL outflow across all CPIs (for SolMaxPerTx check)
    let mut vault_lamports_gross_out: u64 = 0;
    let mut prev_vault_lamports = vault_lamports_before;

    // Reuse the same Vecs across all inner CPIs — allocated once, cleared +
    // repushed each iteration. Saves 2 Vec::with_capacity allocations per
    // inner instruction vs. .collect()ing fresh Vecs each time.
    const MAX_INNER_ACCOUNTS: usize = 32;
    let mut account_metas: Vec<AccountMeta> = Vec::with_capacity(MAX_INNER_ACCOUNTS);
    let mut cpi_accounts: Vec<Account> = Vec::with_capacity(MAX_INNER_ACCOUNTS);

    // PDA signer seeds (constant across the loop)
    let vault_bump_arr = [vault_bump];
    let seeds = [
        Seed::from(b"vault"),
        Seed::from(wallet_pda.key().as_ref()),
        Seed::from(&vault_bump_arr),
    ];

    // Execute each compact instruction
    for compact_ix in &compact_instructions {
        let decompressed = compact_ix.decompress(accounts)?;

        // Prevent self-reentrancy (Issue #10)
        if decompressed.program_id.as_ref() == program_id.as_ref() {
            return Err(AuthError::SelfReentrancyNotAllowed.into());
        }

        account_metas.clear();
        cpi_accounts.clear();
        for &acc in &decompressed.accounts {
            account_metas.push(AccountMeta {
                pubkey: acc.key(),
                is_signer: acc.is_signer() || acc.key() == vault_pda.key(),
                is_writable: acc.is_writable(),
            });
            cpi_accounts.push(Account::from(acc));
        }

        let ix = Instruction {
            program_id: decompressed.program_id,
            accounts: &account_metas,
            data: decompressed.data,
        };

        let signer: Signer = (&seeds).into();

        unsafe {
            invoke_signed_unchecked(&ix, &cpi_accounts, &[signer]);
        }

        // Track gross SOL outflow per CPI (used for SolMaxPerTx — not net balance diff).
        if is_session {
            let post = vault_pda.lamports();
            if prev_vault_lamports > post {
                vault_lamports_gross_out = vault_lamports_gross_out
                    .saturating_add(prev_vault_lamports - post);
            }
            prev_vault_lamports = post;
        }
    }

    // ── Post-CPI session invariants ────────────────────────────────────
    // Verify vault's ownership and data layout were not tampered with. Any
    // change (System::Assign, Allocate, AllocateWithSeed, AssignWithSeed) is
    // rejected. This complements the balance-based limits below.
    if let Some(owner_before) = vault_owner_before {
        if *vault_pda.owner() != owner_before {
            return Err(AuthError::SessionVaultOwnerChanged.into());
        }
    }
    if let Some(len_before) = vault_data_len_before {
        let len_after = unsafe { vault_pda.borrow_data_unchecked().len() };
        if len_after != len_before {
            return Err(AuthError::SessionVaultDataLenChanged.into());
        }
    }
    // Verify no SetAuthority / Approve on listed-mint vault-owned token accounts.
    verify_token_authorities_unchanged(&token_authority_snapshots, accounts)?;

    // Post-CPI action checks (spending limits)
    // Reuse the existing `authority_data` borrow — no additional borrow of authority_pda.
    if session_has_actions {
        evaluate_post_actions(
            authority_data,
            accounts,
            vault_pda.key(),
            vault_lamports_before,
            vault_pda.lamports(),
            vault_lamports_gross_out,
            &token_snapshots_before,
            session_slot,
        )?;
    }

    Ok(())
}

/// Compute SHA256 hash of all account pubkeys referenced by compact instructions (Issue #11).
///
/// Optimisation: pass each 32-byte pubkey as a separate slice to sol_sha256
/// instead of concatenating them into an owned Vec first. sol_sha256 accepts
/// an array of slices natively, so the concat step was pure overhead.
fn compute_accounts_hash(
    accounts: &[AccountInfo],
    compact_instructions: &[CompactInstructionRef<'_>],
) -> Result<[u8; 32], ProgramError> {
    // Collect slice references (16 bytes each) instead of copying 32-byte pubkeys.
    // With MAX_COMPACT_INSTRUCTIONS = 16 and a reasonable per-ix account count,
    // this fits comfortably on the BPF heap.
    let mut refs: Vec<&[u8]> = Vec::with_capacity(compact_instructions.len() * 4);

    for ix in compact_instructions {
        let program_idx = ix.program_id_index as usize;
        if program_idx >= accounts.len() {
            return Err(ProgramError::InvalidInstructionData);
        }
        refs.push(accounts[program_idx].key().as_ref());

        for &acc_idx in ix.accounts {
            let idx = acc_idx as usize;
            if idx >= accounts.len() {
                return Err(ProgramError::InvalidInstructionData);
            }
            refs.push(accounts[idx].key().as_ref());
        }
    }

    #[allow(unused_assignments)]
    let mut hash = [0u8; 32];
    #[cfg(target_os = "solana")]
    unsafe {
        pinocchio::syscalls::sol_sha256(
            refs.as_ptr() as *const u8,
            refs.len() as u64,
            hash.as_mut_ptr(),
        );
    }
    #[cfg(not(target_os = "solana"))]
    {
        hash = [0xAA; 32];
        let _ = refs;
    }

    Ok(hash)
}
