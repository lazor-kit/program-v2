use pinocchio::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction, Seed, Signer},
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    ProgramResult,
};

/// System Program ID (11111111111111111111111111111111)
pub const SYSTEM_PROGRAM_ID: [u8; 32] = [0u8; 32];

/// Wrapper around the `sol_get_stack_height` syscall
pub fn get_stack_height() -> u64 {
    #[cfg(target_os = "solana")]
    unsafe {
        pinocchio::syscalls::sol_get_stack_height()
    }
    #[cfg(not(target_os = "solana"))]
    0
}

/// Safely initializes a PDA account using transfer-allocate-assign pattern.
///
/// This prevents DoS attacks where malicious actors pre-fund target accounts
/// with small amounts of lamports, causing the System Program's `create_account`
/// instruction to fail (since it rejects accounts with non-zero balances).
///
/// The transfer-allocate-assign pattern works in three steps:
/// 1. **Transfer**: Add lamports to reach rent-exemption (if needed)
/// 2. **Allocate**: Set the account's data size
/// 3. **Assign**: Transfer ownership to the target program
///
/// # Security
/// - Prevents Issue #4: Create Account DoS vulnerability
/// - Still enforces rent-exemption requirements
/// - Properly assigns ownership to prevent unauthorized access
/// - Works even if account is pre-funded by attacker
///
/// # Arguments
/// * `payer` - Account paying for initialization (must be signer & writable)
/// * `target_pda` - PDA being initialized (will be writable)
/// * `system_program` - System Program account
/// * `space` - Number of bytes to allocate for account data
/// * `rent_lamports` - Minimum lamports for rent-exemption
/// * `owner` - Program that will own this account
/// * `pda_seeds` - Seeds for PDA signing (for allocate & assign)
///
/// # Errors
/// Returns ProgramError if:
/// - Payer has insufficient funds
/// - Any CPI call fails
/// - Account is already owned by another program
pub fn initialize_pda_account(
    payer: &AccountInfo,
    target_pda: &AccountInfo,
    system_program: &AccountInfo,
    space: usize,
    rent_lamports: u64,
    owner: &Pubkey,
    pda_seeds: &[Seed],
) -> ProgramResult {
    // Validate System Program ID
    if system_program.key() != &SYSTEM_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    let current_balance = target_pda.lamports();

    // Step 1: Transfer lamports if needed to reach rent-exemption
    if current_balance < rent_lamports {
        let transfer_amount = rent_lamports
            .checked_sub(current_balance)
            .ok_or(ProgramError::ArithmeticOverflow)?;

        // System Program Transfer instruction (discriminator: 2)
        let mut transfer_data = Vec::with_capacity(12);
        transfer_data.extend_from_slice(&2u32.to_le_bytes());
        transfer_data.extend_from_slice(&transfer_amount.to_le_bytes());

        let transfer_accounts = [
            AccountMeta {
                pubkey: payer.key(),
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: target_pda.key(),
                is_signer: false,
                is_writable: true,
            },
        ];

        let transfer_ix = Instruction {
            program_id: &Pubkey::from(SYSTEM_PROGRAM_ID),
            accounts: &transfer_accounts,
            data: &transfer_data,
        };

        pinocchio::program::invoke(&transfer_ix, &[&payer, &target_pda, &system_program])?;
    }

    // Step 2: Allocate space
    // System Program Allocate instruction (discriminator: 8)
    let mut allocate_data = Vec::with_capacity(12);
    allocate_data.extend_from_slice(&8u32.to_le_bytes());
    allocate_data.extend_from_slice(&(space as u64).to_le_bytes());

    let allocate_accounts = [AccountMeta {
        pubkey: target_pda.key(),
        is_signer: true,
        is_writable: true,
    }];

    let allocate_ix = Instruction {
        program_id: &Pubkey::from(SYSTEM_PROGRAM_ID),
        accounts: &allocate_accounts,
        data: &allocate_data,
    };

    let signer: Signer = pda_seeds.into();
    invoke_signed(&allocate_ix, &[&target_pda, &system_program], &[signer])?;

    // Step 3: Assign ownership to target program
    // System Program Assign instruction (discriminator: 1)
    let mut assign_data = Vec::with_capacity(36);
    assign_data.extend_from_slice(&1u32.to_le_bytes());
    assign_data.extend_from_slice(owner.as_ref());

    let assign_accounts = [AccountMeta {
        pubkey: target_pda.key(),
        is_signer: true,
        is_writable: true,
    }];

    let assign_ix = Instruction {
        program_id: &Pubkey::from(SYSTEM_PROGRAM_ID),
        accounts: &assign_accounts,
        data: &assign_data,
    };

    let signer: Signer = pda_seeds.into();
    invoke_signed(&assign_ix, &[&target_pda, &system_program], &[signer])?;

    Ok(())
}
