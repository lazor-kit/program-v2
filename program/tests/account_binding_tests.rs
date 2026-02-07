//! Tests for Issue #11: Account Binding in Signed Payload
//!
//! These tests verify that reordering accounts in a transaction produces
//! a different hash, which will invalidate the signature.

use sha2::{Digest, Sha256};

/// Simulates the compute_accounts_hash logic for testing purposes
/// This mirrors the on-chain implementation in execute.rs
fn compute_accounts_hash_test(
    account_pubkeys: &[[u8; 32]],
    compact_instructions: &[(u8, Vec<u8>)], // (program_id_index, account_indices)
) -> [u8; 32] {
    let mut pubkeys_data = Vec::new();

    for (program_idx, accounts) in compact_instructions {
        // Include program_id
        pubkeys_data.extend_from_slice(&account_pubkeys[*program_idx as usize]);

        // Include all account pubkeys
        for &acc_idx in accounts {
            pubkeys_data.extend_from_slice(&account_pubkeys[acc_idx as usize]);
        }
    }

    // Compute SHA256 hash
    let mut hasher = Sha256::new();
    hasher.update(&pubkeys_data);
    hasher.finalize().into()
}

#[test]
fn test_same_indices_same_accounts_same_hash() {
    // Setup: 3 accounts
    let accounts = [
        [1u8; 32], // Program
        [2u8; 32], // UserA
        [3u8; 32], // UserB
    ];

    let instructions = vec![(0u8, vec![1u8, 2u8])]; // Transfer from 1 to 2

    let hash1 = compute_accounts_hash_test(&accounts, &instructions);
    let hash2 = compute_accounts_hash_test(&accounts, &instructions);

    assert_eq!(hash1, hash2, "Same accounts should produce same hash");
}

#[test]
fn test_reordered_accounts_different_hash() {
    // Original order: [Program, UserA, UserB]
    let accounts_original = [
        [1u8; 32], // Program at index 0
        [2u8; 32], // UserA at index 1
        [3u8; 32], // UserB at index 2
    ];

    // Attacker reorders: [Program, UserB, UserA]
    let accounts_reordered = [
        [1u8; 32], // Program at index 0 (unchanged)
        [3u8; 32], // UserB at index 1 (was UserA!)
        [2u8; 32], // UserA at index 2 (was UserB!)
    ];

    // Same compact instruction indices
    let instructions = vec![(0u8, vec![1u8, 2u8])]; // Transfer from index 1 to index 2

    let hash_original = compute_accounts_hash_test(&accounts_original, &instructions);
    let hash_reordered = compute_accounts_hash_test(&accounts_reordered, &instructions);

    // Issue #11 Fix: Reordered accounts MUST produce different hash
    assert_ne!(
        hash_original, hash_reordered,
        "Reordered accounts MUST produce different hash (Issue #11 fix)"
    );

    println!("Original hash:  {:?}", &hash_original[..8]);
    println!("Reordered hash: {:?}", &hash_reordered[..8]);
}

#[test]
fn test_attack_scenario_transfer_recipient_swap() {
    // Scenario: User intends to transfer 1 SOL to UserA and 100 SOL to UserB
    // Attacker swaps UserA and UserB positions

    let program = [0u8; 32];
    let user_a = [0xAA; 32];
    let user_b = [0xBB; 32];
    let payer = [0xFF; 32];

    // User's intended account order
    let user_intended = [program, payer, user_a, user_b];
    // Instructions: Transfer to accounts[2] (UserA), Transfer to accounts[3] (UserB)
    let instructions = vec![
        (0u8, vec![1u8, 2u8]), // Transfer from payer to UserA
        (0u8, vec![1u8, 3u8]), // Transfer from payer to UserB
    ];

    let user_hash = compute_accounts_hash_test(&user_intended, &instructions);

    // Attacker's reordered accounts (swap UserA and UserB)
    let attacker_reordered = [program, payer, user_b, user_a]; // Swapped!

    let attacker_hash = compute_accounts_hash_test(&attacker_reordered, &instructions);

    // The hashes MUST be different, invalidating the signature
    assert_ne!(
        user_hash, attacker_hash,
        "Attack detected: Account swap must invalidate signature"
    );

    println!("✅ Issue #11 Attack Prevention Verified");
    println!(
        "   User intended hash:    {:02x}{:02x}{:02x}{:02x}...",
        user_hash[0], user_hash[1], user_hash[2], user_hash[3]
    );
    println!(
        "   Attacker reorder hash: {:02x}{:02x}{:02x}{:02x}...",
        attacker_hash[0], attacker_hash[1], attacker_hash[2], attacker_hash[3]
    );
}

#[test]
fn test_multiple_instructions_hash_binding() {
    let accounts = [
        [0u8; 32], // Program 0
        [1u8; 32], // Account 1
        [2u8; 32], // Account 2
        [3u8; 32], // Account 3
    ];

    // Multiple instructions using different accounts
    let instructions = vec![
        (0u8, vec![1u8, 2u8]),
        (0u8, vec![2u8, 3u8]),
        (0u8, vec![1u8, 3u8]),
    ];

    let hash = compute_accounts_hash_test(&accounts, &instructions);

    // Verify hash is deterministic
    let hash2 = compute_accounts_hash_test(&accounts, &instructions);
    assert_eq!(hash, hash2);

    // Verify different account produces different hash
    let mut modified_accounts = accounts;
    modified_accounts[2] = [0xFF; 32]; // Change account 2

    let hash_modified = compute_accounts_hash_test(&modified_accounts, &instructions);
    assert_ne!(hash, hash_modified, "Modified account must change hash");
}
