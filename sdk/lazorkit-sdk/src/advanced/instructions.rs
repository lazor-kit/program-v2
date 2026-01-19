use lazorkit_program::instruction::LazorKitInstruction;
use lazorkit_state::authority::AuthorityType;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::system_program;

pub fn create_wallet(
    program_id: &Pubkey,
    payer: &Pubkey,
    wallet_id: [u8; 32],
    owner_auth_type: AuthorityType,
    owner_auth_data: Vec<u8>,
) -> Instruction {
    let (config_pda, bump) = Pubkey::find_program_address(&[b"lazorkit", &wallet_id], program_id);
    let (vault_pda, wallet_bump) = Pubkey::find_program_address(
        &[b"lazorkit-wallet-address", config_pda.as_ref()],
        program_id,
    );

    let instruction = LazorKitInstruction::CreateWallet {
        id: wallet_id,
        bump,
        wallet_bump,
        owner_authority_type: owner_auth_type as u16,
        owner_authority_data: owner_auth_data,
    };

    let accounts = vec![
        AccountMeta::new(config_pda, false),
        AccountMeta::new(*payer, true),
        AccountMeta::new(vault_pda, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(&instruction).unwrap(),
    }
}

pub fn add_authority(
    program_id: &Pubkey,
    wallet: &Pubkey,
    payer: &Pubkey,
    acting_role_id: u32,
    new_auth_type: AuthorityType,
    new_auth_data: Vec<u8>,
    authorization_data: Vec<u8>,
    additional_accounts: Vec<AccountMeta>,
) -> Instruction {
    let instruction = LazorKitInstruction::AddAuthority {
        acting_role_id,
        authority_type: new_auth_type as u16,
        authority_data: new_auth_data,
        authorization_data,
    };

    let mut accounts = vec![
        AccountMeta::new(*wallet, false),
        AccountMeta::new(*payer, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    accounts.extend(additional_accounts);

    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(&instruction).unwrap(),
    }
}

pub fn execute(
    program_id: &Pubkey,
    config: &Pubkey,
    vault: &Pubkey,
    acting_role_id: u32,
    payload: Vec<u8>,
    account_metas: Vec<AccountMeta>,
) -> Instruction {
    let instruction = LazorKitInstruction::Execute {
        role_id: acting_role_id,
        instruction_payload: payload,
    };

    let mut accounts = vec![
        AccountMeta::new(*config, false),
        AccountMeta::new(*vault, false),
        AccountMeta::new_readonly(system_program::id(), false),
    ];

    accounts.extend(account_metas);

    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(&instruction).unwrap(),
    }
}

pub fn remove_authority(
    program_id: &Pubkey,
    config: &Pubkey,
    payer: &Pubkey,
    acting_role_id: u32,
    target_role_id: u32,
    authorization_data: Vec<u8>,
    additional_accounts: Vec<AccountMeta>,
) -> Instruction {
    let instruction = LazorKitInstruction::RemoveAuthority {
        acting_role_id,
        target_role_id,
        authorization_data,
    };

    let mut accounts = vec![
        AccountMeta::new(*config, false),
        AccountMeta::new(*payer, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    accounts.extend(additional_accounts);

    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(&instruction).unwrap(),
    }
}

pub fn update_authority(
    program_id: &Pubkey,
    config: &Pubkey,
    payer: &Pubkey,
    acting_role_id: u32,
    target_role_id: u32,
    new_authority_data: Vec<u8>,
    authorization_data: Vec<u8>,
    additional_accounts: Vec<AccountMeta>,
) -> Instruction {
    let instruction = LazorKitInstruction::UpdateAuthority {
        acting_role_id,
        target_role_id,
        new_authority_data,
        authorization_data,
    };

    let mut accounts = vec![
        AccountMeta::new(*config, false),
        AccountMeta::new(*payer, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    accounts.extend(additional_accounts);

    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(&instruction).unwrap(),
    }
}

pub fn create_session(
    program_id: &Pubkey,
    config: &Pubkey,
    payer: &Pubkey,
    role_id: u32,
    session_key: [u8; 32],
    duration: u64,
    authorization_data: Vec<u8>,
    additional_accounts: Vec<AccountMeta>,
) -> Instruction {
    let instruction = LazorKitInstruction::CreateSession {
        role_id,
        session_key,
        duration,
        authorization_data,
    };

    let mut accounts = vec![
        AccountMeta::new(*config, false),
        AccountMeta::new(*payer, true),
        AccountMeta::new_readonly(system_program::id(), false),
    ];
    accounts.extend(additional_accounts);

    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(&instruction).unwrap(),
    }
}

/// Creates a transfer ownership instruction
///
/// NOTE: This helper currently only supports Ed25519 authorities where the signer
/// is provided directly in accounts. For Secp256k1/r1/ProgramExec, you'll need to
/// construct the auth_payload manually with proper signatures.
///
/// # Arguments
/// * `program_id` - LazorKit program ID
/// * `config` - Config account pubkey
/// * `current_owner` - Current owner (signer)
/// * `new_owner_type` - Type ID (1-8) for new authority
/// * `new_owner_data` - Authority data bytes
pub fn transfer_ownership(
    program_id: &Pubkey,
    config: &Pubkey,
    current_owner: &Pubkey,
    new_owner_type: u16,
    new_owner_data: Vec<u8>,
) -> Instruction {
    let instruction = LazorKitInstruction::TransferOwnership {
        new_owner_authority_type: new_owner_type,
        new_owner_authority_data: new_owner_data,
        // For Ed25519, auth_payload can be empty as signer is verified via is_signer()
        // For other auth types, caller needs to provide proper signature payload
        auth_payload: vec![],
    };

    let accounts = vec![
        AccountMeta::new(*config, false),
        AccountMeta::new_readonly(*current_owner, true),
    ];

    Instruction {
        program_id: *program_id,
        accounts,
        data: borsh::to_vec(&instruction).unwrap(),
    }
}
