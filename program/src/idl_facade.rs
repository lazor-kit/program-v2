// ==========================================
// Shank IDL Facade for Accounts, Types, Errors
// ==========================================
use borsh::{BorshSerialize, BorshDeserialize};
use shank::{ShankAccount, ShankType, ShankInstruction};

/// Shank IDL facade enum describing all program instructions and their required accounts.
/// This is used only for IDL generation and does not affect runtime behavior.
#[derive(ShankInstruction)]
pub enum ProgramIx {
    /// Create a new wallet
    #[account(
        0,
        signer,
        writable,
        name = "payer",
        desc = "Payer and rent contributor"
    )]
    #[account(1, writable, name = "wallet", desc = "Wallet PDA")]
    #[account(2, writable, name = "vault", desc = "Vault PDA")]
    #[account(3, writable, name = "authority", desc = "Initial owner authority PDA")]
    #[account(4, name = "system_program", desc = "System Program")]
    #[account(5, name = "rent", desc = "Rent Sysvar")]
    #[account(6, name = "config", desc = "Config PDA")]
    #[account(7, writable, name = "treasury_shard", desc = "Treasury Shard PDA")]
    CreateWallet {
        user_seed: [u8; 32],
        auth_type: u8,
        auth_bump: u8,
        padding: [u8; 6],
        payload: Vec<u8>,
    },

    /// Add a new authority to the wallet
    #[account(0, signer, writable, name = "payer", desc = "Transaction payer")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        name = "admin_authority",
        desc = "Admin authority PDA authorizing this action"
    )]
    #[account(
        3,
        writable,
        name = "new_authority",
        desc = "New authority PDA to be created"
    )]
    #[account(4, name = "system_program", desc = "System Program")]
    #[account(
        5,
        signer,
        optional,
        name = "authorizer_signer",
        desc = "Optional signer for Ed25519 authentication"
    )]
    #[account(6, name = "config", desc = "Config PDA")]
    #[account(7, writable, name = "treasury_shard", desc = "Treasury Shard PDA")]
    AddAuthority {
        new_type: u8,
        new_role: u8,
        padding: [u8; 6],
        payload: Vec<u8>,
    },

    /// Remove an authority from the wallet
    #[account(0, signer, writable, name = "payer", desc = "Transaction payer")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        name = "admin_authority",
        desc = "Admin authority PDA authorizing this action"
    )]
    #[account(
        3,
        writable,
        name = "target_authority",
        desc = "Authority PDA to be removed"
    )]
    #[account(
        4,
        writable,
        name = "refund_destination",
        desc = "Account to receive rent refund"
    )]
    #[account(5, name = "system_program", desc = "System Program")]
    #[account(
        6,
        signer,
        optional,
        name = "authorizer_signer",
        desc = "Optional signer for Ed25519 authentication"
    )]
    #[account(7, name = "config", desc = "Config PDA")]
    #[account(8, writable, name = "treasury_shard", desc = "Treasury Shard PDA")]
    RemoveAuthority,

    /// Transfer ownership (atomic swap of Owner role)
    #[account(0, signer, writable, name = "payer", desc = "Transaction payer")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        writable,
        name = "current_owner_authority",
        desc = "Current owner authority PDA"
    )]
    #[account(
        3,
        writable,
        name = "new_owner_authority",
        desc = "New owner authority PDA to be created"
    )]
    #[account(4, name = "system_program", desc = "System Program")]
    #[account(
        5,
        signer,
        optional,
        name = "authorizer_signer",
        desc = "Optional signer for Ed25519 authentication"
    )]
    #[account(6, name = "config", desc = "Config PDA")]
    #[account(7, writable, name = "treasury_shard", desc = "Treasury Shard PDA")]
    TransferOwnership {
        new_type: u8,
        payload: Vec<u8>,
    },

    /// Execute transactions
    #[account(0, signer, writable, name = "payer", desc = "Transaction payer")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        name = "authority",
        desc = "Authority or Session PDA authorizing execution"
    )]
    #[account(3, name = "vault", desc = "Vault PDA")]
    #[account(4, name = "config", desc = "Config PDA")]
    #[account(5, writable, name = "treasury_shard", desc = "Treasury Shard PDA")]
    #[account(6, name = "system_program", desc = "System Program")]
    #[account(
        7,
        optional,
        name = "sysvar_instructions",
        desc = "Sysvar Instructions (required for Secp256r1)"
    )]
    Execute { instructions: Vec<u8> },

    /// Create a new session key
    #[account(
        0,
        signer,
        writable,
        name = "payer",
        desc = "Transaction payer and rent contributor"
    )]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        name = "admin_authority",
        desc = "Admin/Owner authority PDA authorizing logic"
    )]
    #[account(3, writable, name = "session", desc = "New session PDA to be created")]
    #[account(4, name = "system_program", desc = "System Program")]
    #[account(5, name = "rent", desc = "Rent Sysvar")]
    #[account(6, name = "config", desc = "Config PDA")]
    #[account(7, writable, name = "treasury_shard", desc = "Treasury Shard PDA")]
    #[account(
        8,
        signer,
        optional,
        name = "authorizer_signer",
        desc = "Optional signer for Ed25519 authentication"
    )]
    CreateSession {
        session_key: [u8; 32],
        expires_at: i64,
    },

    /// Initialize global Config PDA
    #[account(0, signer, writable, name = "admin", desc = "Initial contract admin")]
    #[account(1, writable, name = "config", desc = "Config PDA")]
    #[account(2, name = "system_program", desc = "System Program")]
    #[account(3, name = "rent", desc = "Rent Sysvar")]
    InitializeConfig {
        wallet_fee: u64,
        action_fee: u64,
        num_shards: u8,
    },

    /// Update global Config PDA
    #[account(0, signer, name = "admin", desc = "Current contract admin")]
    #[account(1, writable, name = "config", desc = "Config PDA")]
    UpdateConfig, // args parsed raw

    /// Close an expired or active Session
    #[account(0, signer, writable, name = "payer", desc = "Receives rent refund")]
    #[account(1, name = "wallet", desc = "Session's parent wallet")]
    #[account(2, writable, name = "session", desc = "Target session")]
    #[account(3, name = "config", desc = "Config PDA for contract admin check")]
    #[account(4, optional, name = "authorizer", desc = "Wallet authority PDA")]
    #[account(
        5,
        signer,
        optional,
        name = "authorizer_signer",
        desc = "Ed25519 signer"
    )]
    #[account(6, optional, name = "sysvar_instructions", desc = "Secp256r1 sysvar")]
    CloseSession,

    /// Drain and close a Wallet PDA (Owner-only)
    #[account(0, signer, writable, name = "payer", desc = "Pays tx fee")]
    #[account(1, writable, name = "wallet", desc = "Wallet PDA to close")]
    #[account(2, writable, name = "vault", desc = "Vault PDA to drain")]
    #[account(3, name = "owner_authority", desc = "Owner Authority PDA")]
    #[account(4, writable, name = "destination", desc = "Receives all drained SOL")]
    #[account(5, signer, optional, name = "owner_signer", desc = "Ed25519 signer")]
    #[account(6, optional, name = "sysvar_instructions", desc = "Secp256r1 sysvar")]
    CloseWallet,

    /// Sweep funds from a treasury shard
    #[account(0, signer, name = "admin", desc = "Contract admin")]
    #[account(1, name = "config", desc = "Config PDA")]
    #[account(2, writable, name = "treasury_shard", desc = "Treasury Shard PDA")]
    #[account(3, writable, name = "destination", desc = "Receives swept funds")]
    SweepTreasury { shard_id: u8 },

    /// Initialize a new treasury shard
    #[account(0, signer, writable, name = "payer", desc = "Pays for rent exemption")]
    #[account(1, name = "config", desc = "Config PDA")]
    #[account(2, writable, name = "treasury_shard", desc = "Treasury Shard PDA")]
    #[account(3, name = "system_program", desc = "System Program")]
    #[account(4, name = "rent", desc = "Rent Sysvar")]
    InitTreasuryShard { shard_id: u8 },
}

// --- 1. Account Structs ---

#[derive(BorshSerialize, BorshDeserialize, ShankAccount)]
pub struct WalletAccount {
    pub discriminator: u8,
    pub bump: u8,
    pub version: u8,
    pub padding: [u8; 5],
}

#[derive(BorshSerialize, BorshDeserialize, ShankAccount)]
pub struct AuthorityAccount {
    pub discriminator: u8,
    pub authority_type: u8,
    pub role: u8,
    pub bump: u8,
    pub version: u8,
    pub padding: [u8; 3],
    pub counter: u64,
    pub wallet: [u8; 32],
}

#[derive(BorshSerialize, BorshDeserialize, ShankAccount)]
pub struct SessionAccount {
    pub discriminator: u8,
    pub bump: u8,
    pub version: u8,
    pub padding: [u8; 5],
    pub wallet: [u8; 32],
    pub session_key: [u8; 32],
    pub expires_at: u64,
}

// --- 2. Custom Types / Enums ---

#[derive(BorshSerialize, BorshDeserialize, ShankType)]
pub enum AuthorityType {
    Ed25519,
    Secp256r1,
}

#[derive(BorshSerialize, BorshDeserialize, ShankType)]
pub enum Role {
    Owner,
    Admin,
    Spender,
}

// --- 3. Custom Errors ---
use shank::ShankError;

#[derive(thiserror::Error, Debug, Copy, Clone, ShankError)]
pub enum LazorKitError {
    #[error("Invalid authority payload")]
    InvalidAuthorityPayload = 3001,
    #[error("Permission denied")]
    PermissionDenied = 3002,
    #[error("Invalid instruction")]
    InvalidInstruction = 3003,
    #[error("Invalid public key")]
    InvalidPubkey = 3004,
    #[error("Invalid message hash")]
    InvalidMessageHash = 3005,
    #[error("Signature has already been used")]
    SignatureReused = 3006,
    #[error("Invalid signature age")]
    InvalidSignatureAge = 3007,
    #[error("Invalid session duration")]
    InvalidSessionDuration = 3008,
    #[error("Session expired")]
    SessionExpired = 3009,
    #[error("Authority type does not support sessions")]
    AuthorityDoesNotSupportSession = 3010,
    #[error("Invalid authentication kind")]
    InvalidAuthenticationKind = 3011,
    #[error("Invalid message")]
    InvalidMessage = 3012,
    #[error("Self-reentrancy is not allowed")]
    SelfReentrancyNotAllowed = 3013,
}
