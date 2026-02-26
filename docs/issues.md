
## #17 - OOB Read On Get Slot Hash

State: OPEN

### Description

We found the `get_slot_hash` incorrectly validates `index`, potentially allowing an out of bounds read when `index == slothash.len`.

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/auth/secp256r1/slothashes.rs#L76-L82


### Relevant Code

```rust
/// slothashes.rs L76-L82
    #[inline(always)]
    pub fn get_slot_hash(&self, index: usize) -> Result<&SlotHash, ProgramError> {
        if index > self.get_slothashes_len() as usize {
            return Err(AuthError::PermissionDenied.into()); // Mapping generic error for simplicity
        }
        unsafe { Ok(self.get_slot_hash_unchecked(index)) }
    }
```

### Mitigation Suggestion

Should be `if index >= self. get_slothashes_len() as usize {`

### Remediation

TODO: remediation with link to commit

---

## #16 - Old Nonces Can be Submitted Due To Truncation of Slot

State: OPEN

### Description

We found that due to slot truncation, old hashes can be submitted.
Slot 9050 will become valid in Slot 10050 again.

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/auth/secp256r1/nonce.rs#L28-L52


### Relevant Code

```rust
/// nonce.rs L28-L52
pub fn validate_nonce(
    slothashes_sysvar: &AccountInfo,
    submitted_slot: &TruncatedSlot,
) -> Result<[u8; 32], ProgramError> {
    // Ensure the program isn't being called via CPI
    if get_stack_height() > 1 {
        return Err(AuthError::PermissionDenied.into()); // Mapping CPINotAllowed error
    }

    let slothashes = SlotHashes::<Ref<[u8]>>::try_from(slothashes_sysvar)?;

    // Get current slothash (index 0)
    let most_recent_slot_hash = slothashes.get_slot_hash(0)?;
    let truncated_most_recent_slot = TruncatedSlot::new(most_recent_slot_hash.height);

    let index_difference = truncated_most_recent_slot.get_index_difference(submitted_slot);

    if index_difference >= 150 {
        return Err(AuthError::InvalidSignatureAge.into());
    }

    let slot_hash = slothashes.get_slot_hash(index_difference as usize)?;

    Ok(slot_hash.hash)
}
```

### Mitigation Suggestion

Validate the full slot hash.

### Remediation

TODO: remediation with link to commit

---

## #15 - System program Account isn't checked.

State: OPEN

### Description

We found that the program isn't checking the system program account anywhere, allowing us to spoof it.

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/create_wallet.rs#L230-L234


### Relevant Code

```rust
/// create_wallet.rs L230-L234
    let create_auth_ix = Instruction {
        program_id: system_program.key(),
        accounts: &auth_accounts_meta,
        data: &create_auth_ix_data,
    };
```

### Mitigation Suggestion

Check the system program id, or hardcode the Instruction program_id.

### Remediation

TODO: remediation with link to commit

---

## #14 - Missing Payer in Signed Payload Enables Rent Extraction in Ownership Transfer

State: OPEN

### Description

We found that `transfer_ownership` does not include the **payer** in the `signed_payload`, similar to issue #13. Because the payer is not bound by the signature, an attacker can replace it when submitting the transaction.

Attack scenario:

* The current owner is **auth type 1**.
* The new owner is **auth type 0**, which requires fewer lamports.
* The rent difference is refunded to the payer.
* An attacker supplies their own payer account and receives the refunded lamports.

This allows unauthorized rent extraction during ownership transfer.


### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/transfer_ownership.rs#L96-L98

### Relevant Code

```rust

```

### Mitigation Suggestion

Include the payer pubkey in the `signed_payload` so rent refunds are bound to the signer’s intent.

### Remediation

TODO: remediation with link to commit

---

## #13 - Missing Accounts in Signed Payload Enables Unauthorized Authority Removal and Rent Theft

State: OPEN

### Description

We found that `process_remove_authority` does not include `target_auth_pda` and `refund_dest` in the `signed_payload`. Because these accounts are not signed, a valid signature can be reused with different accounts.

As a result, an attacker can submit the same signature but replace:

* `target_auth_pda` with another user’s authority PDA (to delete it), and
* `refund_dest` with their own account (to receive the reclaimed rent).

This allows unauthorized deletion of authority records and rent theft.

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/manage_authority.rs#L299-L316

### Relevant Code

```rust
let data_payload = &[]; // Empty for remove


    let account_info_iter = &mut accounts.iter();
    let _payer = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let wallet_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let admin_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let target_auth_pda = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
    let refund_dest = account_info_iter
        .next()
        .ok_or(ProgramError::NotEnoughAccountKeys)?;
```

### Mitigation Suggestion

Include `target_auth_pda` and `refund_dest` pubkeys in the `signed_payload` so the signature is bound to the exact accounts being removed and refunded.

### Remediation

TODO: remediation with link to commit

---

## #12 - Secp256r1 Authority Layout Mismatch Can Break Validation

State: OPEN

### Description

We found an inconsistency in how Secp256r1 authority data is written vs read. When writing, the code inserts **four zero bytes** after the header, but when reading, those 4 bytes are not included in the offset calculation.

This makes the read logic interpret the wrong bytes as `rp_id_hash` (and later fields), which can cause failed validation or incorrect authority data parsing. The 4-byte prefix looks like a leftover or unfinished layout change (e.g., planned length/version field).

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/auth/secp256r1/mod.rs#L116

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/create_wallet.rs#L274-L275

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/transfer_ownership.rs#L234

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/manage_authority.rs#L265

### Relevant Code

```rust
let stored_rp_id_hash = &auth_data[header_size..header_size + 32];


    if args.authority_type == 1 {
        variable_target[0..4].copy_from_slice(&0u32.to_le_bytes());
```

### Mitigation Suggestion

Make read and write use the same fixed layout (either remove the extra 4 bytes or include them in the read offsets) and add a version/length field if the format is expected to evolve.


### Remediation

TODO: remediation with link to commit

---

## #11 - Lack of Inclusion of Accounts in Signed Payload

State: OPEN

### Description

We found that in the `execute` instruction, the signed payload only binds to the **index of accounts**, not the **exact account addresses**. Because of this, an attacker can reorder accounts OR submit any account in the transaction while still using a valid signature.

Example:

* User signs a payload intending:

  * Transfer 1 token to **UserA**
  * Transfer 100 tokens to **UserB**
* Accounts are signed by index: `[UserA, UserB]`
* An attacker submits the transaction with accounts reordered: `[UserB, UserA]`

As a result, the transfers execute with swapped recipients, causing unintended fund movement.

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/execute.rs#L109

### Relevant Code

```rust

```

### Mitigation Suggestion

Include the **full account pubkeys** alongside the index, = in the signed payload instead of only account indices, so reordering accounts invalidates the signature.


### Remediation

TODO: remediation with link to commit

---

## #10 - Unintended Self-Reentrancy Risk

State: OPEN

### Description

Solana allows programs to invoke themselves via CPI (self-reentrancy), which may be risky if not explicitly
accounted for. While the current utilization of a counter appears safe and unaffected, reentrancy may
introduce unexpected behavior in future changes. Thus, it will be appropriate to proactively disable
self-reentrancy unless it is an intentional design feature.

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/execute.rs#L184

### Relevant Code

```rust

```

### Mitigation Suggestion

Disable re-entrancy from the CPIs.

### Remediation

TODO: remediation with link to commit

---

## #9 - Secp256r1 Authenticator Allows Anyone to Submit Valid Signatures

State: OPEN

### Description

We found that `Secp256r1Authenticator` allows **anyone** to submit a transaction as long as they provide valid signature data. While this is not a security issue by itself, it weakens control if there is any mistake in the signed payload (for example, missing fields like in issue #8 or #11 ). In such cases, other users could reuse the same signature, leading to unintended execution.


### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/auth/secp256r1/mod.rs#L33

### Relevant Code

```rust

```

### Mitigation Suggestion

Bind the signature to a specific on-chain signer (e.g., require a signer account) so signatures cannot be reused by others.
Signer could be an account that is checked to be the signer and included in the `signed_payload`.

### Remediation

TODO: remediation with link to commit

---

## #8 - Missing Discriminator in Signed Payload Enables Signature Replay Across Instructions

State: OPEN

### Description

We found that the current implementation does not include the instruction discriminator (or any domain separator) in `signed_payload`. Because of this, a signature created for one instruction could potentially be used for a different instruction that builds the same payload format. This weakens authorization and can enable cross-instruction replay.


### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/create_session.rs#L150



### Relevant Code

```rust

```

### Mitigation Suggestion

Include the instruction discriminator (or a fixed domain separator string like `"create_session"`) in `signed_payload` so signatures are bound to a single instruction.

### Remediation

TODO: remediation with link to commit

---

## #7 - Wallet Validation Skips Discriminator Check

State: OPEN

### Description

We found that wallet validation only checks the owner and does not verify the account discriminator. While this does not create an immediate issue, it allows other account types owned by the same program to potentially pass wallet checks, which is not intended.

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/create_session.rs#L92-L94

### Relevant Code

```rust
if wallet_pda.owner() != program_id || authorizer_pda.owner() != program_id {
        return Err(ProgramError::IllegalOwner);
    }
```

### Mitigation Suggestion

Also validate the wallet account discriminator to ensure only real wallet accounts are accepted.


### Remediation

TODO: remediation with link to commit

---

## #6 - General Notes

State: OPEN

### Description

- [ ] N1: We found that in `create_wallet`, the user-provided auth_bump is ignored and not used. 
https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/create_wallet.rs#L29
- [ ] N2: We found that `system_program` not checked to be correct `system_program`.
- [ ] N3: We found that the hash of the user-provided RP ID is not checked against the stored RP ID hash or the RP ID hash in the auth payload. While this does not cause an issue since it is used in the client data hash, it is still better to explicitly check it.

---

## #5 - Hardcoded rent calculations might go out of sync after chain update.

State: OPEN

### Description

We found that the multiple instruction hardcodes rent calculations instead of using the Rent `sysvar`


### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/create_wallet.rs#L206-L210
https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/create_wallet.rs#L135-L141


### Relevant Code

```rust
/// create_wallet.rs L135-L141
    // 897840 + (space * 6960)
    let rent_base = 897840u64;
    let rent_per_byte = 6960u64;
    let wallet_rent = (wallet_space as u64)
        .checked_mul(rent_per_byte)
        .and_then(|val| val.checked_add(rent_base))
        .ok_or(ProgramError::ArithmeticOverflow)?;
/// create_wallet.rs L206-L210
    // Rent calculation: 897840 + (space * 6960)
    let auth_rent = (auth_space as u64)
        .checked_mul(6960)
        .and_then(|val| val.checked_add(897840))
        .ok_or(ProgramError::ArithmeticOverflow)?;
```

### Mitigation Suggestion

Use the Rent sysvar to calculate the correct amount of rent


### Remediation

TODO: remediation with link to commit

---

## #4 - System Program Create Account Usage Leads to Lamport Transfer DoS

State: OPEN

### Description

We found that the program calls the System program's `create_account` instruction to initialize new accounts without checking the account's exising lamports. The System program's `create_account` instruction will fail and return an error when it tries to create an account which already contains any amount of lamports, which is a problem because anyone may transfer a small amount of lamports to the account to be created, effectively preventing the creation using `create_account`.

### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/create_wallet.rs#L143-L179
https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/manage_authority.rs#L206-L247


### Relevant Code

```rust
/// create_wallet.rs L143-L179
    let mut create_wallet_ix_data = Vec::with_capacity(52);
    create_wallet_ix_data.extend_from_slice(&0u32.to_le_bytes());
    create_wallet_ix_data.extend_from_slice(&wallet_rent.to_le_bytes());
    create_wallet_ix_data.extend_from_slice(&(wallet_space as u64).to_le_bytes());
    create_wallet_ix_data.extend_from_slice(program_id.as_ref());

    let wallet_accounts_meta = [
        AccountMeta {
            pubkey: payer.key(),
            is_signer: true,
            is_writable: true,
        },
        AccountMeta {
            pubkey: wallet_pda.key(),
            is_signer: true, // Must be true even with invoke_signed
            is_writable: true,
        },
    ];
    let create_wallet_ix = Instruction {
        program_id: system_program.key(),
        accounts: &wallet_accounts_meta,
        data: &create_wallet_ix_data,
    };
    let wallet_bump_arr = [wallet_bump];
    let wallet_seeds = [
        Seed::from(b"wallet"),
        Seed::from(&args.user_seed),
        Seed::from(&wallet_bump_arr),
    ];
    let wallet_signer: Signer = (&wallet_seeds).into();

    invoke_signed(
        &create_wallet_ix,
        &[&payer.clone(), &wallet_pda.clone(), &system_program.clone()],
        &[wallet_signer],
    )?;
```

### Mitigation Suggestion

To mitigate the issue, apply the manual transfer-allocate-assign pattern. First, transfer the required lamport amount to achieve rent exemption. This amount may be 0 if the account already has at least the amount of lamports required for the intended allocation size. Then, allocate the amount of bytes required and assign the account to the intended program.

### Remediation

TODO: remediation with link to commit

---

## #3 - RemoveAuthority Allows Cross-Wallet Authority Deletion

State: OPEN

### Description

We found that in In `manage_authority.rs:process_remove_authority`, when an **Owner** (role 0) removes an authority, the code does NOT verify that the target authority belongs to the same wallet. The entire authorization block is skipped when `admin_header.role == 0`:


### Location

https://github.com/lazor-kit/program-v2/blob/cd09588d2459571ceb6fe7bd8764abb139b6d3de/program/src/processor/manage_authority.rs#L377-L385


### Relevant Code

```rust
// Authorization
if admin_header.role != 0 {
    // Only enters this block if NOT owner
    let target_data = unsafe { target_auth_pda.borrow_data_unchecked() };
    // ... reads target_header ...
    if target_header.discriminator != AccountDiscriminator::Authority as u8 {
        return Err(ProgramError::InvalidAccountData);
    }
    if admin_header.role != 1 || target_header.role != 2 {
        return Err(AuthError::PermissionDenied.into());
    }
}
```

### Mitigation Suggestion
```
// ALWAYS read and verify target header
let target_data = unsafe { target_auth_pda.borrow_data_unchecked() };
let target_header = // ... parse header ...

// ALWAYS verify target belongs to this wallet
if target_header.wallet != *wallet_pda.key() {
    return Err(ProgramError::InvalidAccountData);
}
if target_header.discriminator != AccountDiscriminator::Authority as u8 {
    return Err(ProgramError::InvalidAccountData);
}

// Then check role-based permissions
if admin_header.role != 0 {
    if admin_header.role != 1 || target_header.role != 2 {
        return Err(AuthError::PermissionDenied.into());
    }
}
```

### Remediation

_No response_

---

## #2 - AUDIT PROGRESS TRACKER: mahdi

State: OPEN

### Description

- [x] Project Set Up: Download Project code build it locally, run tests
- [x] Project Preparation: Read documentation, use product, understand what they're building
- [x] Familiarize with the codebase
- src/
- [x] compact.rs
- [x] entrypoint.rs
- [x] error.rs
- [x] instructions.rs
- [x] lib.rs
- [x] utils.rs
- src/processor/
- [x] create_wallet.rs
- [x] create_session.rs
- [x] execute.rs
- [x] manage_authority.rs
- [x] transfer_ownership.rs
- src/state/
- [x] authority.rs
- [x] session.rs
- [x] wallet.rs
- src/auth/
- [x] ed25519.rs
- src/auth/secp256r1/
- [x] introspection.rs
- [x] nonce.rs
- [x] slothashes.rs
- [x] webauthn.rs
- [x] Review Common Vulnerabilities Checklist
- [x] Review Overall Design
- [ ] Review all Fixes for reported Vulnerabilities


---

## #1 - AUDIT PROGRESS TRACKER: brymko

State: OPEN

### Description

- [ ] Project Set Up: Download Project code build it locally, run tests
- [ ] Project Preparation: Read documentation, use product, understand what they're building
- [ ] Familiarize with the codebase
- [ ] program/src/processor/
  - [ ] create_wallet.rs
  - [ ] create_session.rs
  - [ ] execute.rs
  - [ ] manage_authority.rs
  - [ ] transfer_ownership.rs
- [ ] Review Common Vulnerabilities Checklist
- [ ] Review Overall Design
- [ ] Review all Fixes for reported Vulnerabilities



---
