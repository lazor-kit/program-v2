//! Cross-program invocation utilities.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};

/// Helper to find AccountInfo for a given Pubkey
pub fn find_account_info<'a>(
    key: &Pubkey,
    accounts: &'a [AccountInfo],
) -> Result<&'a AccountInfo, ProgramError> {
    for account in accounts {
        if account.key() == key {
            return Ok(account);
        }
    }
    Err(ProgramError::MissingRequiredSignature)
}

/// Invoke a program instruction with signer seeds.
/// 
/// FIXED: Preserves length information when converting AccountInfo to avoid access violations.
pub fn invoke_signed_dynamic(
    instruction: &Instruction,
    account_infos: &[&AccountInfo],
    signers_seeds: &[&[&[u8]]],
) -> ProgramResult {
    // Convert Pinocchio Instruction to Solana Instruction
    let sol_instruction = solana_program::instruction::Instruction {
        program_id: solana_program::pubkey::Pubkey::from(*instruction.program_id),
        accounts: instruction
            .accounts
            .iter()
            .map(|meta| {
                let key_ptr = meta.pubkey as *const [u8; 32];
                let key_bytes = unsafe { *key_ptr };
                solana_program::instruction::AccountMeta {
                    pubkey: solana_program::pubkey::Pubkey::from(key_bytes),
                    is_signer: meta.is_signer,
                    is_writable: meta.is_writable,
                }
            })
            .collect(),
        data: instruction.data.to_vec(),
    };

    // Convert Pinocchio AccountInfos to Solana AccountInfos
    // FIXED: Preserve length information explicitly to avoid corruption
    let mut sol_account_infos = Vec::with_capacity(account_infos.len());
    for info in account_infos {
        let key: &solana_program::pubkey::Pubkey = unsafe { core::mem::transmute(info.key()) };
        let is_signer = info.is_signer();
        let is_writable = info.is_writable();
        
        // Get lamports reference
        let lamports_ptr =
            (unsafe { info.borrow_mut_lamports_unchecked() } as *const u64 as usize) as *mut u64;
        let lamports_ref = unsafe { &mut *lamports_ptr };
        let lamports = std::rc::Rc::new(std::cell::RefCell::new(lamports_ref));

        // FIXED: Preserve length explicitly when creating data slice
        let data_slice = unsafe { info.borrow_mut_data_unchecked() };
        let data_len = data_slice.len(); // Get length BEFORE casting
        let data_ptr = data_slice.as_mut_ptr(); // Get pointer
        // Create slice with explicit length to avoid corruption
        let data_ref = unsafe { std::slice::from_raw_parts_mut(data_ptr, data_len) };
        let data = std::rc::Rc::new(std::cell::RefCell::new(data_ref));
        
        let owner: &solana_program::pubkey::Pubkey = unsafe { core::mem::transmute(info.owner()) };
        let executable = info.executable();
        let rent_epoch = 0;

        let sol_info = solana_program::account_info::AccountInfo {
            key,
            is_signer,
            is_writable,
            lamports,
            data,
            owner,
            executable,
            rent_epoch,
        };
        sol_account_infos.push(sol_info);
    }

    solana_program::program::invoke_signed(&sol_instruction, &sol_account_infos, signers_seeds)
        .map_err(|_| ProgramError::Custom(0))
}
