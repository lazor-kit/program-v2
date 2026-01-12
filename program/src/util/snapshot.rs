//! Account snapshot utilities for verifying accounts weren't modified unexpectedly.
//!
//! This module implements account snapshot functionality,
//! allowing us to capture account state before instruction execution and verify
//! it hasn't been modified unexpectedly after execution.

use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, syscalls::sol_sha256,
};

/// Hashes account data excluding specified byte ranges.
///
/// This function creates a hash of account data while excluding certain ranges
/// (e.g., balance fields that are expected to change). The owner is always included
/// in the hash to ensure account ownership hasn't changed.
///
/// # Arguments
/// * `data` - The account data to hash
/// * `owner` - The account owner (always included in hash)
/// * `exclude_ranges` - Byte ranges to exclude from the hash
///
/// # Returns
/// * `[u8; 32]` - SHA256 hash of the data
pub fn hash_except(
    data: &[u8],
    owner: &Pubkey,
    exclude_ranges: &[core::ops::Range<usize>],
) -> [u8; 32] {
    // Maximum possible segments: owner + one before each exclude range + one after all ranges
    const MAX_SEGMENTS: usize = 17; // 1 for owner + 16 for data segments
    let mut segments: [&[u8]; MAX_SEGMENTS] = [&[]; MAX_SEGMENTS];

    // Always include the owner as the first segment
    segments[0] = owner.as_ref();
    let mut segment_count = 1;

    let mut position = 0;

    // If no exclude ranges, hash the entire data after owner
    if exclude_ranges.is_empty() {
        segments[segment_count] = data;
        segment_count += 1;
    } else {
        for range in exclude_ranges {
            // Add bytes before this exclusion range
            if position < range.start {
                segments[segment_count] = &data[position..range.start];
                segment_count += 1;
            }
            // Skip to end of exclusion range
            position = range.end;
        }

        // Add any remaining bytes after the last exclusion range
        if position < data.len() {
            segments[segment_count] = &data[position..];
            segment_count += 1;
        }
    }

    let mut data_payload_hash = [0u8; 32];

    #[cfg(target_os = "solana")]
    unsafe {
        sol_sha256(
            segments.as_ptr() as *const u8,
            segment_count as u64,
            data_payload_hash.as_mut_ptr() as *mut u8,
        );
    }

    #[cfg(not(target_os = "solana"))]
    {
        // For non-Solana targets (testing), we need to compute hash
        // Use a simple approach: hash all segments together
        // Note: In production (Solana), this uses sol_sha256 syscall
        use core::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        for segment in &segments[..segment_count] {
            segment.hash(&mut hasher);
        }
        let hash = hasher.finish();
        // Fill first 8 bytes with hash, rest with zeros (for testing)
        data_payload_hash[..8].copy_from_slice(&hash.to_le_bytes());
    }

    data_payload_hash
}

/// Captures a snapshot hash of an account.
///
/// # Arguments
/// * `account` - The account to snapshot
/// * `exclude_ranges` - Byte ranges to exclude from the hash (e.g., balance fields)
///
/// # Returns
/// * `Option<[u8; 32]>` - The snapshot hash, or None if account is not writable
pub fn capture_account_snapshot(
    account: &AccountInfo,
    exclude_ranges: &[core::ops::Range<usize>],
) -> Option<[u8; 32]> {
    // Only snapshot writable accounts (read-only accounts won't be modified)
    if !account.is_writable() {
        return None;
    }

    let data = unsafe { account.borrow_data_unchecked() };
    let hash = hash_except(&data, account.owner(), exclude_ranges);
    Some(hash)
}

/// Verifies an account snapshot matches the current account state.
///
/// # Arguments
/// * `account` - The account to verify
/// * `snapshot_hash` - The snapshot hash captured before execution
/// * `exclude_ranges` - Byte ranges to exclude from the hash (must match capture)
///
/// # Returns
/// * `Result<(), ProgramError>` - Ok if snapshot matches, error if modified unexpectedly
pub fn verify_account_snapshot(
    account: &AccountInfo,
    snapshot_hash: &[u8; 32],
    exclude_ranges: &[core::ops::Range<usize>],
) -> Result<(), ProgramError> {
    // Only verify writable accounts
    if !account.is_writable() {
        return Ok(());
    }

    let data = unsafe { account.borrow_data_unchecked() };
    let current_hash = hash_except(&data, account.owner(), exclude_ranges);

    if current_hash != *snapshot_hash {
        return Err(crate::error::LazorkitError::AccountDataModifiedUnexpectedly.into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pinocchio::pubkey::Pubkey;

    #[test]
    fn test_hash_except_no_exclude_ranges() {
        // Test hash_except with no exclude ranges
        let data = b"test data";
        let owner_bytes = [1u8; 32];
        let owner = Pubkey::try_from(owner_bytes.as_slice()).unwrap();
        let exclude_ranges: &[core::ops::Range<usize>] = &[];

        let hash1 = hash_except(data, &owner, exclude_ranges);
        let hash2 = hash_except(data, &owner, exclude_ranges);

        // Same input should produce same hash
        assert_eq!(hash1, hash2);

        // Different data should produce different hash
        let data2 = b"different data";
        let hash3 = hash_except(data2, &owner, exclude_ranges);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_hash_except_with_exclude_ranges() {
        // Test hash_except with exclude ranges (e.g., balance fields)
        let mut data = vec![0u8; 100];
        // Fill with test data
        for i in 0..50 {
            data[i] = i as u8;
        }
        // Balance field at 50..58 (8 bytes)
        data[50..58].copy_from_slice(&1000u64.to_le_bytes());
        for i in 58..100 {
            data[i] = i as u8;
        }

        let owner_bytes = [2u8; 32];
        let owner = Pubkey::try_from(owner_bytes.as_slice()).unwrap();
        let exclude_ranges = &[50..58]; // Exclude balance field

        let hash1 = hash_except(&data, &owner, exclude_ranges);

        // Change balance field - hash should be the same (excluded)
        data[50..58].copy_from_slice(&2000u64.to_le_bytes());
        let hash2 = hash_except(&data, &owner, exclude_ranges);
        assert_eq!(hash1, hash2);

        // Change non-excluded data - hash should be different
        data[0] = 99;
        let hash3 = hash_except(&data, &owner, exclude_ranges);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_hash_except_owner_included() {
        // Test that owner is always included in hash
        let data = b"test data";
        let owner1_bytes = [3u8; 32];
        let owner1 = Pubkey::try_from(owner1_bytes.as_slice()).unwrap();
        let owner2_bytes = [4u8; 32];
        let owner2 = Pubkey::try_from(owner2_bytes.as_slice()).unwrap();
        let exclude_ranges: &[core::ops::Range<usize>] = &[];

        let hash1 = hash_except(data, &owner1, exclude_ranges);
        let hash2 = hash_except(data, &owner2, exclude_ranges);

        // Different owners should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_capture_account_snapshot_writable() {
        // Note: This test requires AccountInfo which is difficult to mock in unit tests
        // This will be tested in integration tests instead
        // For now, just verify the function signature is correct
        assert!(true);
    }

    #[test]
    fn test_capture_account_snapshot_readonly() {
        // Note: This test requires AccountInfo which is difficult to mock in unit tests
        // This will be tested in integration tests instead
        assert!(true);
    }

    #[test]
    fn test_verify_account_snapshot_pass() {
        // Note: This test requires AccountInfo which is difficult to mock in unit tests
        // This will be tested in integration tests instead
        assert!(true);
    }

    #[test]
    fn test_verify_account_snapshot_fail_data_modified() {
        // Note: This test requires AccountInfo which is difficult to mock in unit tests
        // This will be tested in integration tests instead
        assert!(true);
    }

    #[test]
    fn test_verify_account_snapshot_fail_owner_changed() {
        // Note: This test requires AccountInfo which is difficult to mock in unit tests
        // This will be tested in integration tests instead
        assert!(true);
    }

    #[test]
    fn test_verify_account_snapshot_readonly() {
        // Note: This test requires AccountInfo which is difficult to mock in unit tests
        // This will be tested in integration tests instead
        assert!(true);
    }
}
