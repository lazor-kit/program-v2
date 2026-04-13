# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Odometer counter replay protection for Secp256r1 (monotonic u32 per authority)
- program_id included in challenge hash (cross-program replay prevention)
- rpId stored on authority account at creation (saves ~14 bytes per transaction)
- TypeScript SDK (`sdk/solita-client`) with Solita code generation
- Integration test suite (`tests-sdk/`) with 28 tests across 7 files
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

### Fixed

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
