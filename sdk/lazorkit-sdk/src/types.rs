//! Types for LazorKit SDK
//!
//! # Replay Protection
//!
//! LazorKit uses different replay protection mechanisms per authority type:
//!
//! ## Ed25519 / Ed25519Session
//! - **Native Solana signature verification**
//! - Implicit replay protection via Instructions sysvar
//! - No additional counter needed
//!
//! ## Secp256r1 / Secp256r1Session  
//! - **Dual-layer protection**:
//!   1. **Counter-based sequencing**: Each signature must increment [`RoleInfo::signature_odometer`] by exactly 1
//!   2. **Slot age validation**: Signature must be within 60 slots (~30 seconds) of current slot
//! - See [`RoleInfo::next_signature_counter`] for getting the expected next counter
//! - Use [`crate::utils::build_secp256r1_auth_payload`] to construct authorization payloads
//!
//! # Session Keys
//!
//! Session-based authorities (Ed25519Session, Secp256r1Session) support temporary session keys:
//! - **Session key type**: Always Ed25519 (32 bytes), regardless of master authority type
//!   - This is an intentional design: Ed25519 is native to Solana and cheap to verify
//!   - Master key keeps its original type (Secp256r1 for passkeys, Ed25519 for standard)
//! - **Creation**: Master key signs session creation transaction
//! - **Usage**: Session key can be used until expiration (checked via [`RoleInfo::is_session_active`])
//! - **Limits**:
//!   - Ed25519Session: `max_session_length` defines maximum duration
//!   - Secp256r1Session: `max_session_age` defines maximum duration

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

    /// For session types: the current session key (Ed25519 format)
    pub session_key: Option<[u8; 32]>,

    /// For Ed25519Session: max session duration in slots
    pub max_session_length: Option<u64>,

    /// For Secp256r1Session: max session age in slots
    pub max_session_age: Option<u64>,

    /// For session types: current session expiration slot
    pub current_session_expiration: Option<u64>,

    /// For Secp256r1/Secp256r1Session: signature counter for replay protection
    /// Must increment by exactly 1 with each signature
    pub signature_odometer: Option<u32>,
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

    /// Check if this role has administrative privileges (Owner or Admin)
    pub fn can_manage_authorities(&self) -> bool {
        self.id <= 1
    }

    /// Check if session is currently active (not expired)
    pub fn is_session_active(&self, current_slot: u64) -> bool {
        if let Some(expiration) = self.current_session_expiration {
            current_slot < expiration
        } else {
            false
        }
    }

    /// Get the next expected signature counter for Secp256r1 authorities
    /// Returns None for non-Secp256r1 authorities
    pub fn next_signature_counter(&self) -> Option<u32> {
        self.signature_odometer
            .map(|counter| counter.wrapping_add(1))
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
