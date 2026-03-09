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

/// Maps a pubkey to a shard ID deterministically and stably across platforms.
pub fn hash_pubkey_to_shard(pubkey: &Pubkey, num_shards: u8) -> u8 {
    if num_shards == 0 {
        return 0; // Fallback, though config should ensure num_shards >= 1
    }
    let mut sum: u32 = 0;
    for &b in pubkey.as_ref() {
        sum = sum.wrapping_add(b as u32);
    }
    (sum % (num_shards as u32)) as u8
}

/// Collects the protocol fee from the payer and transfers it to the assigned treasury shard.
///
/// # Arguments
/// * `payer` - Account paying for the fee (must be signer & writable)
/// * `config_account` - The global config account data to read fees/shards from
/// * `treasury_shard` - The pre-initialized treasury shard PDA receiving the fee (must be writable)
/// * `system_program` - System Program account
/// * `is_wallet_creation` - If true, applies `wallet_fee`, otherwise `action_fee`
pub fn collect_protocol_fee(
    program_id: &Pubkey,
    payer: &AccountInfo,
    config_account: &crate::state::config::ConfigAccount,
    treasury_shard: &AccountInfo,
    system_program: &AccountInfo,
    is_wallet_creation: bool,
) -> ProgramResult {
    let fee = if is_wallet_creation {
        config_account.wallet_fee
    } else {
        config_account.action_fee
    };

    if fee == 0 {
        return Ok(()); // Free action
    }

    // Verify system program
    if system_program.key() != &SYSTEM_PROGRAM_ID {
        return Err(ProgramError::IncorrectProgramId);
    }

    // Verify Treasury Shard is the correct one for this payer
    let shard_id = hash_pubkey_to_shard(payer.key(), config_account.num_shards);
    let shard_id_bytes = [shard_id];
    let (expected_shard_key, _bump) =
        pinocchio::pubkey::find_program_address(&[b"treasury", &shard_id_bytes], program_id);

    if treasury_shard.key() != &expected_shard_key {
        return Err(ProgramError::InvalidSeeds);
    }

    // System Program Transfer instruction (discriminator: 2)
    let mut transfer_data = Vec::with_capacity(12);
    transfer_data.extend_from_slice(&2u32.to_le_bytes());
    transfer_data.extend_from_slice(&fee.to_le_bytes());

    let transfer_accounts = [
        AccountMeta {
            pubkey: payer.key(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: treasury_shard.key(),
            is_signer: false,
            is_writable: true, // Shards must be writable
        },
    ];

    let transfer_ix = Instruction {
        program_id: &Pubkey::from(SYSTEM_PROGRAM_ID),
        accounts: &transfer_accounts,
        data: &transfer_data,
    };

    pinocchio::program::invoke(&transfer_ix, &[&payer, &treasury_shard, &system_program])?;

    Ok(())
}
