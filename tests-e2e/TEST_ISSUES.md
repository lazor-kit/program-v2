# E2E Test Issues

## Resolved Issues

### Issue #1: `failures.rs` Scenario 3 - Spender Privilege Escalation Test
**Status**: ✅ Fixed
**Fix**: Corrected account order (payer first) and used proper instruction data format.

### Issue #2: `failures.rs` Scenario 4 - Session Expiry
**Status**: ✅ Fixed
**Fix**: Updated CreateSession format, switched to slot-based expiry, and used `warp_to_slot` to ensure expiry.

### Issue #3: `failures.rs` Scenario 5 - Admin Permission Constraints
**Status**: ✅ Fixed
**Fix**: Corrected AddAuthority instruction data and account order.

### Issue #4: `cross_wallet_attacks.rs` Malformed Data
**Status**: ✅ Fixed
**Fix**: Corrected malformed `vec![1,1]` data to full `add_cross_data`.

### Issue #5: `cross_wallet_attacks.rs` Keypair Mismatch
**Status**: ✅ Fixed
**Fix**: Removed unused owner keypair from transaction signing to match instruction accounts.

### Issue #6 (DoS): System Program Create Account
**Status**: ✅ Fixed
**Fix**: Implemented Transfer-Allocate-Assign pattern in `utils.rs`. Verified by `dos_attack.rs`.

### Issue #7 (Rent Calc): Hardcoded Rent
**Status**: ✅ Fixed
**Fix**: Replaced hardcoded rent calculations with `Rent::minimum_balance(space)` in `create_wallet.rs` and `manage_authority.rs`. Verified by tests.

### Issue #8 (Validation): Wallet Discriminator Check
**Status**: ✅ Fixed
**Fix**: Added `wallet_data[0] == AccountDiscriminator::Wallet` check in `create_session.rs`, `manage_authority.rs`, `execute.rs`, and `transfer_ownership.rs`.

## Current Status
All E2E scenarios are PASSING.
- Happy Path
- Failures (5/5)
- Cross Wallet (3/3)
- DoS Attack
- Audit Validations
