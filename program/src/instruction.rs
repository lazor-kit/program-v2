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
    /// ... Inner accounts
    Execute {
        instructions: Vec<u8>, // CompactInstructions bytes, we'll parse later
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
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}
