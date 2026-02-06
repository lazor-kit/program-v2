# E2E Test Issues

## Issue #1: `failures.rs` Scenario 3 - Spender Privilege Escalation Test

**Status**: 🔴 Broken  
**Priority**: Medium  
**File**: `tests-e2e/src/scenarios/failures.rs` (lines 225-290)

### Problem
Test has incorrect logic - mixes CreateSession and AddAuthority concepts:
- Creates `session_pda` and `session_auth_pda` with session keypair seeds
- But uses AddAuthority instruction discriminator `[1, 3]`
- Account list doesn't match either CreateSession or AddAuthority expected format
- Error: `InvalidInstructionData` (consumed only 113 compute units)

### Root Cause
Test was likely written during refactoring and instruction formats changed. The test:
1. Sets up PDAs for session creation
2. But instruction data is for AddAuthority (discriminator 1)
3. Account order is wrong for both instructions

### Fix Required
Decide what this test should actually verify:
- **Option A**: Test that spender cannot call AddAuthority → fix instruction data and accounts
- **Option B**: Test that expired session cannot be used → rewrite as proper CreateSession + Execute flow

### Affected Code
```rust
// Line 268-272 - Instruction data is mixed up
data: [
    vec![1, 3],                         // AddAuthority(Session) ← WRONG
    (now - 100).to_le_bytes().to_vec(), // Expires in past
]
.concat(),
```

---

## Issue #2: Other Skipped Tests Not Running

Tests after scenario 3 failure are skipped:
- Cross Wallet Attacks
- DoS Attack  
- Audit Validations

These should run after fixing Issue #1.
