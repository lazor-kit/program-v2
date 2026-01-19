use crate::basic::actions::{AddAuthorityBuilder, CreateWalletBuilder, ExecuteBuilder};
use crate::core::connection::SolConnection;
use crate::error::{LazorSdkError, Result};
use crate::types::{RoleInfo, WalletInfo};
use crate::utils;
use solana_sdk::pubkey::Pubkey;

/// Represents a LazorKit Smart Wallet on-chain.
#[derive(Debug, Clone)]
pub struct LazorWallet {
    /// Vault PDA - the user-facing wallet address that holds funds
    pub address: Pubkey,

    /// Program ID of the LazorKit contract
    pub program_id: Pubkey,

    /// Config PDA - the account that stores wallet state and roles
    pub config_pda: Pubkey,

    /// Config PDA bump seed
    pub config_bump: u8,
}

impl LazorWallet {
    pub const DEFAULT_PROGRAM_ID: Pubkey =
        solana_sdk::pubkey!("LazorKit11111111111111111111111111111111111");

    /// Fetch an existing wallet from the blockchain by its config PDA
    ///
    /// # Arguments
    /// * `connection` - Solana RPC connection
    /// * `config_pda` - The config PDA address
    /// * `program_id` - Program ID (optional, defaults to DEFAULT_PROGRAM_ID)
    ///
    /// # Returns
    /// A LazorWallet instance with fetched data
    pub async fn fetch(
        connection: &impl SolConnection,
        config_pda: &Pubkey,
        program_id: Option<Pubkey>,
    ) -> Result<Self> {
        let program_id = program_id.unwrap_or(Self::DEFAULT_PROGRAM_ID);

        // Fetch and validate account exists
        let _data = utils::fetch_wallet_account(connection, config_pda).await?;

        // Derive vault PDA
        let (vault_pda, _) = utils::derive_vault_pda(&program_id, config_pda);

        Ok(Self {
            address: vault_pda,
            program_id,
            config_pda: *config_pda,
            config_bump: 0,
        })
    }

    /// Fetch complete wallet information including all roles
    pub async fn fetch_info(&self, connection: &impl SolConnection) -> Result<WalletInfo> {
        utils::fetch_wallet_info(connection, &self.config_pda).await
    }

    /// List all roles in the wallet
    pub async fn list_roles(&self, connection: &impl SolConnection) -> Result<Vec<RoleInfo>> {
        let data = utils::fetch_wallet_account(connection, &self.config_pda).await?;
        utils::parse_roles(&data)
    }

    /// Get a specific role by ID
    pub async fn get_role(
        &self,
        role_id: u32,
        connection: &impl SolConnection,
    ) -> Result<RoleInfo> {
        let roles = self.list_roles(connection).await?;
        utils::find_role(&roles, role_id)
            .cloned()
            .ok_or(LazorSdkError::RoleNotFound(role_id))
    }

    /// Check if a role exists
    pub async fn has_role(&self, role_id: u32, connection: &impl SolConnection) -> Result<bool> {
        let roles = self.list_roles(connection).await?;
        Ok(utils::find_role(&roles, role_id).is_some())
    }

    /// Connect to an existing wallet by its vault address (legacy)
    pub async fn connect(_connection: &impl SolConnection, wallet_address: Pubkey) -> Result<Self> {
        Ok(Self {
            address: wallet_address,
            program_id: Self::DEFAULT_PROGRAM_ID,
            config_pda: Pubkey::default(),
            config_bump: 0,
        })
    }

    /// Create a new wallet
    pub fn create() -> CreateWalletBuilder {
        CreateWalletBuilder::new()
    }

    /// Start building an AddAuthority transaction
    pub fn add_authority(&self) -> AddAuthorityBuilder<'_> {
        AddAuthorityBuilder::new(self)
    }

    /// Construct wallet instance with known parameters
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
