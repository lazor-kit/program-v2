use lazorkit_state::authority::AuthorityType;

/// Information about a role in the wallet
#[derive(Debug, Clone)]
pub struct RoleInfo {
    /// Role ID (0 = Owner, 1 = Admin, 2+ = Spender)
    pub id: u32,

    /// Authority type code
    pub authority_type: AuthorityType,

    /// For Ed25519/Ed25519Session: the public key
    pub ed25519_pubkey: Option<[u8; 32]>,

    /// For Secp256r1/Secp256r1Session: compressed public key
    pub secp256r1_pubkey: Option<[u8; 33]>,

    /// Whether this authority supports sessions
    pub has_session_support: bool,

    /// For session types: max session duration in slots
    pub max_session_length: Option<u64>,

    /// For session types: current session expiration slot
    pub current_session_expiration: Option<u64>,
}

impl RoleInfo {
    /// Check if this is the Owner role
    pub fn is_owner(&self) -> bool {
        self.id == 0
    }

    /// Check if this is an Admin role
    pub fn is_admin(&self) -> bool {
        self.id == 1
    }

    /// Check if this is a Spender role
    pub fn is_spender(&self) -> bool {
        self.id >= 2
    }

    /// Check if session is currently active (not expired)
    pub fn is_session_active(&self, current_slot: u64) -> bool {
        if let Some(expiration) = self.current_session_expiration {
            current_slot < expiration
        } else {
            false
        }
    }
}

/// Parsed wallet header information
#[derive(Debug, Clone)]
pub struct WalletInfo {
    /// Number of roles in the wallet
    pub role_count: u32,

    /// Total roles ever created (for ID assignment)
    pub role_counter: u32,

    /// Vault PDA bump seed
    pub vault_bump: u8,

    /// List of all roles
    pub roles: Vec<RoleInfo>,
}
