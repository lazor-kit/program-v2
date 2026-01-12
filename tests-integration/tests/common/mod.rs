//! Common test utilities for Lazorkit V2 tests

use lazorkit_v2_state::{wallet_account::WalletAccount, Discriminator, Transmutable};
use litesvm::LiteSVM;
use solana_sdk::{
    account::Account as SolanaAccount,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::VersionedTransaction,
};

/// Test context for Lazorkit V2 tests
pub struct TestContext {
    pub svm: LiteSVM,
    pub default_payer: Keypair,
}

impl TestContext {
    pub fn new() -> anyhow::Result<Self> {
        let mut svm = LiteSVM::new();
        let default_payer = Keypair::new();

        // Load Lazorkit V2 program
        load_lazorkit_program(&mut svm)?;

        // Load Sol Limit Plugin
        load_sol_limit_plugin(&mut svm)?;

        // Load ProgramWhitelist Plugin
        load_program_whitelist_plugin(&mut svm)?;

        // Airdrop to default payer
        // Convert solana_program::Pubkey to solana_sdk::Pubkey
        let payer_program_pubkey = default_payer.pubkey();
        let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
            .map_err(|_| anyhow::anyhow!("Failed to convert Pubkey"))?;
        svm.airdrop(&payer_pubkey, 10_000_000_000)
            .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;

        Ok(Self { svm, default_payer })
    }
}

/// Setup test context
pub fn setup_test_context() -> anyhow::Result<TestContext> {
    TestContext::new()
}

/// Get Lazorkit V2 program ID
pub fn lazorkit_program_id() -> Pubkey {
    // Convert from pinocchio Pubkey to solana_sdk Pubkey
    use pinocchio_pubkey::pubkey as pinocchio_pubkey;
    let pinocchio_id: pinocchio::pubkey::Pubkey =
        pinocchio_pubkey!("BAXwCwbBbs5WmdUkG9EEtFoLsYq2vRADBkdShbRN7w1P");
    // Convert directly from bytes
    Pubkey::try_from(pinocchio_id.as_ref()).expect("Invalid program ID")
}

/// Load Lazorkit V2 program into SVM
pub fn load_lazorkit_program(svm: &mut LiteSVM) -> anyhow::Result<()> {
    // Try to load from deploy directory
    let program_path = "../target/deploy/lazorkit_v2.so";
    let program_id = lazorkit_program_id();
    svm.add_program_from_file(program_id, program_path)
        .map_err(|e| anyhow::anyhow!("Failed to load Lazorkit V2 program from {}: {:?}. Build it first with: cargo build-sbf --manifest-path program/Cargo.toml", program_path, e))
}

/// Get Sol Limit Plugin Program ID
pub fn sol_limit_program_id() -> Pubkey {
    // Arbitrary program ID for testing (all 2s)
    Pubkey::new_from_array([2u8; 32])
}

/// Load Sol Limit Plugin program into SVM
pub fn load_sol_limit_plugin(svm: &mut LiteSVM) -> anyhow::Result<()> {
    let program_path = "../target/deploy/lazorkit_plugin_sol_limit.so";
    let program_id = sol_limit_program_id();
    svm.add_program_from_file(program_id, program_path)
        .map_err(|e| anyhow::anyhow!("Failed to load Sol Limit plugin from {}: {:?}. Build it first with: cargo build-sbf --manifest-path plugins/sol-limit/Cargo.toml", program_path, e))
}

/// Get ProgramWhitelist Plugin Program ID
pub fn program_whitelist_program_id() -> Pubkey {
    // Arbitrary program ID for testing (all 3s)
    let mut bytes = [3u8; 32];
    bytes[0] = 0x77; // 'w' for whitelist
    Pubkey::new_from_array(bytes)
}

/// Load ProgramWhitelist Plugin program into SVM
pub fn load_program_whitelist_plugin(svm: &mut LiteSVM) -> anyhow::Result<()> {
    let program_path = "../target/deploy/lazorkit_plugin_program_whitelist.so";
    let program_id = program_whitelist_program_id();
    svm.add_program_from_file(program_id, program_path)
        .map_err(|e| anyhow::anyhow!("Failed to load ProgramWhitelist plugin from {}: {:?}. Build it first with: cargo build-sbf --manifest-path plugins/program-whitelist/Cargo.toml", program_path, e))
}

/// Helper to create a wallet account PDA seeds as slice
pub fn wallet_account_seeds(id: &[u8; 32]) -> [&[u8]; 2] {
    [b"wallet_account", id]
}

/// Helper to create a wallet vault PDA seeds as slice
pub fn wallet_vault_seeds(wallet_account: &Pubkey) -> [&[u8]; 2] {
    [b"wallet_vault", wallet_account.as_ref()]
}

/// Helper to create a wallet authority PDA seeds as slice
pub fn wallet_authority_seeds<'a>(
    smart_wallet: &'a Pubkey,
    authority_hash: &'a [u8; 32],
) -> [&'a [u8]; 3] {
    [b"wallet_authority", smart_wallet.as_ref(), authority_hash]
}

/// Create a Lazorkit V2 wallet (Hybrid Architecture)
/// Returns (wallet_account, wallet_vault, root_authority_keypair)
pub fn create_lazorkit_wallet(
    context: &mut TestContext,
    id: [u8; 32],
) -> anyhow::Result<(Pubkey, Pubkey, Keypair)> {
    // Convert solana_program::Pubkey to solana_sdk::Pubkey
    let payer_program_pubkey = context.default_payer.pubkey();
    let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
        .map_err(|_| anyhow::anyhow!("Failed to convert Pubkey"))?;

    // Derive PDAs
    let seeds = wallet_account_seeds(&id);
    let (wallet_account, wallet_account_bump) =
        Pubkey::find_program_address(&seeds, &lazorkit_program_id());

    let vault_seeds = wallet_vault_seeds(&wallet_account);
    let (wallet_vault, wallet_vault_bump) =
        Pubkey::find_program_address(&vault_seeds, &lazorkit_program_id());

    // Build CreateSmartWallet instruction
    let root_authority_keypair = Keypair::new();
    let root_authority_pubkey = root_authority_keypair.pubkey();
    let root_authority_data = root_authority_pubkey.to_bytes();

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(0u16).to_le_bytes()); // CreateSmartWallet = 0 (2 bytes)
    instruction_data.extend_from_slice(&id); // id (32 bytes)
    instruction_data.push(wallet_account_bump); // bump (1 byte)
    instruction_data.push(wallet_vault_bump); // wallet_bump (1 byte)
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // first_authority_type = Ed25519 (2 bytes)
    instruction_data.extend_from_slice(&(32u16).to_le_bytes()); // first_authority_data_len = 32 (2 bytes)
    instruction_data.extend_from_slice(&(0u16).to_le_bytes()); // num_plugin_refs = 0 (2 bytes)
    instruction_data.push(0u8); // role_permission = All (default for root)
    instruction_data.push(0u8); // padding
    instruction_data.extend_from_slice(&[0u8; 6]); // Additional padding to align to 48 bytes
    instruction_data.extend_from_slice(&root_authority_data);

    let create_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(wallet_account, false),
            AccountMeta::new(wallet_vault, false),
            AccountMeta::new(payer_pubkey, true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
        ],
        data: instruction_data,
    };

    // Build and send transaction
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            create_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone()],
    )?;

    let result = context.svm.send_transaction(tx);

    match result {
        Ok(_) => Ok((wallet_account, wallet_vault, root_authority_keypair)),
        Err(e) => Err(anyhow::anyhow!("Failed to create wallet: {:?}", e)),
    }
}

/// Add plugin to wallet
/// Returns the plugin index
pub fn add_plugin(
    context: &mut TestContext,
    wallet_state: &Pubkey,
    _smart_wallet: &Pubkey,
    acting_authority: &Keypair,
    acting_authority_id: u32,
    plugin_program_id: Pubkey,
    plugin_config: Pubkey,
) -> anyhow::Result<u16> {
    // Instruction format: [instruction: u16, acting_authority_id: u32, program_id: Pubkey, config_account: Pubkey,
    //                      enabled: u8, priority: u8, padding: [u8; 2]]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(3u16).to_le_bytes()); // AddPlugin = 3
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes()); // acting_authority_id (4 bytes)
    instruction_data.extend_from_slice(plugin_program_id.as_ref()); // program_id (32 bytes)
    instruction_data.extend_from_slice(plugin_config.as_ref()); // config_account (32 bytes)
    instruction_data.push(1u8); // enabled = true
    instruction_data.push(0u8); // priority
    instruction_data.extend_from_slice(&[0u8; 2]); // padding (2 bytes)

    let add_plugin_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_state, false),
            AccountMeta::new(context.default_payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(acting_authority.pubkey(), true),
        ],
        data: instruction_data,
    };

    // Build and send transaction
    // Convert solana_program::Pubkey to solana_sdk::Pubkey
    let payer_program_pubkey = context.default_payer.pubkey();
    let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
        .map_err(|_| anyhow::anyhow!("Failed to convert Pubkey"))?;

    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            add_plugin_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to add plugin: {:?}", e))?;

    // Get plugin index by reading wallet account
    let wallet_account_data = context
        .svm
        .get_account(wallet_state)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet = get_wallet_account(&wallet_account_data)?;
    let plugins = wallet
        .get_plugins(&wallet_account_data.data)
        .map_err(|e| anyhow::anyhow!("Failed to get plugins: {:?}", e))?;

    // Find the plugin we just added
    let plugin_index = plugins
        .iter()
        .position(|p| p.program_id.as_ref() == plugin_program_id.as_ref())
        .ok_or_else(|| anyhow::anyhow!("Plugin not found after adding"))?;

    Ok(plugin_index as u16)
}

/// Create Sign instruction with Ed25519 authority
pub fn create_sign_instruction_ed25519(
    wallet_state: &Pubkey,
    smart_wallet: &Pubkey,
    authority: &Keypair,
    authority_id: u32,
    inner_instruction: Instruction,
) -> anyhow::Result<Instruction> {
    // Build accounts list and find indices
    let mut accounts = vec![
        AccountMeta::new(*wallet_state, false),
        AccountMeta::new(*smart_wallet, false), // Vault must be writable for transfer, non-signer (PDA)
        AccountMeta::new_readonly(authority.pubkey(), true), // Must be signer for Ed25519
    ];

    let mut get_index = |pubkey: &Pubkey, is_writable: bool, is_signer: bool| -> u8 {
        for (i, meta) in accounts.iter_mut().enumerate() {
            if &meta.pubkey == pubkey {
                meta.is_writable |= is_writable;
                meta.is_signer |= is_signer;
                return i as u8;
            }
        }
        let index = accounts.len() as u8;
        accounts.push(if is_writable {
            AccountMeta::new(*pubkey, is_signer)
        } else {
            AccountMeta::new_readonly(*pubkey, is_signer)
        });
        index
    };

    let program_id_index = get_index(&inner_instruction.program_id, false, false);

    // Compact inner instruction
    let mut compacted = Vec::new();
    compacted.push(1u8); // num_instructions
    compacted.push(program_id_index);
    compacted.push(inner_instruction.accounts.len() as u8); // num_accounts
    for account_meta in &inner_instruction.accounts {
        let is_signer = if account_meta.pubkey == *smart_wallet {
            false // Vault is a PDA
        } else {
            account_meta.is_signer
        };
        let idx = get_index(&account_meta.pubkey, account_meta.is_writable, is_signer);
        compacted.push(idx);
    }
    compacted.extend_from_slice(&(inner_instruction.data.len() as u16).to_le_bytes());
    compacted.extend_from_slice(&inner_instruction.data);

    // Build Sign instruction data
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Sign = 1
    instruction_data.extend_from_slice(&(compacted.len() as u16).to_le_bytes());
    instruction_data.extend_from_slice(&(authority_id.to_le_bytes())); // authority_id (u32)
                                                                       // No padding needed - ExecuteArgs is 8 bytes (aligned)
    instruction_data.extend_from_slice(&compacted);

    // Ed25519 authority_payload: [authority_index: u8]
    let authority_payload = vec![2u8]; // Index of authority in accounts
    instruction_data.extend_from_slice(&authority_payload);

    Ok(Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    })
}

/// Add authority with role permission
pub fn add_authority_with_role_permission(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    wallet_vault: &Pubkey,
    new_authority: &Keypair,
    acting_authority_id: u32,
    acting_authority: &Keypair,
    role_permission: lazorkit_v2_state::role_permission::RolePermission,
) -> anyhow::Result<u32> {
    // Calculate authority hash
    let authority_hash = {
        let hasher = solana_sdk::hash::Hash::default();
        let mut hasher_state = hasher.to_bytes();
        hasher_state[..32].copy_from_slice(new_authority.pubkey().as_ref());
        solana_sdk::hash::hashv(&[&hasher_state]).to_bytes()
    };

    let seeds = wallet_authority_seeds(wallet_vault, &authority_hash);
    let (_new_wallet_authority, _authority_bump) =
        Pubkey::find_program_address(&seeds, &lazorkit_program_id());

    // Build AddAuthority instruction
    let authority_data = new_authority.pubkey().to_bytes();
    let authority_data_len = authority_data.len() as u16;

    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519 = 1
    instruction_data.extend_from_slice(&authority_data_len.to_le_bytes());
    instruction_data.extend_from_slice(&0u16.to_le_bytes()); // num_plugin_refs = 0
    instruction_data.push(role_permission as u8); // role_permission
    instruction_data.extend_from_slice(&[0u8; 3]); // padding (3 bytes)
    instruction_data.extend_from_slice(&[0u8; 2]); // Alignment padding to reach 16 bytes
    instruction_data.extend_from_slice(&authority_data);

    // Authority payload for Ed25519
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload: {:?}", e))?;

    let authority_payload_data = vec![4u8]; // acting_authority is at index 4
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload: {:?}", e))?;

    let add_authority_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_account, false),
            AccountMeta::new(context.default_payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(authority_payload_pubkey, false),
            AccountMeta::new_readonly(acting_authority.pubkey(), true),
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");

    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            add_authority_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to add authority: {:?}", e))?;

    // Get the new authority ID by reading the wallet account
    let wallet_account_data = context
        .svm
        .get_account(wallet_account)
        .ok_or_else(|| anyhow::anyhow!("Wallet account not found"))?;
    let wallet = get_wallet_account(&wallet_account_data)?;
    let num_authorities = wallet.num_authorities(&wallet_account_data.data)?;

    // Find the authority we just added (it should be the last one)
    let mut new_authority_id = None;
    for i in 0..num_authorities {
        if let Ok(Some(auth_data)) = wallet.get_authority(&wallet_account_data.data, i as u32) {
            // Check if this authority matches our new authority
            if auth_data.authority_data == authority_data {
                new_authority_id = Some(auth_data.position.id);
                break;
            }
        }
    }

    new_authority_id.ok_or_else(|| anyhow::anyhow!("Failed to find newly added authority"))
}

/// Update authority helper
pub fn update_authority(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    _wallet_vault: &Pubkey,
    acting_authority_id: u32,
    authority_id_to_update: u32,
    acting_authority: &Keypair,
    new_authority_data: &[u8],
) -> anyhow::Result<()> {
    // Build UpdateAuthority instruction
    // Format: [instruction: u16, acting_authority_id: u32, authority_id: u32,
    //          new_authority_type: u16, new_authority_data_len: u16, num_plugin_refs: u16,
    //          padding: [u8; 2], authority_data]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(6u16).to_le_bytes()); // UpdateAuthority = 6
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&authority_id_to_update.to_le_bytes());
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519 = 1
    instruction_data.extend_from_slice(&(new_authority_data.len() as u16).to_le_bytes());
    instruction_data.extend_from_slice(&(0u16).to_le_bytes()); // num_plugin_refs = 0
    instruction_data.extend_from_slice(&[0u8; 2]); // padding
    instruction_data.extend_from_slice(new_authority_data);

    // Authority payload for Ed25519
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload: {:?}", e))?;

    let authority_payload_data = vec![4u8]; // acting_authority is at index 4
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload: {:?}", e))?;

    let update_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_account, false),
            AccountMeta::new(context.default_payer.pubkey(), true),
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
            AccountMeta::new_readonly(authority_payload_pubkey, false),
            AccountMeta::new_readonly(acting_authority.pubkey(), true),
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            update_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to update authority: {:?}", e))?;
    Ok(())
}

/// Remove authority helper
pub fn remove_authority(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    _wallet_vault: &Pubkey,
    acting_authority_id: u32,
    authority_id_to_remove: u32,
    acting_authority: &Keypair,
) -> anyhow::Result<()> {
    // Build RemoveAuthority instruction
    // Format: [instruction: u16, acting_authority_id: u32, authority_id: u32]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(7u16).to_le_bytes()); // RemoveAuthority = 7
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&authority_id_to_remove.to_le_bytes());

    // Authority payload for Ed25519
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload: {:?}", e))?;

    let authority_payload_data = vec![4u8]; // acting_authority is at index 4 (after wallet_account, payer, system_program, authority_payload)
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload: {:?}", e))?;

    let remove_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_account, false), // wallet_account
            AccountMeta::new(context.default_payer.pubkey(), true), // payer (writable, signer)
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false), // system_program
            AccountMeta::new_readonly(authority_payload_pubkey, false), // authority_payload
            AccountMeta::new_readonly(acting_authority.pubkey(), true), // acting_authority
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            remove_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to remove authority: {:?}", e))?;
    Ok(())
}

/// Remove plugin helper
pub fn remove_plugin(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    _wallet_vault: &Pubkey,
    acting_authority_id: u32,
    plugin_index: u16,
    acting_authority: &Keypair,
) -> anyhow::Result<()> {
    // Build RemovePlugin instruction
    // Format: [instruction: u16, acting_authority_id: u32, plugin_index: u16, padding: [u8; 2]]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(4u16).to_le_bytes()); // RemovePlugin = 4
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&plugin_index.to_le_bytes());
    instruction_data.extend_from_slice(&[0u8; 2]); // padding

    // Authority payload for Ed25519
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload: {:?}", e))?;

    let authority_payload_data = vec![2u8]; // acting_authority is at index 2
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload: {:?}", e))?;

    // RemovePlugin requires:
    // 0. wallet_account (writable)
    // 1. smart_wallet (signer) - same as wallet_account (PDA)
    // 2. acting_authority (signer)
    // Note: Program doesn't actually check if smart_wallet is signer (it's just _smart_wallet)
    // So we can mark it as non-signer for LiteSVM tests
    let remove_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_account, false), // wallet_account (writable)
            AccountMeta::new(*wallet_account, false), // smart_wallet (same PDA, not checked as signer)
            AccountMeta::new_readonly(acting_authority.pubkey(), true), // acting_authority
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            remove_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to remove plugin: {:?}", e))?;
    Ok(())
}

/// Update plugin helper
pub fn update_plugin(
    context: &mut TestContext,
    wallet_account: &Pubkey,
    _wallet_vault: &Pubkey,
    acting_authority_id: u32,
    plugin_index: u16,
    enabled: bool,
    priority: u8,
    acting_authority: &Keypair,
) -> anyhow::Result<()> {
    // Build UpdatePlugin instruction
    // Format: [instruction: u16, acting_authority_id: u32, plugin_index: u16, enabled: u8, priority: u8, padding: [u8; 2]]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(5u16).to_le_bytes()); // UpdatePlugin = 5
    instruction_data.extend_from_slice(&acting_authority_id.to_le_bytes());
    instruction_data.extend_from_slice(&plugin_index.to_le_bytes());
    instruction_data.push(if enabled { 1u8 } else { 0u8 });
    instruction_data.push(priority);
    instruction_data.extend_from_slice(&[0u8; 2]); // padding

    // Authority payload for Ed25519
    let authority_payload_keypair = Keypair::new();
    let authority_payload_pubkey = authority_payload_keypair.pubkey();
    context
        .svm
        .airdrop(&authority_payload_pubkey, 1_000_000)
        .map_err(|e| anyhow::anyhow!("Failed to airdrop authority_payload: {:?}", e))?;

    let authority_payload_data = vec![2u8]; // acting_authority is at index 2
    let mut account = context
        .svm
        .get_account(&authority_payload_pubkey)
        .ok_or_else(|| anyhow::anyhow!("Failed to get authority_payload account"))?;
    account.data = authority_payload_data;
    context
        .svm
        .set_account(authority_payload_pubkey, account)
        .map_err(|e| anyhow::anyhow!("Failed to set authority_payload: {:?}", e))?;

    let update_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_account, false), // wallet_account (writable)
            AccountMeta::new(*wallet_account, false), // smart_wallet (same PDA, not checked as signer)
            AccountMeta::new_readonly(acting_authority.pubkey(), true), // acting_authority
        ],
        data: instruction_data,
    };

    let payer_pubkey = Pubkey::try_from(context.default_payer.pubkey().as_ref())
        .expect("Failed to convert Pubkey");
    let message = v0::Message::try_compile(
        &payer_pubkey,
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_000_000),
            update_ix,
        ],
        &[],
        context.svm.latest_blockhash(),
    )?;

    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[
            context.default_payer.insecure_clone(),
            acting_authority.insecure_clone(),
        ],
    )?;

    context
        .svm
        .send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to update plugin: {:?}", e))?;
    Ok(())
}

/// Helper to get wallet account from account
pub fn get_wallet_account(account: &SolanaAccount) -> anyhow::Result<WalletAccount> {
    let data = &account.data;
    if data.is_empty() || data[0] != Discriminator::WalletAccount as u8 {
        return Err(anyhow::anyhow!("Invalid wallet account"));
    }

    // WalletAccount is Copy, so we can dereference
    let wallet_account_ref = unsafe { WalletAccount::load_unchecked(data)? };

    Ok(*wallet_account_ref)
}
