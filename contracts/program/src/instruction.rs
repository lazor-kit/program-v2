//! LazorKit Instruction Definitions
//!
//! Matches architecture spec v2.1.0

use borsh::{BorshDeserialize, BorshSerialize};
use pinocchio::program_error::ProgramError;

/// Instruction discriminators (matching docs/ARCHITECTURE.md)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InstructionDiscriminator {
    CreateWallet = 0,
    AddAuthority = 1,
    RemoveAuthority = 2,
    UpdateAuthority = 3,
    CreateSession = 4,
    Execute = 5,
    TransferOwnership = 6,
    RegisterPolicy = 7,
    DeactivatePolicy = 8,
}

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum LazorKitInstruction {
    /// Create a new LazorKit wallet
    ///
    /// Accounts:
    /// 0. `[writable]` LazorKit Config account (PDA: ["lazorkit", id])
    /// 1. `[writable, signer]` Payer
    /// 2. `[writable]` WalletAddress (Vault PDA: ["lazorkit-wallet-address", config_key])
    /// 3. `[]` System program
    CreateWallet {
        /// Unique wallet ID (32 bytes)
        id: [u8; 32],
        /// PDA bump seed for Config
        bump: u8,
        /// PDA bump seed for Vault
        wallet_bump: u8,
        /// Owner authority type (1-8)
        owner_authority_type: u16,
        /// Owner authority data (pubkey or key data)
        owner_authority_data: Vec<u8>,
    },

    /// Add a new authority (role) to the wallet
    ///
    /// Accounts:
    /// 0. `[writable, signer]` LazorKit Config account
    /// 1. `[writable, signer]` Payer
    /// 2. `[]` System program
    AddAuthority {
        /// Acting role ID (caller must have ManageAuthority permission)
        acting_role_id: u32,
        /// New authority type (1-8)
        authority_type: u16,
        /// New authority data
        authority_data: Vec<u8>,
        /// Serialized policy configs (PolicyHeader + State blobs)
        /// Format: [PolicyHeader (40 bytes)][State Data]...
        policies_config: Vec<u8>,
        /// Authorization signature data
        authorization_data: Vec<u8>,
    },

    /// Remove an authority from the wallet
    ///
    /// Accounts:
    /// 0. `[writable, signer]` LazorKit Config account
    /// 1. `[writable, signer]` Payer
    /// 2. `[]` System program
    RemoveAuthority {
        /// Acting role ID (caller)
        acting_role_id: u32,
        /// Role ID to remove
        target_role_id: u32,
    },

    /// Update an authority's plugins
    ///
    /// Accounts:
    /// 0. `[writable, signer]` LazorKit Config account
    /// 1. `[writable, signer]` Payer
    /// 2. `[]` System program
    UpdateAuthority {
        /// Acting role ID
        acting_role_id: u32,
        /// Role ID to update
        target_role_id: u32,
        /// Operation: 0=ReplaceAll, 1=AddPlugins, 2=RemoveByType, 3=RemoveByIndex
        operation: u8,
        /// Payload (new plugins or indices to remove)
        payload: Vec<u8>,
    },

    /// Create a session key for an authority
    ///
    /// Accounts:
    /// 0. `[writable, signer]` LazorKit Config account
    /// 1. `[signer]` Payer (must be the role owner)
    /// 2. `[]` System program
    CreateSession {
        /// Role ID to create session for
        role_id: u32,
        /// New session public key (Ed25519)
        session_key: [u8; 32],
        /// Duration in slots
        duration: u64,
        /// Authorization signature data (needed for non-native authorities)
        authorization_data: Vec<u8>,
    },

    /// Execute a transaction (Bounce Flow)
    ///
    /// Accounts:
    /// 0. `[writable]` LazorKit Config account
    /// 1. `[writable, signer]` WalletAddress (Vault - PDA signer)
    /// 2. `[]` System program
    /// 3+ `[]` Plugin programs and target accounts (dynamic)
    Execute {
        /// Role ID executing this operation
        role_id: u32,
        /// Serialized instruction payload to execute
        instruction_payload: Vec<u8>,
    },

    /// Transfer ownership to a new owner
    ///
    /// Accounts:
    /// 0. `[writable, signer]` LazorKit Config account
    /// 1. `[signer]` Current owner (Role 0)
    TransferOwnership {
        /// New owner authority type
        new_owner_authority_type: u16,
        /// New owner authority data
        new_owner_authority_data: Vec<u8>,
    },

    /// Register a verified policy in the system
    ///
    /// Accounts:
    /// 0. `[writable]` Registry Entry PDA: ["policy-registry", program_id]
    /// 1. `[writable, signer]` Payer/Admin
    /// 2. `[]` System Program
    RegisterPolicy {
        /// The program ID of the policy to register
        policy_program_id: [u8; 32],
    },

    /// Deactivate a policy in the registry
    ///
    /// Accounts:
    /// 0. `[writable]` Registry Entry PDA: ["policy-registry", program_id]
    /// 1. `[writable, signer]` Payer/Admin
    DeactivatePolicy {
        /// The program ID of the policy to deactivate
        policy_program_id: [u8; 32],
    },
}

impl LazorKitInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        Self::try_from_slice(input).map_err(|_| ProgramError::InvalidInstructionData)
    }
}

/// Authority update operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UpdateOperation {
    /// Replace all policies
    ReplaceAll = 0,
    /// Add policies to end
    AddPolicies = 1,
    /// Remove policies by program ID
    RemoveByType = 2,
    /// Remove policies by index
    RemoveByIndex = 3,
}

impl TryFrom<u8> for UpdateOperation {
    type Error = ProgramError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(UpdateOperation::ReplaceAll),
            1 => Ok(UpdateOperation::AddPolicies),
            2 => Ok(UpdateOperation::RemoveByType),
            3 => Ok(UpdateOperation::RemoveByIndex),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
