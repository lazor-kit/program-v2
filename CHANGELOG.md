# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Unified SDK API with discriminated union signer types (`ed25519()`, `secp256r1()`, `session()` helper constructors)
- `CreateWalletOwner` union type: single `createWallet()` method for both Ed25519 and Secp256r1
- `AdminSigner` union type for admin operations (addAuthority, removeAuthority, transferOwnership, createSession)
- `ExecuteSigner` union type for execute operations (includes session keys)
- `DeferredPayload` interface for clean authorize() -> executeDeferredFromPayload() flow
- Security test suite: 19 new tests across 3 files (permissions, session execution, attack vectors)
- Permission boundary tests: role enforcement for spender/admin/owner (error 3002 verification)
- Session execution tests: session key execute, transferSol via session, wrong key rejection, expiry enforcement
- Security edge case tests: counter increment verification, self-reentrancy prevention (error 3013), cross-wallet authority isolation, accounts hash binding (recipient swap detection)
- High-level `transferSol()` method: transfer SOL with just payer, wallet, signer, recipient, and amount
- High-level `execute()` method: execute arbitrary TransactionInstructions without manual compact encoding
- Auto-derivation of authority PDAs from signer.credentialIdHash (authorityPda now optional in Secp256r1 methods)
- Deferred Execution: 2-transaction flow for large payloads exceeding the ~574-byte limit of a single Secp256r1 Execute tx
- Authorize instruction (disc=6): TX1 signs over instruction/account hashes, creates DeferredExec PDA
- ExecuteDeferred instruction (disc=7): TX2 verifies hashes and executes via CPI with vault signing
- ReclaimDeferred instruction (disc=8): closes expired DeferredExec accounts, refunds rent to original payer
- DeferredExecAccount (176 bytes): stores instruction/account hashes, wallet, authority, payer, expiry
- RevokeSession instruction (disc=9): Owner/Admin can close session accounts early, refunding rent to specified destination
- Error code 3019 (InvalidSessionAccount) for invalid session PDA during revocation
- Devnet smoke test (`tests-sdk/tests/devnet-smoke.ts`): exercises all 9 instructions across Ed25519/Secp256r1/Session auth types and Owner/Admin/Spender roles, reporting CU/TX size/rent
- Deferred execution benchmarks (CU + tx size measurements for TX1/TX2)
- Error codes 3014-3018 for deferred execution (expired, hash mismatch, invalid expiry, unauthorized reclaim)
- SDK builders: `createAuthorizeIx`, `createExecuteDeferredIx`, `createReclaimDeferredIx`
- SDK helpers: `findDeferredExecPda`, `computeInstructionsHash`
- LazorKitClient methods: `authorize`, `executeDeferredFromPayload`, `reclaimDeferred`
- Odometer counter replay protection for Secp256r1 (monotonic u32 per authority)
- program_id included in challenge hash (cross-program replay prevention)
- rpId stored on authority account at creation (saves ~14 bytes per transaction)
- TypeScript SDK (`sdk/solita-client`) with Solita code generation
- Integration + security test suite (`tests-sdk/`) with 56 tests across 11 files
- Benchmark script for CU and transaction size measurements
- CompactInstructions accounts hash for anti-reordering protection
- Session expiry validation (future check + 30-day max duration)
- Self-removal and owner removal protection in RemoveAuthority
- AuthDataParser minimum 37-byte validation
- Signature offset validation in precompile introspection
- Dynamic message length check in precompile verification
- Instruction index restricted to 0xFFFF only (reject index 0)
- Comprehensive open-source documentation (Costs, Architecture, SDK API)
- SECURITY.md, CONTRIBUTING.md, CHANGELOG.md

### Changed

- SDK API: unified all methods via discriminated unions (breaking: removed `createWalletEd25519`, `createWalletSecp256r1`, `addAuthoritySecp256r1`, `removeAuthoritySecp256r1`, `executeEd25519`, `executeSecp256r1`, `executeSession`, `createSessionSecp256r1`, `transferOwnershipSecp256r1`, `authorizeSecp256r1`)
- SDK API: all methods now return `{ instructions: TransactionInstruction[]; ...extraPdas }` consistently
- SDK API: `createSession` now takes `sessionKey: PublicKey` instead of `Uint8Array`
- SDK architecture: split monolithic wrapper.ts into client.ts, types.ts, signing.ts, compact.ts
- Secp256r1 replay protection: primary mechanism changed from WebAuthn hardware counter to program-controlled odometer
- Auth payload layout: added 4-byte counter field at offset 8 (all subsequent fields shifted)
- Challenge hash: 5 elements -> 7 elements (added counter + program_id)
- AuthorityAccountHeader: added `counter` (u32) and `version` (u8) fields
- Secp256r1 pubkey storage: verified as 33-byte compressed format
- Authenticator trait: added `program_id` parameter
- Counter write timing: moved to after full signature verification
- Slot freshness: replaced SlotHashes sysvar with `Clock::get()` (removes 1 account from transaction)
- Counter size: u64 -> u32 (4 billion operations per authority is sufficient)
- Execute Secp256r1 transaction size: 708 -> 658 bytes (50 bytes saved)
- Execute Secp256r1 accounts: 8 -> 7 (SlotHashes sysvar removed)
- Cost documentation: updated all CU numbers from local validator to devnet-measured actuals, expanded CU table to all 9 instructions across all auth types and roles
- Shank IDL: fixed 4 instructions missing `rent_sysvar` accounts, added 3 missing deferred execution instructions (Authorize, ExecuteDeferred, ReclaimDeferred)

### Fixed

- Authorize signed payload now includes `expiry_offset` (66 bytes total), preventing relayers from modifying the expiry window
- `sol_assert_bytes_eq` now uses the `len` parameter instead of `left.len()` (latent OOB read on-chain)
- `reclaim_deferred` uses `checked_add` for lamports (consistent with `execute_deferred` and `manage_authority`)
- `PublicKey.default` collision with `SystemProgram.programId` in SDK execute methods: both are 32 zero bytes, causing `buildCompactLayout` to map SystemProgram to the sysvar slot (index 4) instead of adding it as a remaining account. Replaced with `SYSVAR_INSTRUCTIONS_PUBKEY`.
- Synced passkey lockout: WebAuthn hardware counter=0 no longer causes rejection
- 17/17 audit issues resolved (Accretion audit)

## [1.0.0] - 2025-01-01

### Added

- Initial release with Ed25519 and Secp256r1 authentication
- Role-Based Access Control (Owner, Admin, Spender)
- Ephemeral session keys with slot-based expiry
- CompactInstructions for Execute
- SlotHashes nonce for signature freshness (replaced by Clock::get() in v2)
- Zero-copy serialization via pinocchio
