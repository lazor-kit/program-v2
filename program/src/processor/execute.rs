use crate::{
    auth::{ed25519, secp256r1},
    compact::parse_compact_instructions,
    error::AuthError,
    state::{authority::AuthorityAccountHeader, AccountDiscriminator},
};
use pinocchio::{
    account_info::AccountInfo,
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

/// Process the Execute instruction  
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
    let inner_accounts = &accounts[inner_accounts_start..];

    // Verify ownership
    if wallet_pda.owner() != program_id || authority_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }

    // Read authority header
    let mut authority_data = unsafe { authority_pda.borrow_mut_data_unchecked() };
    if authority_data.len() < std::mem::size_of::<AuthorityAccountHeader>() {
        return Err(ProgramError::InvalidAccountData);
    }

    let authority_header = unsafe { &*(authority_data.as_ptr() as *const AuthorityAccountHeader) };

    if authority_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }

    if authority_header.wallet != *wallet_pda.key() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Parse compact instructions
    let compact_instructions = parse_compact_instructions(instruction_data)?;

    // Serialize compact instructions to get their byte length
    let compact_bytes = crate::compact::serialize_compact_instructions(&compact_instructions);
    let compact_len = compact_bytes.len();

    // Everything after compact instructions is authority payload
    let authority_payload = if instruction_data.len() > compact_len {
        &instruction_data[compact_len..]
    } else {
        &[]
    };

    // Get current slot for Secp256r1
    let clock = Clock::get()?;
    let current_slot = clock.slot;

    // Authenticate based on authority type
    match authority_header.authority_type {
        0 => {
            // Ed25519: Verify signer
            ed25519::authenticate(&authority_data, accounts)?;
        },
        1 => {
            // Secp256r1: Full authentication
            secp256r1::authenticate(
                &mut authority_data,
                accounts,
                authority_payload,
                &compact_bytes,
                current_slot,
            )?;
        },
        _ => return Err(AuthError::InvalidAuthenticationKind.into()),
    }

    // Get vault bump for signing
    let (vault_key, vault_bump) =
        find_program_address(&[b"vault", wallet_pda.key().as_ref()], program_id);

    // Verify vault PDA
    if vault_pda.key() != &vault_key {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create seeds for vault signing
    let vault_bump_arr = [vault_bump];
    let signersseeds: &[&[u8]] = &[b"vault", wallet_pda.key().as_ref(), &vault_bump_arr];

    // Execute each compact instruction
    for compact_ix in &compact_instructions {
        let decompressed = compact_ix.decompress(inner_accounts)?;

        // Build AccountMeta array for instruction
        let mut account_metas = Vec::with_capacity(decompressed.accounts.len());
        for acc in &decompressed.accounts {
            account_metas.push(pinocchio::instruction::AccountMeta {
                pubkey: acc.key(),
                is_signer: acc.is_signer(),
                is_writable: acc.is_writable(),
            });
        }

        // Use pinocchio's raw syscall for invoke_signed with dynamic account counts
        // This builds the instruction data manually to work around const generic requirements
        unsafe {
            #[cfg(target_os = "solana")]
            {
                // Build instruction in the format Solana expects
                let ix_account_metas_ptr = account_metas.as_ptr() as *const u8;
                let ix_account_metas_len = account_metas.len();
                let ix_data_ptr = decompressed.data.as_ptr();
                let ix_data_len = decompressed.data.len();
                let ix_program_id = decompressed.program_id.as_ref().as_ptr();

                // Account infos for CPI
                let account_infos_ptr = decompressed.accounts.as_ptr() as *const u8;
                let account_infos_len = decompressed.accounts.len();

                // Signers seeds
                let signers_seeds_ptr = &signersseeds as *const &[&[u8]] as *const u8;
                let signers_seeds_len = 1;

                // Call raw syscall
                let result = pinocchio::syscalls::sol_invoke_signed_rust(
                    ix_program_id,
                    ix_account_metas_ptr,
                    ix_account_metas_len as u64,
                    ix_data_ptr,
                    ix_data_len as u64,
                    account_infos_ptr,
                    account_infos_len as u64,
                    signers_seeds_ptr,
                    signers_seeds_len as u64,
                );

                if result != 0 {
                    return Err(ProgramError::from(result));
                }
            }

            #[cfg(not(target_os = "solana"))]
            {
                // For testing, just succeed
                let _ = (decompressed, signersseeds);
            }
        }
    }

    Ok(())
}
