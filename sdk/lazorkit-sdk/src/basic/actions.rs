use crate::advanced::instructions;
use crate::basic::proxy::ProxyBuilder;
use crate::basic::wallet::LazorWallet;
use crate::core::connection::SolConnection;
use lazorkit_state::authority::AuthorityType;
use solana_sdk::instruction::Instruction;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::transaction::Transaction;

pub struct CreateWalletBuilder {
    payer: Option<Pubkey>,
    owner: Option<Pubkey>,
    wallet_id: Option<[u8; 32]>,
    program_id: Pubkey,
    owner_type: AuthorityType,
    owner_data: Option<Vec<u8>>,
}

impl CreateWalletBuilder {
    pub fn new() -> Self {
        Self {
            payer: None,
            owner: None,
            wallet_id: None,
            program_id: LazorWallet::DEFAULT_PROGRAM_ID,
            owner_type: AuthorityType::Ed25519,
            owner_data: None,
        }
    }

    pub fn with_payer(mut self, payer: Pubkey) -> Self {
        self.payer = Some(payer);
        self
    }

    pub fn with_owner(mut self, owner: Pubkey) -> Self {
        self.owner = Some(owner);
        self
    }

    pub fn with_id(mut self, id: [u8; 32]) -> Self {
        self.wallet_id = Some(id);
        self
    }

    pub fn with_owner_authority_type(mut self, auth_type: AuthorityType) -> Self {
        self.owner_type = auth_type;
        self
    }

    pub fn with_owner_authority_key(mut self, key: Vec<u8>) -> Self {
        self.owner_data = Some(key);
        self
    }

    pub fn get_pdas(&self) -> (Pubkey, Pubkey) {
        let wallet_id = self.wallet_id.unwrap_or([7u8; 32]);
        let (config_pda, _) =
            Pubkey::find_program_address(&[b"lazorkit", &wallet_id], &self.program_id);
        let (vault_pda, _) = Pubkey::find_program_address(
            &[b"lazorkit-wallet-address", config_pda.as_ref()],
            &self.program_id,
        );
        (config_pda, vault_pda)
    }

    pub async fn build_transaction(
        &self,
        connection: &impl SolConnection,
    ) -> Result<Transaction, String> {
        let payer = self.payer.ok_or("Payer required")?;

        // Generate random wallet ID
        let wallet_id = self.wallet_id.unwrap_or([7u8; 32]);

        let auth_data = if let Some(data) = &self.owner_data {
            match self.owner_type {
                AuthorityType::Ed25519 => {
                    if data.len() != 32 {
                        return Err("Invalid Ed25519 key".into());
                    }
                    data.clone()
                },
                AuthorityType::Secp256r1 => {
                    if data.len() != 33 {
                        return Err("Invalid Secp256r1 key length".into());
                    }
                    data.clone()
                },
                AuthorityType::Ed25519Session => {
                    if data.len() != 72 {
                        return Err("Invalid Ed25519Session data length (expected 72)".into());
                    }
                    data.clone()
                },
                AuthorityType::Secp256r1Session => {
                    if data.len() != 80 {
                        return Err("Invalid Secp256r1Session data length (expected 80)".into());
                    }
                    data.clone()
                },
                _ => return Err("Unsupported owner type".into()),
            }
        } else {
            let owner = self.owner.ok_or("Owner or owner_data required")?;
            owner.to_bytes().to_vec()
        };

        let ix = instructions::create_wallet(
            &self.program_id,
            &payer,
            wallet_id,
            self.owner_type,
            auth_data,
        );

        let _recent_blockhash = connection
            .get_latest_blockhash()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Transaction::new_unsigned(
            solana_sdk::message::Message::new(&[ix], Some(&payer)),
        ))
    }
}

pub struct AddAuthorityBuilder<'a> {
    wallet: &'a LazorWallet,
    new_authority: Option<Vec<u8>>,
    authorized_account: Option<Pubkey>,
    auth_type: AuthorityType,
    authorization_data: Vec<u8>,
    role: u32,
    additional_accounts: Vec<solana_sdk::instruction::AccountMeta>,
    acting_role_id: u32,
}

impl<'a> AddAuthorityBuilder<'a> {
    pub fn new(wallet: &'a LazorWallet) -> Self {
        Self {
            wallet,
            new_authority: None,
            authorized_account: None,
            auth_type: AuthorityType::Ed25519,
            authorization_data: vec![],
            role: 1,
            additional_accounts: Vec::new(),
            acting_role_id: 0,
        }
    }

    pub fn with_authority(mut self, authority: Pubkey) -> Self {
        self.new_authority = Some(authority.to_bytes().to_vec());
        self
    }

    pub fn with_authority_key(mut self, key: Vec<u8>) -> Self {
        self.new_authority = Some(key);
        self
    }

    pub fn with_type(mut self, auth_type: AuthorityType) -> Self {
        self.auth_type = auth_type;
        self
    }

    pub fn with_role(mut self, role: u32) -> Self {
        self.role = role;
        self
    }

    pub fn with_acting_role(mut self, role: u32) -> Self {
        self.acting_role_id = role;
        self
    }

    pub fn with_authorization_data(mut self, data: Vec<u8>) -> Self {
        self.authorization_data = data;
        self
    }

    pub fn with_authorizer(mut self, authorizer: Pubkey) -> Self {
        self.authorized_account = Some(authorizer);
        self.additional_accounts
            .push(solana_sdk::instruction::AccountMeta::new_readonly(
                authorizer, true,
            ));
        self
    }

    pub async fn build_transaction(
        &self,
        connection: &impl SolConnection,
        payer: Pubkey,
    ) -> Result<Transaction, String> {
        let key_vec = self.new_authority.clone().ok_or("New authority required")?;

        let auth_data = match self.auth_type {
            AuthorityType::Ed25519 => {
                if key_vec.len() != 32 {
                    return Err("Invalid Ed25519 key length".to_string());
                }
                key_vec.clone()
            },
            AuthorityType::Secp256r1 => {
                if key_vec.len() != 33 {
                    return Err("Invalid Secp256r1 key length".to_string());
                }
                key_vec.clone()
            },
            AuthorityType::Ed25519Session => {
                if key_vec.len() != 72 {
                    return Err("Invalid Ed25519Session data length (expected 72)".to_string());
                }
                key_vec.clone()
            },
            AuthorityType::Secp256r1Session => {
                if key_vec.len() != 80 {
                    return Err("Invalid Secp256r1Session data length (expected 80)".to_string());
                }
                key_vec.clone()
            },
            _ => return Err("Unsupported Authority Type in SDK".to_string()),
        };

        let ix = instructions::add_authority(
            &self.wallet.program_id,
            &self.wallet.config_pda,
            &payer,
            self.acting_role_id,
            self.auth_type,
            auth_data,
            self.authorization_data.clone(),
            self.additional_accounts.clone(),
        );

        let _recent_blockhash = connection
            .get_latest_blockhash()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Transaction::new_unsigned(
            solana_sdk::message::Message::new(&[ix], Some(&payer)),
        ))
    }
}

pub struct ExecuteBuilder<'a> {
    wallet: &'a LazorWallet,
    proxy_builder: ProxyBuilder,
    acting_role: u32,
    auth_payload: Vec<u8>,
    additional_accounts: Vec<solana_sdk::instruction::AccountMeta>,
}

impl<'a> ExecuteBuilder<'a> {
    pub fn new(wallet: &'a LazorWallet) -> Self {
        Self {
            wallet,
            proxy_builder: ProxyBuilder::new(wallet.address), // Pass Vault address
            acting_role: 0,                                   // Default owner
            auth_payload: Vec::new(),
            additional_accounts: Vec::new(),
        }
    }

    pub fn add_instruction(mut self, ix: Instruction) -> Self {
        self.proxy_builder = self.proxy_builder.add_instruction(ix);
        self
    }

    pub fn with_signer(mut self, signer: Pubkey) -> Self {
        self.proxy_builder = self.proxy_builder.with_signer(signer);
        self
    }

    pub fn with_auth_payload(mut self, auth: Vec<u8>) -> Self {
        self.auth_payload = auth;
        self
    }

    pub fn with_acting_role(mut self, role: u32) -> Self {
        self.acting_role = role;
        self
    }

    pub fn with_role_id(mut self, role: u32) -> Self {
        self.acting_role = role;
        self
    }

    pub fn with_policy(mut self, policy: Pubkey) -> Self {
        self.additional_accounts
            .push(solana_sdk::instruction::AccountMeta::new_readonly(
                policy, false,
            ));
        self
    }

    pub fn with_registry(mut self, registry: Pubkey) -> Self {
        self.additional_accounts
            .push(solana_sdk::instruction::AccountMeta::new_readonly(
                registry, false,
            ));
        self
    }

    pub fn with_authorizer(mut self, pubkey: Pubkey) -> Self {
        self.additional_accounts
            .push(solana_sdk::instruction::AccountMeta::new_readonly(
                pubkey, true,
            ));
        self
    }

    pub async fn build_transaction(
        &self,
        connection: &impl SolConnection,
        payer: Pubkey,
    ) -> Result<Transaction, String> {
        let (target_data, mut accounts, proxy_auth_idx) = self.proxy_builder.clone().build();

        // Add additional signers/authorizers
        accounts.extend(self.additional_accounts.clone());

        // Calculate final absolute auth index
        // Config(0), Vault(1), System(2) are prepended.
        // If auth_payload is provided, it replaces the Proxy index.
        let final_auth_idx = if !self.auth_payload.is_empty() {
            // Should be full payload for Secp256r1, or index for Ed25519
            // But builder here seems focused on Ed25519/Simple cases where auth_payload IS the payload.
            // If self.auth_payload is set, use it.
            // NOTE: If using Secp256r1 builder, caller should have provided full formatted payload.
            // For simple Signer, we usually just need index.
            self.auth_payload.clone()
        } else {
            vec![3 + proxy_auth_idx]
        };

        let ix = instructions::execute(
            &self.wallet.program_id,
            &self.wallet.config_pda,
            &self.wallet.address,
            self.acting_role,
            target_data,
            final_auth_idx,
            accounts,
        );

        let _recent_blockhash = connection
            .get_latest_blockhash()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Transaction::new_unsigned(
            solana_sdk::message::Message::new(&[ix], Some(&payer)),
        ))
    }
}

pub struct RemoveAuthorityBuilder<'a> {
    wallet: &'a LazorWallet,
    acting_role_id: u32,
    target_role_id: Option<u32>,
    authorization_data: Vec<u8>,
    additional_accounts: Vec<solana_sdk::instruction::AccountMeta>,
}

impl<'a> RemoveAuthorityBuilder<'a> {
    pub fn new(wallet: &'a LazorWallet) -> Self {
        Self {
            wallet,
            acting_role_id: 0,
            target_role_id: None,
            authorization_data: Vec::new(),
            additional_accounts: Vec::new(),
        }
    }

    pub fn with_acting_role(mut self, role: u32) -> Self {
        self.acting_role_id = role;
        self
    }

    pub fn with_target_role(mut self, role: u32) -> Self {
        self.target_role_id = Some(role);
        self
    }

    pub fn with_authorization_data(mut self, data: Vec<u8>) -> Self {
        self.authorization_data = data;
        self
    }

    pub fn with_authorizer(mut self, pubkey: Pubkey) -> Self {
        self.additional_accounts
            .push(solana_sdk::instruction::AccountMeta::new_readonly(
                pubkey, true,
            ));
        self
    }

    pub async fn build_transaction(
        &self,
        connection: &impl SolConnection,
        payer: Pubkey,
    ) -> Result<Transaction, String> {
        let target_role = self.target_role_id.ok_or("Target role required")?;

        let ix = instructions::remove_authority(
            &self.wallet.program_id,
            &self.wallet.config_pda,
            &payer,
            self.acting_role_id,
            target_role,
            self.authorization_data.clone(),
            self.additional_accounts.clone(),
        );

        let _recent_blockhash = connection
            .get_latest_blockhash()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Transaction::new_unsigned(
            solana_sdk::message::Message::new(&[ix], Some(&payer)),
        ))
    }
}

pub struct UpdateAuthorityBuilder<'a> {
    wallet: &'a LazorWallet,
    acting_role_id: u32,
    target_role_id: Option<u32>,
    new_authority_data: Vec<u8>,
    authorization_data: Vec<u8>,
    additional_accounts: Vec<solana_sdk::instruction::AccountMeta>,
}

impl<'a> UpdateAuthorityBuilder<'a> {
    pub fn new(wallet: &'a LazorWallet) -> Self {
        Self {
            wallet,
            acting_role_id: 0,
            target_role_id: None,
            new_authority_data: Vec::new(),
            authorization_data: Vec::new(),
            additional_accounts: Vec::new(),
        }
    }

    pub fn with_acting_role(mut self, role: u32) -> Self {
        self.acting_role_id = role;
        self
    }

    pub fn with_target_role(mut self, role: u32) -> Self {
        self.target_role_id = Some(role);
        self
    }

    pub fn with_new_authority_data(mut self, data: Vec<u8>) -> Self {
        self.new_authority_data = data;
        self
    }

    pub fn with_authorization_data(mut self, data: Vec<u8>) -> Self {
        self.authorization_data = data;
        self
    }

    pub fn with_registry(mut self, registry_pda: Pubkey) -> Self {
        self.additional_accounts
            .push(solana_sdk::instruction::AccountMeta::new_readonly(
                registry_pda,
                false,
            ));
        self
    }

    pub fn with_authorizer(mut self, pubkey: Pubkey) -> Self {
        self.additional_accounts
            .push(solana_sdk::instruction::AccountMeta::new_readonly(
                pubkey, true,
            ));
        self
    }

    pub async fn build_transaction(
        &self,
        connection: &impl SolConnection,
        payer: Pubkey,
    ) -> Result<Transaction, String> {
        let target_role = self.target_role_id.ok_or("Target role required")?;

        let ix = instructions::update_authority(
            &self.wallet.program_id,
            &self.wallet.config_pda,
            &payer,
            self.acting_role_id,
            target_role,
            self.new_authority_data.clone(),
            self.authorization_data.clone(),
            self.additional_accounts.clone(),
        );

        let _recent_blockhash = connection
            .get_latest_blockhash()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Transaction::new_unsigned(
            solana_sdk::message::Message::new(&[ix], Some(&payer)),
        ))
    }
}

pub struct CreateSessionBuilder<'a> {
    wallet: &'a LazorWallet,
    role_id: u32,
    session_key: Option<[u8; 32]>,
    duration: u64,
    authorization_data: Vec<u8>,
    additional_accounts: Vec<solana_sdk::instruction::AccountMeta>,
}

impl<'a> CreateSessionBuilder<'a> {
    pub fn new(wallet: &'a LazorWallet) -> Self {
        Self {
            wallet,
            role_id: 0,
            session_key: None,
            duration: 0,
            authorization_data: Vec::new(),
            additional_accounts: Vec::new(),
        }
    }

    pub fn with_role(mut self, role: u32) -> Self {
        self.role_id = role;
        self
    }

    pub fn with_session_key(mut self, key: [u8; 32]) -> Self {
        self.session_key = Some(key);
        self
    }

    pub fn with_duration(mut self, duration: u64) -> Self {
        self.duration = duration;
        self
    }

    pub fn with_authorization_data(mut self, data: Vec<u8>) -> Self {
        self.authorization_data = data;
        self
    }

    pub fn with_authorizer(mut self, pubkey: Pubkey) -> Self {
        self.additional_accounts
            .push(solana_sdk::instruction::AccountMeta::new_readonly(
                pubkey, true,
            ));
        self
    }

    pub async fn build_transaction(
        &self,
        connection: &impl SolConnection,
        payer: Pubkey,
    ) -> Result<Transaction, String> {
        let session_key = self.session_key.ok_or("Session key required")?;

        let ix = instructions::create_session(
            &self.wallet.program_id,
            &self.wallet.config_pda,
            &payer,
            self.role_id,
            session_key,
            self.duration,
            self.authorization_data.clone(),
            self.additional_accounts.clone(),
        );

        let _recent_blockhash = connection
            .get_latest_blockhash()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Transaction::new_unsigned(
            solana_sdk::message::Message::new(&[ix], Some(&payer)),
        ))
    }
}

pub struct TransferOwnershipBuilder<'a> {
    wallet: &'a LazorWallet,
    current_owner: Option<Pubkey>,
    new_owner_type: AuthorityType,
    new_owner_data: Option<Vec<u8>>,
    authorization_data: Vec<u8>,
}

impl<'a> TransferOwnershipBuilder<'a> {
    pub fn new(wallet: &'a LazorWallet) -> Self {
        Self {
            wallet,
            current_owner: None,
            new_owner_type: AuthorityType::Ed25519,
            new_owner_data: None,
            authorization_data: Vec::new(),
        }
    }

    pub fn with_current_owner(mut self, owner: Pubkey) -> Self {
        self.current_owner = Some(owner);
        self
    }

    pub fn with_new_owner(mut self, owner_type: AuthorityType, owner_data: Vec<u8>) -> Self {
        self.new_owner_type = owner_type;
        self.new_owner_data = Some(owner_data);
        self
    }

    pub fn with_authorization_data(mut self, data: Vec<u8>) -> Self {
        self.authorization_data = data;
        self
    }

    pub async fn build_transaction(
        &self,
        connection: &impl SolConnection,
        payer: Pubkey,
    ) -> Result<Transaction, String> {
        let current_owner = self.current_owner.ok_or("Current owner required")?;
        let new_owner_data = self
            .new_owner_data
            .clone()
            .ok_or("New owner data required")?;

        let ix = instructions::transfer_ownership(
            &self.wallet.program_id,
            &self.wallet.config_pda,
            &current_owner,
            self.new_owner_type as u16,
            new_owner_data,
            self.authorization_data.clone(),
        );

        let _recent_blockhash = connection
            .get_latest_blockhash()
            .await
            .map_err(|e| e.to_string())?;
        Ok(Transaction::new_unsigned(
            solana_sdk::message::Message::new(&[ix], Some(&payer)),
        ))
    }
}

///
/// # Arguments
/// * `pubkey` - The Ed25519 public key (32 bytes)
/// * `max_session_age` - Maximum allowed session duration in slots
///
/// # Returns
/// A 72-byte vector containing the session authority data:
/// - 32 bytes: public key
/// - 32 bytes: initial session key (empty)
/// - 8 bytes: max_session_age
pub fn create_ed25519_session_data(pubkey: [u8; 32], max_session_age: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(72);
    data.extend_from_slice(&pubkey);
    data.extend_from_slice(&[0u8; 32]); // Initial session key is empty
    data.extend_from_slice(&max_session_age.to_le_bytes());
    data
}

///
/// # Arguments
/// * `pubkey` - The compressed Secp256r1 public key (33 bytes)
/// * `max_session_age` - Maximum allowed session duration in slots
///
/// # Returns
/// An 80-byte vector containing the session creation data:
/// - 33 bytes: public key
/// - 7 bytes: padding
/// - 32 bytes: initial session key (empty)
/// - 8 bytes: max_session_length
pub fn create_secp256r1_session_data(pubkey: [u8; 33], max_session_age: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(80);
    data.extend_from_slice(&pubkey);
    data.extend_from_slice(&[0u8; 7]); // Padding
    data.extend_from_slice(&[0u8; 32]); // Initial session key is empty
    data.extend_from_slice(&max_session_age.to_le_bytes());
    data
}
