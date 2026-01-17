use crate::basic::actions::{AddAuthorityBuilder, CreateWalletBuilder, ExecuteBuilder};
use crate::core::connection::SolConnection;
use solana_sdk::pubkey::Pubkey;

/// Represents a LazorKit Smart Wallet on-chain.
pub struct LazorWallet {
    pub address: Pubkey,
    pub program_id: Pubkey,
    pub config_pda: Pubkey,
    pub config_bump: u8,
}

impl LazorWallet {
    pub const DEFAULT_PROGRAM_ID: Pubkey =
        solana_sdk::pubkey!("LazorKit11111111111111111111111111111111111");

    /// Connect to an existing wallet by its vault address (The user-facing "Wallet Address").
    /// Requires fetching on-chain data to verify and find the config PDA.
    pub async fn connect(
        _connection: &impl SolConnection,
        wallet_address: Pubkey,
    ) -> Result<Self, String> {
        // In a real impl, we would fetch the vault account, check the owner/seeds to find the Config.
        // For now, let's assume standard derivation from a known Config ID ?
        // Wait, standard derivation is: Config -> [seeds] -> address.
        // Reverse lookup from Address -> Config is hard unless we know the Config ID (which is the wallet ID).
        // If the user provides the Vault Address, we might need to scan or ask indexer.
        // "Connect" in SDK usually takes the "address" everyone executes transactions against.
        // In LazorKit, `Execute` takes `Vault` as signer but `Config` as the state.

        // Simplification for v1: We assume the user provides the Wallet ID (config seed) or we have a way to derive.
        // OR we just take the Config Address?
        // Let's assume input is Config Address for now to be safe, or we document that `address` is the Config PDA.
        // Actually, creating a wallet yields a Config PDA and a Vault PDA.
        // Users transfer SOL to Vault PDA.
        // Users send Instructions to Config PDA (Execute).

        // Let's store the Config PDA as the primary identity.
        Ok(Self {
            address: wallet_address, // Assuming this is the Vault Address for funds
            program_id: Self::DEFAULT_PROGRAM_ID,
            config_pda: Pubkey::default(), // TODO: Derived or fetched
            config_bump: 0,
        })
    }

    /// Create a new wallet "Factory" entry point
    pub fn create() -> CreateWalletBuilder {
        CreateWalletBuilder::new()
    }

    /// Start building an AddAuthority transaction
    pub fn add_authority(&self) -> AddAuthorityBuilder<'_> {
        AddAuthorityBuilder::new(self)
    }

    pub fn new(program_id: Pubkey, config_pda: Pubkey, address: Pubkey) -> Self {
        Self {
            address,
            program_id,
            config_pda,
            config_bump: 0,
        }
    }

    /// Helper for "Execute" / "Proxy" flow
    pub fn proxy(&self) -> ExecuteBuilder<'_> {
        ExecuteBuilder::new(self)
    }

    pub fn remove_authority(&self) -> crate::basic::actions::RemoveAuthorityBuilder<'_> {
        crate::basic::actions::RemoveAuthorityBuilder::new(self)
    }

    pub fn update_authority(&self) -> crate::basic::actions::UpdateAuthorityBuilder<'_> {
        crate::basic::actions::UpdateAuthorityBuilder::new(self)
    }

    pub fn create_session(&self) -> crate::basic::actions::CreateSessionBuilder<'_> {
        crate::basic::actions::CreateSessionBuilder::new(self)
    }

    pub fn transfer_ownership(&self) -> crate::basic::actions::TransferOwnershipBuilder<'_> {
        crate::basic::actions::TransferOwnershipBuilder::new(self)
    }
}
