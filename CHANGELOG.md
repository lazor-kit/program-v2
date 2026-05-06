# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- End-to-end vitest tests for session-action enforcement (`tests-sdk/tests/12-actions.test.ts`, 9 cases): `programWhitelist` allow + reject (3021), `programBlacklist` allow + reject (3022), `solMaxPerTx` allow at-cap + reject over-cap (3023), `solLimit` lifetime budget exhaustion (3024), and combined-rules enforcement. Runs against a live `solana-test-validator` with the foundation binary loaded and uses `@lazorkit/sdk-legacy`'s `Actions` builder to dogfood the full encode → on-chain enforce path.
- `docs/audit/` artifacts for an Accretion delta-audit follow-up: `DELTA_BRIEF.md` summarises the changes from the previous audited baseline by phase with explicit audit asks; `program-src.diff` is the full unified diff of `program/`; `program-src.diff.stat` is a per-file changed-line summary; `upstream-parity.txt` reports byte-identity vs the already-audited `lazorkit-protocol` per file (13/19 changed files identical).
- Local git tags `audit-baseline-2026-02-accretion` (previous Accretion-audited state, commit `d1eaaeb`) and `audit-pending-v1` (the current consolidated state ready for delta review).
- Session action permissions: 8 immutable permission rules attachable at session creation — `SolLimit`, `SolRecurringLimit`, `SolMaxPerTx`, `TokenLimit`, `TokenRecurringLimit`, `TokenMaxPerTx`, `ProgramWhitelist`, `ProgramBlacklist`. Action discriminators (1, 2, 3, 4, 5, 6, 10, 11) and the 11-byte header layout match `lazorkit-protocol` so the unified SDK can encode actions identically for both builds.
- `SessionAccount` is now variable-size: a session can carry a trailing action buffer (max 16 actions, ≤ 2048 bytes) validated at creation time.
- `CreateSession` instruction data accepts the new `[actions_len: u16][actions: N]` extension after the legacy 40-byte args; old 40-byte clients continue to work via the legacy parser branch.
- Pre-CPI action enforcement at `Execute` time: program whitelist/blacklist checks against each CPI target.
- Post-CPI action enforcement: SOL/token spending caps with saturating arithmetic; recurring-window resets aligned to slot boundaries; per-execute SOL outflow tracked across all CPIs for `SolMaxPerTx`.
- Vault-invariant defenses against `System::Assign` / `SetAuthority` / `Approve` escapes: vault owner + data-length snapshotted pre-CPI and verified unchanged post-CPI; vault-owned token accounts on listed mints have their owner / delegate / close_authority fields snapshotted and verified.
- Anti-CPI guard for session-authenticated `Execute`: stack-height must be 1 (rejects wrapper programs chaining through `Execute`).
- Error codes 3020–3029 (action validation + enforcement) and 3030–3032 (`SessionVaultOwnerChanged`, `SessionVaultDataLenChanged`, `SessionTokenAuthorityChanged`).
- Dual-cluster Cargo features (`mainnet`, `devnet`): the embedded program ID is chosen at compile time via a feature flag with a `compile_error!` if neither / both is set. The `mainnet` feature embeds `LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi` (same slot as `lazorkit-protocol`) for the foundation deployment; `devnet` keeps `FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao`.
- `security.txt` block embedded via `solana-security-txt` macro: links to SECURITY.md, contact email, source repo, source revision (from `GITHUB_SHA`), and the Accretion audit PDF.
- Zero-copy `CompactInstructionRef` parser (`parse_compact_instructions_ref_with_len`) used by the Execute hot path — no per-instruction `Vec<u8>` allocations for account-index bytes or instruction data.
- Cherry-pick guardrails: `scripts/fee-paths.txt` declares forbidden fee-surface paths and symbols, `scripts/check-no-fee.sh` verifies the working tree (used by CI), `scripts/strip-fee.sh` auto-removes fee files post-cherry-pick.
- CI workflow `check-no-fee` runs the verifier on every PR.
- CI workflow `sbf-cluster-check` builds both mainnet and devnet SBF binaries, verifies their hashes differ, and asserts that an unflagged `cargo build-sbf` fails with the expected `compile_error!`.
- `scripts/build-all.sh <devnet|mainnet>` now drives a feature-flagged build + IDL regen + SDK regen in one step. The previous `scripts/sync-program-id.sh` is removed (program ID is now a compile-time feature, not a sed target).
- `solana-security-txt` and `default-env` dependencies, `[workspace.metadata.cli]` pinning Solana CLI 3.0.4 for verified builds.
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
- TypeScript SDK: standardised on `@lazorkit/sdk-legacy` (lives in sibling `lazorkit-protocol` repo); the in-tree `sdk/solita-client` has been removed
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

- Secp256r1 auth payload format: replaces the older `typeAndFlags` byte at `auth_payload[13]` with full raw `clientDataJSON` embedded in the payload. The on-chain auth verifier now parses the JSON directly rather than reconstructing it from `typeAndFlags + rpId`. Aligns with `lazorkit-protocol` byte-for-byte and is required for binary-swap compatibility at the shared mainnet slot.
- Secp256r1 authority on-chain layout: replaces the previously stored variable-length raw `rpId` with a precomputed 32-byte `rpIdHash` (SHA-256 digest computed at registration). New layout: `header(48) + cred_hash(32) + pubkey(33) + rpIdHash(32) = 145 bytes`. Saves one `sol_sha256` syscall per `Execute`. Existing wallets created on the upstream commercial binary remain readable after binary swap.
- Shank IDL declarations on the `ProgramIx` enum (account metadata: `writable` modifiers, account positions, descriptions) resynced with `lazorkit-protocol`. Five fee-related variants (disc 10–14: `InitializeProtocol`, `UpdateProtocol`, `RegisterPayer`, `WithdrawTreasury`, `InitializeTreasuryShard`) stripped — `program-v2` keeps disc 0–9 only. Runtime not affected (`@lazorkit/sdk-legacy` uses hand-written builders rather than the generated IDL).
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

- `tests-sdk` integration tests now pass `PROGRAM_ID` explicitly to the `LazorKitClient` constructor. `@lazorkit/sdk-legacy`'s URL-based program-ID inference defaulted localhost to the commercial devnet ID (`4h3X…`); against a local validator loading the foundation binary at the keypair's pubkey this caused all txs to fail with "Attempt to load a program that does not exist". `tests/common.ts` now resolves `PROGRAM_ID` from (1) `PROGRAM_ID` env override, (2) the keypair file at `target/deploy/lazorkit_program-keypair.json`, or (3) the foundation devnet fallback `FLb7…`.
- `tests-sdk/tests/08-deferred.test.ts` builds the `Authorize` `signed_payload` as `instructions_hash || accounts_hash || expiry_offset (u16 LE)` to match what the on-chain verifier hashes. The test code was missing the 2-byte expiry buffer at all 6 sign sites, causing all 7 deferred tests to fail with `InvalidMessageHash` (3005). After the fix, all 65 vitest E2E tests pass against a live validator.
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
