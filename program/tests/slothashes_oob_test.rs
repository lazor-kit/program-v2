use lazorkit_program::auth::secp256r1::slothashes::SlotHashes;

#[test]
fn test_slot_hashes_oob_read() {
    // 1. Setup Mock Data
    // num_entries: 2 (u64)
    // entry 0: slot 100, hash [1; 32]
    // entry 1: slot 99, hash [2; 32]
    let mut data = Vec::new();
    data.extend_from_slice(&2u64.to_le_bytes()); // len = 2

    // Entry 0
    data.extend_from_slice(&100u64.to_le_bytes());
    data.extend_from_slice(&[1u8; 32]);

    // Entry 1
    data.extend_from_slice(&99u64.to_le_bytes());
    data.extend_from_slice(&[2u8; 32]);

    // Interpret data as &[u8]
    let data_slice: &[u8] = &data;

    // Safety: we constructed data correctly
    let slot_hashes = unsafe { SlotHashes::new_unchecked(data_slice) };

    // 2. Verify Valid Access
    let hash_0 = slot_hashes.get_slot_hash(0).unwrap();
    assert_eq!(hash_0.height, 100);

    let hash_1 = slot_hashes.get_slot_hash(1).unwrap();
    assert_eq!(hash_1.height, 99);

    // 3. Verify OOB Access (The Bug)
    println!("Trying to access OOB index 2...");
    // This call accesses index 2.
    // Length is 2.
    // Current Buggy Logic: 2 > 2 is FALSE.
    // So it PROCEEDS to unsafe code and returns Ok or Panics.
    // We expect it to be Err(PermissionDenied).

    let result = slot_hashes.get_slot_hash(2);

    // If the bug is present, result.is_ok() will be true (or panic).
    // If fixed, result.is_err() will be true.
    assert!(
        result.is_err(),
        "Index equal to length should be an error! (OOB Read)"
    );
}
