use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

/// Wraps external instructions into a format compatible with LazorKit's `Execute` instruction.
/// This matches `program/src/actions/execute.rs` which expects:
/// 1. Single Target Instruction (for now).
/// 2. Payload: [auth_index(u8), ...instruction_data].
/// 3. Accounts: [Config, Vault, System, TargetProgram, ...TargetAccounts, AuthSigner(optional)].
#[derive(Clone)]
pub struct ProxyBuilder {
    vault_pda: Pubkey,
    instruction: Option<Instruction>,
    signer: Option<Pubkey>,
}

impl ProxyBuilder {
    pub fn new(vault_pda: Pubkey) -> Self {
        Self {
            vault_pda,
            instruction: None,
            signer: None,
        }
    }

    pub fn add_instruction(mut self, ix: Instruction) -> Self {
        // Currently only supports one instruction due to program implementation
        self.instruction = Some(ix);
        self
    }

    pub fn with_signer(mut self, signer: Pubkey) -> Self {
        self.signer = Some(signer);
        self
    }

    /// Build the payload and account_metas for the LazorKit Execute instruction
    /// Returns (instruction_data, account_metas, auth_idx_relative_to_returned_accounts).
    pub fn build(self) -> (Vec<u8>, Vec<AccountMeta>, u8) {
        let ix = self.instruction.expect("No instruction provided");

        // 1. Construct Accounts List
        // Order: [TargetProgram, ...TargetAccounts, Signer?]
        // Config is added by caller (index 0).
        let mut accounts = Vec::new();
        // Target Program (Proxy Index 0, Absolute Index 3)
        accounts.push(AccountMeta::new_readonly(ix.program_id, false));

        // Target Accounts (Proxy Index 1+, Absolute Index 4+)
        for mut meta in ix.accounts {
            if meta.pubkey == self.vault_pda {
                meta.is_signer = false;
            }
            accounts.push(meta);
        }

        // Signer (Index N)
        let auth_idx: u8;

        if let Some(signer) = self.signer {
            accounts.push(AccountMeta::new_readonly(signer, true));
            // Calculate absolute index
            // Config(0) + accounts.len() - 1 (last element)
            auth_idx = (accounts.len() - 1) as u8; // accounts is [Vault, ... Signer]. len includes Signer.
                                                   // Config is index 0. Vault is 1.
                                                   // So Signer index is accounts.len().
                                                   // Example: [Vault]. len=1. Real indices: 0:Config, 1:Vault.
                                                   // So AuthIdx = len. Correct.
        } else {
            // If no signer provided, pass 0?
            // Or maybe we don't need auth (e.g. ProgramExec/None).
            // We'll leave it 0.
            auth_idx = 0;
        }

        (ix.data, accounts, auth_idx)
    }
}
