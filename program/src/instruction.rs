use pinocchio::program_error::ProgramError;
use shank::ShankInstruction;

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
    #[account(5, name = "rent_sysvar", desc = "Rent Sysvar")]
    CreateWallet {
        user_seed: Vec<u8>,
        auth_type: u8,
        auth_pubkey: [u8; 33],
        credential_hash: [u8; 32],
    },

    /// Add a new authority to the wallet
    #[account(0, signer, writable, name = "payer", desc = "Payer and rent contributor")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        writable,
        name = "admin_authority",
        desc = "Admin authority PDA authorizing this action (counter incremented)"
    )]
    #[account(
        3,
        writable,
        name = "new_authority",
        desc = "New authority PDA to be created"
    )]
    #[account(4, name = "system_program", desc = "System Program")]
    #[account(5, name = "rent_sysvar", desc = "Rent Sysvar")]
    #[account(
        6,
        signer,
        optional,
        name = "authorizer_signer",
        desc = "Optional signer for Ed25519 authentication"
    )]
    AddAuthority {
        new_type: u8,
        new_pubkey: [u8; 33],
        new_hash: [u8; 32],
        new_role: u8,
    },

    /// Remove an authority from the wallet
    #[account(0, signer, writable, name = "payer", desc = "Transaction payer")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        writable,
        name = "admin_authority",
        desc = "Admin authority PDA authorizing this action (counter incremented)"
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
    #[account(
        5,
        optional,
        name = "auth_extra",
        desc = "Ed25519: signer keypair | Secp256r1: sysvar_instructions"
    )]
    RemoveAuthority,

    /// Transfer ownership (atomic swap of Owner role)
    #[account(0, signer, writable, name = "payer", desc = "Payer and rent contributor")]
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
    #[account(
        4,
        writable,
        name = "refund_destination",
        desc = "Account to receive rent refund from closed current owner"
    )]
    #[account(5, name = "system_program", desc = "System Program")]
    #[account(6, name = "rent_sysvar", desc = "Rent Sysvar")]
    #[account(
        7,
        signer,
        optional,
        name = "authorizer_signer",
        desc = "Optional signer for Ed25519 authentication"
    )]
    TransferOwnership {
        new_type: u8,
        new_pubkey: [u8; 33],
        new_hash: [u8; 32],
    },

    /// Execute transactions
    #[account(0, signer, writable, name = "payer", desc = "Transaction payer")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        writable,
        name = "authority",
        desc = "Authority or Session PDA authorizing execution (counter incremented)"
    )]
    #[account(3, writable, name = "vault", desc = "Vault PDA (signer for CPI, lamports debited)")]
    #[account(
        4,
        optional,
        name = "sysvar_instructions",
        desc = "Sysvar Instructions (required for Secp256r1)"
    )]
    Execute { instructions: Vec<u8> },

    /// Create a new session key
    #[account(
        0,
        signer,
        name = "payer",
        desc = "Transaction payer and rent contributor"
    )]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        writable,
        name = "admin_authority",
        desc = "Admin/Owner authority PDA authorizing logic (counter incremented)"
    )]
    #[account(3, writable, name = "session", desc = "New session PDA to be created")]
    #[account(4, name = "system_program", desc = "System Program")]
    #[account(5, name = "rent_sysvar", desc = "Rent Sysvar")]
    #[account(
        6,
        signer,
        optional,
        name = "authorizer_signer",
        desc = "Optional signer for Ed25519 authentication"
    )]
    CreateSession {
        session_key: [u8; 32],
        expires_at: i64,
    },

    /// Authorize deferred execution (TX1 of 2-transaction flow)
    ///
    /// Verifies Secp256r1 signature over instruction/account hashes, then creates
    /// a DeferredExec PDA storing the authorization for later execution.
    #[account(
        0,
        signer,
        writable,
        name = "payer",
        desc = "Payer and rent contributor"
    )]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        writable,
        name = "authority",
        desc = "Authority PDA (counter incremented)"
    )]
    #[account(
        3,
        writable,
        name = "deferred_exec",
        desc = "DeferredExec PDA to be created"
    )]
    #[account(4, name = "system_program", desc = "System Program")]
    #[account(5, name = "rent_sysvar", desc = "Rent Sysvar")]
    #[account(
        6,
        name = "sysvar_instructions",
        desc = "Sysvar Instructions (for Secp256r1 precompile introspection)"
    )]
    Authorize {
        instructions_hash: [u8; 32],
        accounts_hash: [u8; 32],
        expiry_offset: u16,
    },

    /// Execute a previously authorized deferred execution (TX2 of 2-transaction flow)
    ///
    /// Verifies compact instructions against stored hashes, executes via CPI
    /// with vault PDA signing, then closes the DeferredExec account.
    #[account(0, signer, writable, name = "payer", desc = "Transaction payer")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(2, writable, name = "vault", desc = "Vault PDA (signer for CPI)")]
    #[account(
        3,
        writable,
        name = "deferred_exec",
        desc = "DeferredExec PDA (read and closed)"
    )]
    #[account(
        4,
        writable,
        name = "refund_destination",
        desc = "Account to receive rent refund from closed DeferredExec"
    )]
    ExecuteDeferred { instructions: Vec<u8> },

    /// Reclaim an expired DeferredExec account and refund rent
    ///
    /// Only the original payer can reclaim, and only after the authorization has expired.
    #[account(
        0,
        signer,
        name = "payer",
        desc = "Original payer (must match stored payer)"
    )]
    #[account(
        1,
        writable,
        name = "deferred_exec",
        desc = "Expired DeferredExec PDA to close"
    )]
    #[account(
        2,
        writable,
        name = "refund_destination",
        desc = "Account to receive rent refund"
    )]
    ReclaimDeferred,

    /// Revoke a session key early (before expiry)
    ///
    /// Only Owner or Admin can revoke. Closes the session account and refunds rent.
    #[account(0, signer, writable, name = "payer", desc = "Transaction payer")]
    #[account(1, name = "wallet", desc = "Wallet PDA")]
    #[account(
        2,
        writable,
        name = "admin_authority",
        desc = "Owner/Admin authority PDA (counter incremented for Secp256r1)"
    )]
    #[account(3, writable, name = "session", desc = "Session PDA to revoke")]
    #[account(
        4,
        writable,
        name = "refund_destination",
        desc = "Account to receive rent refund"
    )]
    #[account(
        5,
        optional,
        name = "auth_extra",
        desc = "Ed25519: signer keypair | Secp256r1: sysvar_instructions"
    )]
    RevokeSession,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq)]
pub enum LazorKitInstruction {
    /// Create a new wallet
    ///
    /// Accounts:
    /// 1. `[signer, writable]` Payer
    /// 2. `[writable]` Wallet PDA
    /// 3. `[writable]` Vault PDA
    /// 4. `[writable]` Authority PDA
    /// 5. `[]` System Program
    CreateWallet {
        user_seed: Vec<u8>,
        auth_type: u8,
        auth_pubkey: [u8; 33],
        credential_hash: [u8; 32],
    },

    /// Add a new authority to the wallet
    ///
    /// Accounts:
    /// 1. `[signer]` Payer
    /// 2. `[]` Wallet PDA
    /// 3. `[signer]` Admin Authority PDA (The one authorizing this action)
    /// 4. `[writable]` New Authority PDA
    /// 5. `[]` System Program
    AddAuthority {
        new_type: u8,
        new_pubkey: [u8; 33],
        new_hash: [u8; 32],
        new_role: u8,
    },

    /// Remove an authority from the wallet
    ///
    /// Accounts:
    /// 1. `[signer]` Payer
    /// 2. `[]` Wallet PDA
    /// 3. `[signer]` Admin Authority PDA
    /// 4. `[writable]` Target Authority PDA
    /// 5. `[writable]` Refund Destination
    RemoveAuthority,

    /// Transfer ownership (atomic swap of Owner role)
    ///
    /// Accounts:
    /// 1. `[signer]` Payer
    /// 2. `[]` Wallet PDA
    /// 3. `[writable]` Current Owner Authority PDA
    /// 4. `[writable]` New Owner Authority PDA
    /// 5. `[writable]` Refund Destination
    /// 6. `[]` System Program
    /// 7. `[]` Rent Sysvar
    TransferOwnership {
        new_type: u8,
        new_pubkey: [u8; 33],
        new_hash: [u8; 32],
    },

    /// Execute transactions
    ///
    /// Accounts:
    /// 1. `[signer]` Payer
    /// 2. `[]` Wallet PDA
    /// 3. `[]` Authority PDA
    /// 4. `[signer]` Vault PDA
    /// 5. `[]` Sysvar Instructions (if Secp256r1)
    ///    ... Inner accounts
    Execute {
        instructions: Vec<u8>, // CompactInstructions bytes, we'll parse later
    },

    /// Create a new session key
    ///
    /// Accounts:
    /// 1. `[signer]` Payer
    /// 2. `[]` Wallet PDA
    /// 3. `[signer]` Authority PDA (Authorizer)
    /// 4. `[writable]` Session PDA
    /// 5. `[]` System Program
    CreateSession {
        session_key: [u8; 32],
        expires_at: u64,
    },
}

impl LazorKitInstruction {
    pub fn unpack(input: &[u8]) -> Result<Self, ProgramError> {
        let (&tag, rest) = input
            .split_first()
            .ok_or(ProgramError::InvalidInstructionData)?;

        match tag {
            0 => {
                // CreateWallet
                // Format: [user_seed_len(4)][user_seed][auth_type(1)][auth_pubkey(33)][credential_hash(32)]

                if rest.len() < 4 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let (len_bytes, rest) = rest.split_at(4);
                let seed_len = u32::from_le_bytes(len_bytes.try_into().unwrap()) as usize;

                if rest.len() < seed_len + 1 + 33 + 32 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let (user_seed, rest) = rest.split_at(seed_len);
                let (&auth_type, rest) = rest.split_first().unwrap();
                let (auth_pubkey, rest) = rest.split_at(33);
                let (credential_hash, _) = rest.split_at(32);

                Ok(Self::CreateWallet {
                    user_seed: user_seed.to_vec(),
                    auth_type,
                    auth_pubkey: auth_pubkey.try_into().unwrap(),
                    credential_hash: credential_hash.try_into().unwrap(),
                })
            },
            1 => {
                // AddAuthority
                // Format: [new_type(1)][new_pubkey(33)][new_hash(32)][new_role(1)]
                if rest.len() < 1 + 33 + 32 + 1 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let (&new_type, rest) = rest.split_first().unwrap();
                let (new_pubkey, rest) = rest.split_at(33);
                let (new_hash, rest) = rest.split_at(32);
                let (&new_role, _) = rest.split_first().unwrap();

                Ok(Self::AddAuthority {
                    new_type,
                    new_pubkey: new_pubkey.try_into().unwrap(),
                    new_hash: new_hash.try_into().unwrap(),
                    new_role,
                })
            },
            2 => Ok(Self::RemoveAuthority),
            3 => {
                // Format: [new_type(1)][new_pubkey(33)][new_hash(32)]
                if rest.len() < 1 + 33 + 32 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let (&new_type, rest) = rest.split_first().unwrap();
                let (new_pubkey, rest) = rest.split_at(33);
                let (new_hash, _) = rest.split_at(32);

                Ok(Self::TransferOwnership {
                    new_type,
                    new_pubkey: new_pubkey.try_into().unwrap(),
                    new_hash: new_hash.try_into().unwrap(),
                })
            },
            4 => {
                // Execute
                // Remaining bytes are compact instructions
                Ok(Self::Execute {
                    instructions: rest.to_vec(),
                })
            },
            5 => {
                // CreateSession
                // Format: [session_key(32)][expires_at(8)]
                if rest.len() < 32 + 8 {
                    return Err(ProgramError::InvalidInstructionData);
                }
                let (session_key, rest) = rest.split_at(32);
                let (expires_at_bytes, _) = rest.split_at(8);
                let expires_at = u64::from_le_bytes(expires_at_bytes.try_into().unwrap());

                Ok(Self::CreateSession {
                    session_key: session_key.try_into().unwrap(),
                    expires_at,
                })
            },
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
