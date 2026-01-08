//! Common test utilities for Lazorkit V2 tests

use solana_sdk::{
    account::Account as SolanaAccount,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    native_token::LAMPORTS_PER_SOL,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    sysvar::{clock::Clock, rent::Rent},
    transaction::{TransactionError, VersionedTransaction},
};
use std::str::FromStr;
use litesvm::LiteSVM;
use lazorkit_v2_state::{
    wallet_account::{WalletAccount, wallet_account_seeds_with_bump, wallet_vault_seeds_with_bump},
    wallet_authority::{WalletAuthority, wallet_authority_seeds_with_bump},
    plugin::PluginEntry,
    authority::AuthorityType,
    Discriminator,
    Transmutable,
    IntoBytes,
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
        
        // Airdrop to default payer
        // Convert solana_program::Pubkey to solana_sdk::Pubkey
        let payer_program_pubkey = default_payer.pubkey();
        let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
            .map_err(|_| anyhow::anyhow!("Failed to convert Pubkey"))?;
        svm.airdrop(&payer_pubkey, 10_000_000_000)
            .map_err(|e| anyhow::anyhow!("Failed to airdrop: {:?}", e))?;
        
        Ok(Self {
            svm,
            default_payer,
        })
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
    let pinocchio_id: pinocchio::pubkey::Pubkey = pinocchio_pubkey!("Gsuz7YcA5sbMGVRXT3xSYhJBessW4xFC4xYsihNCqMFh");
    // Convert directly from bytes
    Pubkey::try_from(pinocchio_id.as_ref())
        .expect("Invalid program ID")
}

/// Load Lazorkit V2 program into SVM
pub fn load_lazorkit_program(svm: &mut LiteSVM) -> anyhow::Result<()> {
    // Try to load from deploy directory
    let program_path = "../target/deploy/lazorkit_v2.so";
    let program_id = lazorkit_program_id();
    svm.add_program_from_file(program_id, program_path)
        .map_err(|e| anyhow::anyhow!("Failed to load Lazorkit V2 program from {}: {:?}. Build it first with: cargo build-sbf --manifest-path program/Cargo.toml", program_path, e))
}

/// Load plugin program into SVM
pub fn load_plugin_program(svm: &mut LiteSVM, program_id: Pubkey, program_path: &str) -> anyhow::Result<()> {
    svm.add_program_from_file(program_id, program_path)
        .map_err(|e| anyhow::anyhow!("Failed to load plugin program from {}: {:?}", program_path, e))
}

/// Helper to create a wallet account PDA seeds as slice
pub fn wallet_account_seeds(id: &[u8; 32]) -> [&[u8]; 2] {
    [
        b"wallet_account",
        id,
    ]
}

/// Helper to create a wallet vault PDA seeds as slice
pub fn wallet_vault_seeds(wallet_account: &Pubkey) -> [&[u8]; 2] {
    [
        b"wallet_vault",
        wallet_account.as_ref(),
    ]
}

// Removed smart_wallet_seeds - no longer needed in Pure External architecture

/// Helper to create a wallet authority PDA seeds as slice
pub fn wallet_authority_seeds<'a>(smart_wallet: &'a Pubkey, authority_hash: &'a [u8; 32]) -> [&'a [u8]; 3] {
    [
        b"wallet_authority",
        smart_wallet.as_ref(),
        authority_hash,
    ]
}

/// Helper to create a plugin config PDA seeds as slice
pub fn plugin_config_seeds<'a>(wallet_account: &'a Pubkey, plugin_seed: &'a [u8]) -> [&'a [u8]; 2] {
    [
        plugin_seed,
        wallet_account.as_ref(),
    ]
}

/// Create a Lazorkit V2 wallet (Pure External architecture)
/// Returns (wallet_account, wallet_vault)
pub fn create_lazorkit_wallet(
    context: &mut TestContext,
    id: [u8; 32],
) -> anyhow::Result<(Pubkey, Pubkey)> {
    // Convert solana_program::Pubkey to solana_sdk::Pubkey
    let payer_program_pubkey = context.default_payer.pubkey();
    let payer_pubkey = Pubkey::try_from(payer_program_pubkey.as_ref())
        .map_err(|_| anyhow::anyhow!("Failed to convert Pubkey"))?;
    
    // Derive PDAs
    let seeds = wallet_account_seeds(&id);
    let (wallet_account, wallet_account_bump) = Pubkey::find_program_address(
        &seeds,
        &lazorkit_program_id(),
    );
    
    let vault_seeds = wallet_vault_seeds(&wallet_account);
    let (wallet_vault, wallet_vault_bump) = Pubkey::find_program_address(
        &vault_seeds,
        &solana_sdk::system_program::id(),
    );
    
    // Build CreateSmartWallet instruction
    // Instruction format: [instruction: u16, id: [u8; 32], bump: u8, wallet_bump: u8, padding: [u8; 6]]
    // CreateSmartWalletArgs layout (after skipping instruction): id (32) + bump (1) + wallet_bump (1) + padding (6) = 40 bytes (aligned to 8)
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(0u16).to_le_bytes()); // CreateSmartWallet = 0 (2 bytes)
    instruction_data.extend_from_slice(&id); // id (32 bytes)
    instruction_data.push(wallet_account_bump); // bump (1 byte)
    instruction_data.push(wallet_vault_bump); // wallet_bump (1 byte)
    instruction_data.extend_from_slice(&[0u8; 6]); // Padding to align struct to 8 bytes (6 bytes to make total 40)
    // Total: 2 + 32 + 1 + 1 + 6 = 42 bytes (args part is 40 bytes)
    
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
        &[ComputeBudgetInstruction::set_compute_unit_limit(1_000_000), create_ix],
        &[],
        context.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone()],
    )?;
    
    let result = context.svm.send_transaction(tx);
    
    match result {
        Ok(_) => Ok((wallet_account, wallet_vault)),
        Err(e) => Err(anyhow::anyhow!("Failed to create wallet: {:?}", e)),
    }
}

/// Add Ed25519 authority to wallet
pub fn add_authority_ed25519(
    context: &mut TestContext,
    wallet_state: &Pubkey,
    smart_wallet: &Pubkey,
    acting_authority: &Keypair,
    new_authority: &Keypair,
) -> anyhow::Result<Pubkey> {
    // Derive new authority PDA
    let authority_hash = {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&new_authority.pubkey().to_bytes());
        hash
    };
    
    let seeds = wallet_authority_seeds(smart_wallet, &authority_hash);
    let (new_wallet_authority, authority_bump) = Pubkey::find_program_address(
        &seeds,
        &lazorkit_program_id(),
    );
    
    // Build AddAuthority instruction
    // Instruction format: [instruction: u16, new_authority_type: u16, new_authority_data_len: u16, 
    //                      acting_authority_index: u16, wallet_authority_bump: u8, padding: u8,
    //                      authority_data, authority_payload]
    let authority_data = new_authority.pubkey().to_bytes();
    let authority_data_len = authority_data.len() as u16;
    
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // AddAuthority = 2
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Ed25519 = 1
    instruction_data.extend_from_slice(&authority_data_len.to_le_bytes());
    instruction_data.extend_from_slice(&(4u16).to_le_bytes()); // acting_authority_index
    instruction_data.push(authority_bump);
    instruction_data.push(0); // padding
    instruction_data.extend_from_slice(&authority_data);
    
    // For Ed25519, authority_payload format: [authority_index: u8]
    // The signature is verified by checking if the authority account is a signer
    // In tests, we pass the authority as a signer, so payload is just [4] (index of acting_authority)
    let authority_payload = vec![4u8]; // Index of acting authority in accounts
    instruction_data.extend_from_slice(&authority_payload);
    
    let add_authority_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_state, false),
            AccountMeta::new(context.default_payer.pubkey(), true),
            AccountMeta::new_readonly(*smart_wallet, true),
            AccountMeta::new(new_wallet_authority, false),
            AccountMeta::new_readonly(acting_authority.pubkey(), true), // Must be signer for Ed25519
            AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
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
        &[ComputeBudgetInstruction::set_compute_unit_limit(1_000_000), add_authority_ix],
        &[],
        context.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone(), acting_authority.insecure_clone()],
    )?;
    
    context.svm.send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to add authority: {:?}", e))?;
    
    Ok(new_wallet_authority)
}

/// Add plugin to wallet
pub fn add_plugin(
    context: &mut TestContext,
    wallet_state: &Pubkey,
    smart_wallet: &Pubkey,
    acting_authority: &Keypair,
    plugin_program_id: Pubkey,
    plugin_config: Pubkey,
) -> anyhow::Result<()> {
    // Build AddPlugin instruction
    // Instruction format: [instruction: u16, acting_authority_index: u16, program_id: Pubkey, 
    //                      config_account: Pubkey, enabled: u8, priority: u8, padding: [u8; 2],
    //                      authority_payload]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(3u16).to_le_bytes()); // AddPlugin = 3
    instruction_data.extend_from_slice(&(3u16).to_le_bytes()); // acting_authority_index
    instruction_data.extend_from_slice(plugin_program_id.as_ref());
    instruction_data.extend_from_slice(plugin_config.as_ref());
    instruction_data.push(1); // enabled
    instruction_data.push(0); // priority
    instruction_data.extend_from_slice(&[0u8; 2]); // padding
    
    // For Ed25519, authority_payload format: [authority_index: u8]
    // The signature is verified by checking if the authority account is a signer
    let authority_payload = vec![3u8]; // Index of acting authority in accounts
    instruction_data.extend_from_slice(&authority_payload);
    
    let add_plugin_ix = Instruction {
        program_id: lazorkit_program_id(),
        accounts: vec![
            AccountMeta::new(*wallet_state, false),
            AccountMeta::new(context.default_payer.pubkey(), true),
            AccountMeta::new_readonly(*smart_wallet, true),
            AccountMeta::new_readonly(acting_authority.pubkey(), true), // Must be signer for Ed25519
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
        &[ComputeBudgetInstruction::set_compute_unit_limit(1_000_000), add_plugin_ix],
        &[],
        context.svm.latest_blockhash(),
    )?;
    
    let tx = VersionedTransaction::try_new(
        VersionedMessage::V0(message),
        &[context.default_payer.insecure_clone(), acting_authority.insecure_clone()],
    )?;
    
    context.svm.send_transaction(tx)
        .map_err(|e| anyhow::anyhow!("Failed to add plugin: {:?}", e))?;
    
    Ok(())
}

/// Create Sign instruction with Ed25519 authority
pub fn create_sign_instruction_ed25519(
    wallet_state: &Pubkey,
    smart_wallet: &Pubkey,
    authority: &Keypair,
    inner_instruction: Instruction,
) -> anyhow::Result<Instruction> {
    // Compact inner instruction
    // For now, we'll use a simple approach - in production, use compact_instructions
    // Format: [num_instructions: u8, for each: [program_id_index: u8, num_accounts: u8, account_indices..., data_len: u16, data...]]
    let mut compacted = Vec::new();
    compacted.push(1u8); // num_instructions
    compacted.push(3u8); // program_id_index (assume first account after wallet_state, smart_wallet, authority)
    compacted.push(inner_instruction.accounts.len() as u8); // num_accounts
    for (i, _) in inner_instruction.accounts.iter().enumerate() {
        compacted.push(i as u8 + 4); // Account indices (offset by wallet_state, smart_wallet, authority, program_id)
    }
    compacted.extend_from_slice(&(inner_instruction.data.len() as u16).to_le_bytes());
    compacted.extend_from_slice(&inner_instruction.data);
    
    // Build Sign instruction data
    // Format: [instruction: u16, instruction_payload_len: u16, authority_index: u16, 
    //          instruction_payload, authority_payload]
    let mut instruction_data = Vec::new();
    instruction_data.extend_from_slice(&(1u16).to_le_bytes()); // Sign = 1
    instruction_data.extend_from_slice(&(compacted.len() as u16).to_le_bytes());
    instruction_data.extend_from_slice(&(2u16).to_le_bytes()); // authority_index
    instruction_data.extend_from_slice(&compacted);
    
    // For Ed25519, authority_payload format: [authority_index: u8]
    // The signature is verified by checking if the authority account is a signer
    let authority_payload = vec![2u8]; // Index of authority in accounts
    instruction_data.extend_from_slice(&authority_payload);
    
    // Build accounts list
    let mut accounts = vec![
        AccountMeta::new(*wallet_state, false),
        AccountMeta::new_readonly(*smart_wallet, true),
        AccountMeta::new_readonly(authority.pubkey(), true), // Must be signer for Ed25519
    ];
    
    // Add inner instruction accounts
    accounts.push(AccountMeta::new_readonly(inner_instruction.program_id, false));
    for account_meta in inner_instruction.accounts {
        accounts.push(account_meta);
    }
    
    Ok(Instruction {
        program_id: lazorkit_program_id(),
        accounts,
        data: instruction_data,
    })
}

/// Helper to get wallet account from account
pub fn get_wallet_account(account: &SolanaAccount) -> anyhow::Result<WalletAccount> {
    let data = &account.data;
    if data.is_empty() || data[0] != Discriminator::WalletAccount as u8 {
        return Err(anyhow::anyhow!("Invalid wallet account"));
    }
    
    // WalletAccount is Copy, so we can dereference
    let wallet_account_ref = unsafe {
        WalletAccount::load_unchecked(data)?
    };
    
    Ok(*wallet_account_ref)
}
