//! Cross-program invocation utilities.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed_unchecked,
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
/// This matches Swig's pattern using solana_program.
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
    let mut sol_account_infos = Vec::with_capacity(account_infos.len());
    for info in account_infos {
        let key: &solana_program::pubkey::Pubkey = unsafe { core::mem::transmute(info.key()) };
        let is_signer = info.is_signer();
        let is_writable = info.is_writable();
        let lamports_ptr =
            (unsafe { info.borrow_mut_lamports_unchecked() } as *const u64 as usize) as *mut u64;
        let lamports_ref = unsafe { &mut *lamports_ptr };
        let lamports = std::rc::Rc::new(std::cell::RefCell::new(lamports_ref));

        let data_ptr = unsafe { info.borrow_mut_data_unchecked() } as *const [u8] as *mut [u8];
        let data_ref = unsafe { &mut *data_ptr };
        let data = std::rc::Rc::new(std::cell::RefCell::new(data_ref));
        let owner: &solana_program::pubkey::Pubkey = unsafe { core::mem::transmute(info.owner()) };
        let executable = info.executable();
        let rent_epoch = 0; // Deprecated/unused mostly

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
        .map_err(|_| ProgramError::Custom(0)) // simplified error mapping
}
