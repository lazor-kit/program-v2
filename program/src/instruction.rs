use pinocchio::program_error::ProgramError;

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
    /// 6. `[]` System Program
    RemoveAuthority,

    /// Transfer ownership (atomic swap of Owner role)
    ///
    /// Accounts:
    /// 1. `[signer]` Payer
    /// 2. `[]` Wallet PDA
    /// 3. `[writable]` Current Owner Authority PDA
    /// 4. `[writable]` New Owner Authority PDA
    /// 5. `[]` System Program
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

    InitializeConfig {
        wallet_fee: u64,
        action_fee: u64,
        num_shards: u8,
    },
    UpdateConfig,
    CloseSession,
    CloseWallet,
    SweepTreasury {
        shard_id: u8,
    },
    InitTreasuryShard {
        shard_id: u8,
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
            6 => Ok(Self::InitializeConfig {
                wallet_fee: 0,
                action_fee: 0,
                num_shards: 16,
            }), // Dummy unpack, actual args parsed in processor
            7 => Ok(Self::UpdateConfig),
            8 => Ok(Self::CloseSession),
            9 => Ok(Self::CloseWallet),
            10 => {
                let (&shard_id, _) = rest.split_first().unwrap();
                Ok(Self::SweepTreasury { shard_id })
            },
            11 => {
                let (&shard_id, _) = rest.split_first().unwrap();
                Ok(Self::InitTreasuryShard { shard_id })
            },
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
