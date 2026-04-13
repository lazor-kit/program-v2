use crate::{
    compact::parse_compact_instructions,
    error::AuthError,
    state::{deferred::DeferredExecAccount, AccountDiscriminator},
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

/// Process the ExecuteDeferred instruction (deferred execution tx2).
///
/// Verifies the compact instructions against the stored hash, executes them
/// via CPI with vault PDA signing, then closes the DeferredExec account.
///
/// # Accounts:
/// 1. `[signer, writable]` Payer
/// 2. `[]` Wallet PDA
/// 3. `[writable]` Vault PDA (signer for CPI)
/// 4. `[writable]` DeferredExec PDA (read + closed)
/// 5. `[writable]` Refund destination (receives rent refund)
/// 6. `...` Inner accounts referenced by compact instructions
///
/// # Instruction Data (after discriminator):
///   [compact_instructions(variable)]
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
    let vault_pda = accounts
        .get(2)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let deferred_pda = accounts
        .get(3)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let refund_dest = accounts
        .get(4)
        .ok_or(ProgramError::NotEnoughAccountKeys)?;

    // Validate payer
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Verify ownership of wallet and deferred
    if wallet_pda.owner() != program_id || deferred_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Validate Wallet discriminator
    let wallet_data = unsafe { wallet_pda.borrow_data_unchecked() };
    if wallet_data.is_empty() || wallet_data[0] != AccountDiscriminator::Wallet as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    // Read DeferredExec account (read-only borrow for validation)
    {
        let deferred_check = unsafe { deferred_pda.borrow_data_unchecked() };
        if deferred_check.len() < std::mem::size_of::<DeferredExecAccount>() {
            return Err(ProgramError::InvalidAccountData);
        }
    }

    let deferred = unsafe {
        let data = deferred_pda.borrow_data_unchecked();
        std::ptr::read_unaligned(data.as_ptr() as *const DeferredExecAccount)
    };

    if deferred.discriminator != AccountDiscriminator::DeferredExec as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify wallet matches
    if deferred.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Verify refund destination matches stored payer
    if deferred.payer != *refund_dest.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check expiry
    let clock = Clock::get()?;
    if clock.slot > deferred.expires_at {
        return Err(AuthError::DeferredAuthorizationExpired.into());
    }

    // Parse compact instructions
    let compact_instructions = parse_compact_instructions(instruction_data)?;

    // Serialize compact instructions to compute hash
    let compact_bytes = crate::compact::serialize_compact_instructions(&compact_instructions);

    // Verify instructions hash
    let instructions_hash = compute_sha256(&compact_bytes);
    if instructions_hash != deferred.instructions_hash {
        return Err(AuthError::DeferredHashMismatch.into());
    }

    // Verify accounts hash
    let accounts_hash = compute_accounts_hash(accounts, &compact_instructions)?;
    if accounts_hash != deferred.accounts_hash {
        return Err(AuthError::DeferredHashMismatch.into());
    }

    // Derive vault PDA and verify
    let (vault_key, vault_bump) =
        find_program_address(&[b"vault", wallet_pda.key().as_ref()], program_id);

    if vault_pda.key() != &vault_key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Close the DeferredExec account BEFORE CPI execution.
    // All validation is complete — hashes verified, expiry checked.
    // Closing before CPI avoids stale-pointer issues with invoke_signed_unchecked.
    // If any CPI fails, the entire transaction reverts atomically.
    let deferred_lamports = unsafe { *deferred_pda.borrow_mut_lamports_unchecked() };
    let refund_lamports = unsafe { *refund_dest.borrow_mut_lamports_unchecked() };
    unsafe {
        *refund_dest.borrow_mut_lamports_unchecked() = refund_lamports
            .checked_add(deferred_lamports)
            .ok_or(ProgramError::ArithmeticOverflow)?;
        *deferred_pda.borrow_mut_lamports_unchecked() = 0;
    }
    let close_data = unsafe { deferred_pda.borrow_mut_data_unchecked() };
    close_data.fill(0);

    // Execute each compact instruction via CPI with vault PDA signing
    for compact_ix in &compact_instructions {
        let decompressed = compact_ix.decompress(accounts)?;

        // Build AccountMeta array
        let account_metas: Vec<AccountMeta> = decompressed
            .accounts
            .iter()
            .map(|acc| AccountMeta {
                pubkey: acc.key(),
                is_signer: acc.is_signer() || acc.key() == vault_pda.key(),
                is_writable: acc.is_writable(),
            })
            .collect();

        // Prevent self-reentrancy
        if decompressed.program_id.as_ref() == program_id.as_ref() {
            return Err(AuthError::SelfReentrancyNotAllowed.into());
        }

        let ix = Instruction {
            program_id: decompressed.program_id,
            accounts: &account_metas,
            data: &decompressed.data,
        };

        let vault_bump_arr = [vault_bump];
        let seeds = [
            Seed::from(b"vault"),
            Seed::from(wallet_pda.key().as_ref()),
            Seed::from(&vault_bump_arr),
        ];
        let signer: Signer = (&seeds).into();

        let cpi_accounts: Vec<Account> = decompressed
            .accounts
            .iter()
            .map(|acc| Account::from(*acc))
            .collect();

        unsafe {
            invoke_signed_unchecked(&ix, &cpi_accounts, &[signer]);
        }
    }

    Ok(())
}

/// Compute SHA256 hash of bytes.
fn compute_sha256(data: &[u8]) -> [u8; 32] {
    #[allow(unused_assignments)]
    let mut hash = [0u8; 32];
    #[cfg(target_os = "solana")]
    unsafe {
        pinocchio::syscalls::sol_sha256(
            [data].as_ptr() as *const u8,
            1,
            hash.as_mut_ptr(),
        );
    }
    #[cfg(not(target_os = "solana"))]
    {
        hash = [0xAA; 32];
        let _ = data;
    }
    hash
}

/// Compute SHA256 hash of all account pubkeys referenced by compact instructions.
/// Same logic as execute.rs::compute_accounts_hash.
fn compute_accounts_hash(
    accounts: &[AccountInfo],
    compact_instructions: &[crate::compact::CompactInstruction],
) -> Result<[u8; 32], ProgramError> {
    let mut pubkeys_data = Vec::new();

    for ix in compact_instructions {
        let program_idx = ix.program_id_index as usize;
        if program_idx >= accounts.len() {
            return Err(ProgramError::InvalidInstructionData);
        }
        pubkeys_data.extend_from_slice(accounts[program_idx].key().as_ref());

        for &acc_idx in &ix.accounts {
            let idx = acc_idx as usize;
            if idx >= accounts.len() {
                return Err(ProgramError::InvalidInstructionData);
            }
            pubkeys_data.extend_from_slice(accounts[idx].key().as_ref());
        }
    }

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
        hash = [0xAA; 32];
        let _ = pubkeys_data;
    }

    Ok(hash)
}
