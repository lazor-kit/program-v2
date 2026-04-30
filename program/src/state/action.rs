//! Session action types for permission enforcement.
//!
//! Actions are optional, immutable permission rules attached to sessions at creation time.
//! They are stored as a flat byte buffer appended after the 80-byte SessionAccount header.
//!
//! Each action has an 11-byte header: [type: u8][data_len: u16 LE][expires_at: u64 LE]
//! followed by type-specific data bytes.

use pinocchio::program_error::ProgramError;

use crate::error::AuthError;

// ─── Action Header ────────────────────────────────────────────────────

/// Size of each action header in bytes.
pub const ACTION_HEADER_SIZE: usize = 11;

/// Maximum number of actions per session.
pub const MAX_ACTIONS: usize = 16;

// ─── Action Types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ActionType {
    /// Lifetime SOL spending cap. Data: {remaining: u64}
    SolLimit = 1,
    /// Per-window SOL spending cap. Data: {limit, spent, window, last_reset}
    SolRecurringLimit = 2,
    /// Maximum SOL per single execute. Data: {max: u64}
    SolMaxPerTx = 3,
    /// Lifetime token spending cap per mint. Data: {mint: [u8;32], remaining: u64}
    TokenLimit = 4,
    /// Per-window token spending cap per mint. Data: {mint, limit, spent, window, last_reset}
    TokenRecurringLimit = 5,
    /// Maximum tokens per single execute per mint. Data: {mint: [u8;32], max: u64}
    TokenMaxPerTx = 6,
    /// Allow CPI only to this program. Repeatable. Data: {program_id: [u8;32]}
    ProgramWhitelist = 10,
    /// Block CPI to this program. Repeatable. Data: {program_id: [u8;32]}
    ProgramBlacklist = 11,
}

impl ActionType {
    pub fn from_u8(v: u8) -> Result<Self, ProgramError> {
        match v {
            1 => Ok(Self::SolLimit),
            2 => Ok(Self::SolRecurringLimit),
            3 => Ok(Self::SolMaxPerTx),
            4 => Ok(Self::TokenLimit),
            5 => Ok(Self::TokenRecurringLimit),
            6 => Ok(Self::TokenMaxPerTx),
            10 => Ok(Self::ProgramWhitelist),
            11 => Ok(Self::ProgramBlacklist),
            _ => Err(AuthError::ActionBufferInvalid.into()),
        }
    }

    /// Expected data size for this action type (excluding header).
    pub fn expected_data_size(&self) -> usize {
        match self {
            Self::SolLimit => SOL_LIMIT_SIZE,
            Self::SolRecurringLimit => SOL_RECURRING_LIMIT_SIZE,
            Self::SolMaxPerTx => SOL_MAX_PER_TX_SIZE,
            Self::TokenLimit => TOKEN_LIMIT_SIZE,
            Self::TokenRecurringLimit => TOKEN_RECURRING_LIMIT_SIZE,
            Self::TokenMaxPerTx => TOKEN_MAX_PER_TX_SIZE,
            Self::ProgramWhitelist => PROGRAM_WHITELIST_SIZE,
            Self::ProgramBlacklist => PROGRAM_BLACKLIST_SIZE,
        }
    }
}

// ─── Data Sizes ───────────────────────────────────────────────────────

pub const SOL_LIMIT_SIZE: usize = 8;
pub const SOL_RECURRING_LIMIT_SIZE: usize = 32;
pub const SOL_MAX_PER_TX_SIZE: usize = 8;
pub const TOKEN_LIMIT_SIZE: usize = 40;
pub const TOKEN_RECURRING_LIMIT_SIZE: usize = 64;
pub const TOKEN_MAX_PER_TX_SIZE: usize = 40;
pub const PROGRAM_WHITELIST_SIZE: usize = 32;
pub const PROGRAM_BLACKLIST_SIZE: usize = 32;

// ─── Action View (zero-copy index into buffer) ───────────────────────

/// A parsed reference to an action within the session data buffer.
/// Does not own data — just indexes into the buffer.
#[derive(Debug, Clone)]
pub struct ActionView {
    pub action_type: ActionType,
    pub expires_at: u64,
    /// Byte offset of this action's data within the actions buffer
    /// (relative to start of actions buffer, NOT session account start).
    pub data_offset: usize,
    pub data_len: usize,
}

// ─── Buffer Parsing ───────────────────────────────────────────────────

/// Parse all actions from a raw actions buffer.
///
/// The buffer starts immediately after the 80-byte session header.
/// Returns a Vec of ActionViews indexing into the buffer.
pub fn parse_actions(buf: &[u8]) -> Result<Vec<ActionView>, ProgramError> {
    let mut actions = Vec::new();
    let mut cursor = 0;

    while cursor < buf.len() {
        if cursor + ACTION_HEADER_SIZE > buf.len() {
            return Err(AuthError::ActionBufferInvalid.into());
        }

        let action_type = ActionType::from_u8(buf[cursor])?;
        let data_len = u16::from_le_bytes([buf[cursor + 1], buf[cursor + 2]]) as usize;
        let expires_at = u64::from_le_bytes(
            buf[cursor + 3..cursor + 11]
                .try_into()
                .map_err(|_| AuthError::ActionBufferInvalid)?,
        );

        let data_offset = cursor + ACTION_HEADER_SIZE;
        if data_offset + data_len > buf.len() {
            return Err(AuthError::ActionBufferInvalid.into());
        }

        actions.push(ActionView {
            action_type,
            expires_at,
            data_offset,
            data_len,
        });

        cursor = data_offset + data_len;

        if actions.len() > MAX_ACTIONS {
            return Err(AuthError::ActionBufferInvalid.into());
        }
    }

    Ok(actions)
}

/// Validate an actions buffer at session creation time.
///
/// Checks:
/// - All action types are known
/// - Data sizes match expected sizes per type
/// - No simultaneous ProgramWhitelist + ProgramBlacklist
/// - Buffer is fully consumed (no trailing bytes)
/// - Not exceeding MAX_ACTIONS
pub fn validate_actions_buffer(buf: &[u8]) -> Result<(), ProgramError> {
    if buf.is_empty() {
        return Ok(());
    }

    let actions = parse_actions(buf)?;

    // Verify data sizes match expected
    for action in &actions {
        if action.data_len != action.action_type.expected_data_size() {
            return Err(AuthError::ActionBufferInvalid.into());
        }
    }

    // Check no whitelist + blacklist coexistence
    let has_whitelist = actions
        .iter()
        .any(|a| a.action_type == ActionType::ProgramWhitelist);
    let has_blacklist = actions
        .iter()
        .any(|a| a.action_type == ActionType::ProgramBlacklist);
    if has_whitelist && has_blacklist {
        return Err(AuthError::ActionWhitelistBlacklistConflict.into());
    }

    // Check no duplicate non-repeatable actions
    let mut has_sol_limit = false;
    let mut has_sol_recurring = false;
    let mut has_sol_max_per_tx = false;
    for action in &actions {
        match action.action_type {
            ActionType::SolLimit => {
                if has_sol_limit {
                    return Err(AuthError::ActionBufferInvalid.into());
                }
                has_sol_limit = true;
            }
            ActionType::SolRecurringLimit => {
                if has_sol_recurring {
                    return Err(AuthError::ActionBufferInvalid.into());
                }
                has_sol_recurring = true;
            }
            ActionType::SolMaxPerTx => {
                if has_sol_max_per_tx {
                    return Err(AuthError::ActionBufferInvalid.into());
                }
                has_sol_max_per_tx = true;
            }
            _ => {} // Repeatable types are fine
        }
    }

    // Check no duplicate token actions for the same mint
    // (Two TokenLimit for the same mint would create confusing deduction semantics)
    {
        let token_types = [
            ActionType::TokenLimit,
            ActionType::TokenRecurringLimit,
            ActionType::TokenMaxPerTx,
        ];
        for token_type in &token_types {
            let token_actions: Vec<&ActionView> = actions
                .iter()
                .filter(|a| &a.action_type == token_type)
                .collect();
            for i in 0..token_actions.len() {
                for j in (i + 1)..token_actions.len() {
                    let mint_a = &buf[token_actions[i].data_offset..token_actions[i].data_offset + 32];
                    let mint_b = &buf[token_actions[j].data_offset..token_actions[j].data_offset + 32];
                    if mint_a == mint_b {
                        return Err(AuthError::ActionBufferInvalid.into());
                    }
                }
            }
        }
    }

    // Validate recurring limit initial state
    for action in &actions {
        if action.action_type == ActionType::SolRecurringLimit {
            let data = &buf[action.data_offset..action.data_offset + action.data_len];
            // spent must be 0 at creation
            let spent = u64::from_le_bytes(data[8..16].try_into().unwrap());
            if spent != 0 {
                return Err(AuthError::ActionBufferInvalid.into());
            }
            // window must be > 0
            let window = u64::from_le_bytes(data[16..24].try_into().unwrap());
            if window == 0 {
                return Err(AuthError::ActionBufferInvalid.into());
            }
            // last_reset must be 0
            let last_reset = u64::from_le_bytes(data[24..32].try_into().unwrap());
            if last_reset != 0 {
                return Err(AuthError::ActionBufferInvalid.into());
            }
        }
        if action.action_type == ActionType::TokenRecurringLimit {
            let data = &buf[action.data_offset..action.data_offset + action.data_len];
            // spent must be 0 (bytes 40..48)
            let spent = u64::from_le_bytes(data[40..48].try_into().unwrap());
            if spent != 0 {
                return Err(AuthError::ActionBufferInvalid.into());
            }
            // window must be > 0 (bytes 48..56)
            let window = u64::from_le_bytes(data[48..56].try_into().unwrap());
            if window == 0 {
                return Err(AuthError::ActionBufferInvalid.into());
            }
            // last_reset must be 0 (bytes 56..64)
            let last_reset = u64::from_le_bytes(data[56..64].try_into().unwrap());
            if last_reset != 0 {
                return Err(AuthError::ActionBufferInvalid.into());
            }
        }
    }

    Ok(())
}

// ─── Data Layout Helpers ──────────────────────────────────────────────

// SolLimit: [remaining: u64] = 8 bytes
// Offsets within data: remaining = 0..8

// SolRecurringLimit: [limit: u64][spent: u64][window: u64][last_reset: u64] = 32 bytes
// Offsets: limit = 0..8, spent = 8..16, window = 16..24, last_reset = 24..32

// SolMaxPerTx: [max: u64] = 8 bytes
// Offsets: max = 0..8

// TokenLimit: [mint: [u8;32]][remaining: u64] = 40 bytes
// Offsets: mint = 0..32, remaining = 32..40

// TokenRecurringLimit: [mint: [u8;32]][limit: u64][spent: u64][window: u64][last_reset: u64] = 64 bytes
// Offsets: mint = 0..32, limit = 32..40, spent = 40..48, window = 48..56, last_reset = 56..64

// TokenMaxPerTx: [mint: [u8;32]][max: u64] = 40 bytes
// Offsets: mint = 0..32, max = 32..40

// ProgramWhitelist: [program_id: [u8;32]] = 32 bytes
// ProgramBlacklist: [program_id: [u8;32]] = 32 bytes

/// Read a u64 from a byte slice at the given offset (LE).
#[inline(always)]
pub fn read_u64(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
}

/// Write a u64 to a byte slice at the given offset (LE).
#[inline(always)]
pub fn write_u64(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

// ─── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn build_action(action_type: u8, expires_at: u64, data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.push(action_type);
        buf.extend_from_slice(&(data.len() as u16).to_le_bytes());
        buf.extend_from_slice(&expires_at.to_le_bytes());
        buf.extend_from_slice(data);
        buf
    }

    #[test]
    fn test_parse_empty_buffer() {
        let actions = parse_actions(&[]).unwrap();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_parse_sol_limit() {
        let data = 1_000_000u64.to_le_bytes();
        let buf = build_action(1, 0, &data);
        let actions = parse_actions(&buf).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, ActionType::SolLimit);
        assert_eq!(actions[0].expires_at, 0);
        assert_eq!(actions[0].data_len, 8);
    }

    #[test]
    fn test_parse_sol_recurring_limit() {
        let mut data = Vec::new();
        data.extend_from_slice(&1_000_000u64.to_le_bytes()); // limit
        data.extend_from_slice(&0u64.to_le_bytes()); // spent
        data.extend_from_slice(&216_000u64.to_le_bytes()); // window (~1 day)
        data.extend_from_slice(&0u64.to_le_bytes()); // last_reset
        let buf = build_action(2, 0, &data);
        let actions = parse_actions(&buf).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].action_type, ActionType::SolRecurringLimit);
        assert_eq!(actions[0].data_len, 32);
    }

    #[test]
    fn test_parse_program_whitelist_multiple() {
        let prog1 = [1u8; 32];
        let prog2 = [2u8; 32];
        let mut buf = build_action(10, 0, &prog1);
        buf.extend_from_slice(&build_action(10, 0, &prog2));
        let actions = parse_actions(&buf).unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].action_type, ActionType::ProgramWhitelist);
        assert_eq!(actions[1].action_type, ActionType::ProgramWhitelist);
    }

    #[test]
    fn test_parse_multiple_action_types() {
        let mut buf = Vec::new();
        // SolMaxPerTx
        buf.extend_from_slice(&build_action(3, 0, &500_000u64.to_le_bytes()));
        // ProgramWhitelist
        buf.extend_from_slice(&build_action(10, 0, &[0xAA; 32]));
        let actions = parse_actions(&buf).unwrap();
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].action_type, ActionType::SolMaxPerTx);
        assert_eq!(actions[1].action_type, ActionType::ProgramWhitelist);
    }

    #[test]
    fn test_parse_with_expiry() {
        let data = 1_000_000u64.to_le_bytes();
        let buf = build_action(1, 5000, &data);
        let actions = parse_actions(&buf).unwrap();
        assert_eq!(actions[0].expires_at, 5000);
    }

    #[test]
    fn test_validate_unknown_type() {
        let buf = build_action(99, 0, &[0u8; 8]);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_wrong_data_size() {
        // SolLimit expects 8 bytes, give it 16
        let buf = build_action(1, 0, &[0u8; 16]);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_whitelist_blacklist_conflict() {
        let mut buf = build_action(10, 0, &[1u8; 32]); // whitelist
        buf.extend_from_slice(&build_action(11, 0, &[2u8; 32])); // blacklist
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_duplicate_sol_limit() {
        let mut buf = build_action(1, 0, &1_000_000u64.to_le_bytes());
        buf.extend_from_slice(&build_action(1, 0, &2_000_000u64.to_le_bytes()));
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_recurring_nonzero_spent() {
        let mut data = Vec::new();
        data.extend_from_slice(&1_000_000u64.to_le_bytes()); // limit
        data.extend_from_slice(&100u64.to_le_bytes()); // spent (should be 0!)
        data.extend_from_slice(&216_000u64.to_le_bytes()); // window
        data.extend_from_slice(&0u64.to_le_bytes()); // last_reset
        let buf = build_action(2, 0, &data);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_recurring_zero_window() {
        let mut data = Vec::new();
        data.extend_from_slice(&1_000_000u64.to_le_bytes()); // limit
        data.extend_from_slice(&0u64.to_le_bytes()); // spent
        data.extend_from_slice(&0u64.to_le_bytes()); // window = 0 (invalid!)
        data.extend_from_slice(&0u64.to_le_bytes()); // last_reset
        let buf = build_action(2, 0, &data);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_empty_ok() {
        assert!(validate_actions_buffer(&[]).is_ok());
    }

    #[test]
    fn test_validate_valid_combined() {
        let mut buf = Vec::new();
        // SolRecurringLimit
        let mut sol_rec = Vec::new();
        sol_rec.extend_from_slice(&1_000_000u64.to_le_bytes());
        sol_rec.extend_from_slice(&0u64.to_le_bytes());
        sol_rec.extend_from_slice(&216_000u64.to_le_bytes());
        sol_rec.extend_from_slice(&0u64.to_le_bytes());
        buf.extend_from_slice(&build_action(2, 0, &sol_rec));
        // ProgramWhitelist
        buf.extend_from_slice(&build_action(10, 0, &[0xBB; 32]));
        // SolMaxPerTx
        buf.extend_from_slice(&build_action(3, 0, &500_000u64.to_le_bytes()));
        assert!(validate_actions_buffer(&buf).is_ok());
    }

    #[test]
    fn test_truncated_header() {
        let buf = vec![1u8, 0]; // Only 2 bytes, need 11
        assert!(parse_actions(&buf).is_err());
    }

    #[test]
    fn test_truncated_data() {
        let mut buf = Vec::new();
        buf.push(1); // type
        buf.extend_from_slice(&8u16.to_le_bytes()); // data_len = 8
        buf.extend_from_slice(&0u64.to_le_bytes()); // expires_at
        buf.extend_from_slice(&[0u8; 4]); // only 4 bytes of data (need 8)
        assert!(parse_actions(&buf).is_err());
    }

    #[test]
    fn test_token_max_per_tx() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xCC; 32]); // mint
        data.extend_from_slice(&1_000_000u64.to_le_bytes()); // max
        let buf = build_action(6, 0, &data);
        let actions = parse_actions(&buf).unwrap();
        assert_eq!(actions[0].action_type, ActionType::TokenMaxPerTx);
        assert!(validate_actions_buffer(&buf).is_ok());
    }

    #[test]
    fn test_read_write_u64() {
        let mut data = [0u8; 16];
        write_u64(&mut data, 0, 12345);
        write_u64(&mut data, 8, 67890);
        assert_eq!(read_u64(&data, 0), 12345);
        assert_eq!(read_u64(&data, 8), 67890);
    }

    // ─── Security: Duplicate token mint validation (Audit Finding 6) ──

    #[test]
    fn test_validate_duplicate_token_limit_same_mint() {
        let mint = [0xAA; 32];
        let mut data1 = Vec::new();
        data1.extend_from_slice(&mint);
        data1.extend_from_slice(&1_000_000u64.to_le_bytes());

        let mut data2 = Vec::new();
        data2.extend_from_slice(&mint); // same mint!
        data2.extend_from_slice(&2_000_000u64.to_le_bytes());

        let mut buf = build_action(4, 0, &data1); // TokenLimit
        buf.extend_from_slice(&build_action(4, 0, &data2)); // TokenLimit same mint
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_duplicate_token_recurring_same_mint() {
        let mint = [0xBB; 32];
        let mut make_data = || {
            let mut data = Vec::new();
            data.extend_from_slice(&mint);
            data.extend_from_slice(&1_000_000u64.to_le_bytes()); // limit
            data.extend_from_slice(&0u64.to_le_bytes()); // spent
            data.extend_from_slice(&100u64.to_le_bytes()); // window
            data.extend_from_slice(&0u64.to_le_bytes()); // last_reset
            data
        };

        let mut buf = build_action(5, 0, &make_data()); // TokenRecurringLimit
        buf.extend_from_slice(&build_action(5, 0, &make_data())); // same mint
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_different_token_mints_ok() {
        let mut data1 = Vec::new();
        data1.extend_from_slice(&[0xAA; 32]); // mint A
        data1.extend_from_slice(&1_000_000u64.to_le_bytes());

        let mut data2 = Vec::new();
        data2.extend_from_slice(&[0xBB; 32]); // mint B (different!)
        data2.extend_from_slice(&2_000_000u64.to_le_bytes());

        let mut buf = build_action(4, 0, &data1);
        buf.extend_from_slice(&build_action(4, 0, &data2));
        assert!(validate_actions_buffer(&buf).is_ok());
    }

    #[test]
    fn test_validate_same_mint_different_action_types_ok() {
        // Same mint in TokenLimit and TokenMaxPerTx is OK (different action types)
        let mint = [0xCC; 32];
        let mut data_limit = Vec::new();
        data_limit.extend_from_slice(&mint);
        data_limit.extend_from_slice(&1_000_000u64.to_le_bytes());

        let mut data_max = Vec::new();
        data_max.extend_from_slice(&mint);
        data_max.extend_from_slice(&500_000u64.to_le_bytes());

        let mut buf = build_action(4, 0, &data_limit); // TokenLimit
        buf.extend_from_slice(&build_action(6, 0, &data_max)); // TokenMaxPerTx
        assert!(validate_actions_buffer(&buf).is_ok());
    }

    // ─── Security: Duplicate SOL actions ──────────────────────────────

    #[test]
    fn test_validate_duplicate_sol_recurring_limit() {
        let make_data = || {
            let mut data = Vec::new();
            data.extend_from_slice(&1_000_000u64.to_le_bytes());
            data.extend_from_slice(&0u64.to_le_bytes());
            data.extend_from_slice(&100u64.to_le_bytes());
            data.extend_from_slice(&0u64.to_le_bytes());
            data
        };
        let mut buf = build_action(2, 0, &make_data());
        buf.extend_from_slice(&build_action(2, 0, &make_data()));
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_duplicate_sol_max_per_tx() {
        let mut buf = build_action(3, 0, &500_000u64.to_le_bytes());
        buf.extend_from_slice(&build_action(3, 0, &300_000u64.to_le_bytes()));
        assert!(validate_actions_buffer(&buf).is_err());
    }

    // ─── Security: Token recurring limit initial state ────────────────

    #[test]
    fn test_validate_token_recurring_nonzero_spent() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xAA; 32]); // mint
        data.extend_from_slice(&1_000_000u64.to_le_bytes()); // limit
        data.extend_from_slice(&100u64.to_le_bytes()); // spent (should be 0!)
        data.extend_from_slice(&100u64.to_le_bytes()); // window
        data.extend_from_slice(&0u64.to_le_bytes()); // last_reset
        let buf = build_action(5, 0, &data);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_token_recurring_zero_window() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xAA; 32]); // mint
        data.extend_from_slice(&1_000_000u64.to_le_bytes()); // limit
        data.extend_from_slice(&0u64.to_le_bytes()); // spent
        data.extend_from_slice(&0u64.to_le_bytes()); // window = 0!
        data.extend_from_slice(&0u64.to_le_bytes()); // last_reset
        let buf = build_action(5, 0, &data);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_token_recurring_nonzero_last_reset() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xAA; 32]); // mint
        data.extend_from_slice(&1_000_000u64.to_le_bytes()); // limit
        data.extend_from_slice(&0u64.to_le_bytes()); // spent
        data.extend_from_slice(&100u64.to_le_bytes()); // window
        data.extend_from_slice(&50u64.to_le_bytes()); // last_reset != 0!
        let buf = build_action(5, 0, &data);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    // ─── Security: Action type boundary values ────────────────────────

    #[test]
    fn test_validate_action_type_7_rejected() {
        let buf = build_action(7, 0, &[0u8; 8]); // gap value
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_action_type_8_rejected() {
        let buf = build_action(8, 0, &[0u8; 8]);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_action_type_9_rejected() {
        let buf = build_action(9, 0, &[0u8; 8]);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_action_type_12_rejected() {
        let buf = build_action(12, 0, &[0u8; 32]);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_action_type_255_rejected() {
        let buf = build_action(255, 0, &[0u8; 8]);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_action_type_0_rejected() {
        let buf = build_action(0, 0, &[0u8; 8]);
        assert!(validate_actions_buffer(&buf).is_err());
    }

    // ─── Security: MAX_ACTIONS limit ──────────────────────────────────

    #[test]
    fn test_validate_max_actions_limit() {
        let mut buf = Vec::new();
        for i in 0..=MAX_ACTIONS {
            let mut prog = [0u8; 32];
            prog[0] = i as u8;
            buf.extend_from_slice(&build_action(10, 0, &prog)); // ProgramWhitelist
        }
        // 17 actions should fail (MAX_ACTIONS = 16)
        assert!(validate_actions_buffer(&buf).is_err());
    }

    #[test]
    fn test_validate_exactly_max_actions_ok() {
        let mut buf = Vec::new();
        for i in 0..MAX_ACTIONS {
            let mut prog = [0u8; 32];
            prog[0] = i as u8;
            buf.extend_from_slice(&build_action(10, 0, &prog));
        }
        // 16 actions should be fine
        assert!(validate_actions_buffer(&buf).is_ok());
    }

    // ─── Security: Trailing bytes ─────────────────────────────────────

    #[test]
    fn test_validate_trailing_bytes_rejected() {
        let mut buf = build_action(3, 0, &500_000u64.to_le_bytes());
        buf.push(0xFF); // trailing garbage byte
        // parse_actions should fail because the trailing byte doesn't form a valid header
        assert!(validate_actions_buffer(&buf).is_err());
    }
}
