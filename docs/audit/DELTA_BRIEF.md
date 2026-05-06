# Accretion Audit Delta Brief — `program-v2` `audit-baseline-2026-02-accretion` → `audit-pending-v1`

**Repository:** `lazor-kit/program-v2`
**Previous audit:** Accretion Labs, February 2026, A26SFR1
([audits/2026-accretion-solana-foundation-lazorkit-audit-A26SFR1.pdf](../../audits/2026-accretion-solana-foundation-lazorkit-audit-A26SFR1.pdf))
**Previous baseline tag:** `audit-baseline-2026-02-accretion` → commit `d1eaaeb` (Merge PR #49 fix/audit-hardening, 17/17 findings resolved)
**Delta tag:** `audit-pending-v1` → commit `9c97fe2`
**Delta scope:** 21 files in `program/`, +4168 / −444 lines

This delta brief is for an **Accretion follow-up engagement** to confirm the
changes between the two tags introduce no new vulnerabilities versus the prior
audited state. Most of the new code is **byte-identical with the
already-audited `lazorkit-protocol` repo** (commercial sibling under the same
audit engagement); this is documented per-file in
[upstream-parity.txt](./upstream-parity.txt).

---

## Strategic context (deploy plan)

`program-v2` ships at the **same mainnet program ID** as `lazorkit-protocol`
(`LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi`). The foundation contract
period uses the `program-v2` (no-fee) build at that slot; afterwards, the
upgrade authority swaps the binary at the same slot to the `lazorkit-protocol`
(commercial, with-fee) build.

**Implication for audit scope:** binary-swap compatibility requires that
program-v2 and lazorkit-protocol share identical:
- Account layouts (Wallet, Authority, Session, DeferredExec — verified
  byte-identical via `state/*.rs` ↔ upstream)
- Instruction encoding (discriminators 0–9, account orderings — kept aligned
  via the IDL sync in P5.4)
- Auth verification logic (auth/secp256r1/* — byte-identical with upstream
  after P5.1)

Audit confirmation of this delta therefore also benefits the slot-share
strategy: any wallet/authority/session created on either binary remains
verifiable by the other after swap.

---

## Delta by phase

The 23 commits between the two tags group into 5 phases:

### P0 — Cherry-pick guardrails (zero-audit, mechanical)

Tooling-only. Adds `scripts/strip-fee.sh`, `scripts/check-no-fee.sh`,
`scripts/fee-paths.txt`, and `.github/workflows/check-no-fee.yml` to enforce
that fee/admin/FeeRecord surface from `lazorkit-protocol` cannot leak into
`program-v2` during cherry-picks. **No `program/` source changed.**

### P1 — Action types port + execute enforcement (delta-audit)

**Most security-relevant section of the delta.** New on-chain feature.

Files touched:
- `program/src/state/action.rs` (NEW, 697 lines) — byte-identical with
  upstream `lazorkit-protocol/program/src/state/action.rs` (already audited).
  Defines 8 action types + parser + validator.
- `program/src/state/session.rs` — minor (+23 lines) — adds
  `SESSION_HEADER_SIZE` const and `actions_slice()` helper. Byte-identical
  with upstream.
- `program/src/state/mod.rs` — adds `pub mod action;`.
- `program/src/processor/create_session.rs` — adopts variable-size session PDA
  for optional actions buffer + validation at creation. Byte-identical with
  upstream.
- `program/src/processor/execute_actions.rs` (NEW, 1644 lines) —
  byte-identical with upstream `lazorkit-protocol/program/src/processor/execute/actions.rs`.
  Pre-CPI program whitelist/blacklist + token snapshots; post-CPI delta
  computation, SOL/token cap enforcement, recurring window resets, vault
  invariant defenses against System::Assign / SetAuthority escapes.
- `program/src/processor/execute.rs` — wires pre/post action evaluation
  around the CPI loop, adds the L5 anti-CPI guard for sessions, gross SOL
  outflow tracking. Differs from upstream `processor/execute/immediate.rs`
  by **1 import-path line** (`processor::execute_actions::` vs
  `processor::execute::actions::`) due to flat vs nested processor layout.
- `program/src/error.rs` — adds 13 error variants (3020–3032) for action
  validation, enforcement, and vault invariant defense.

**Audit ask (P1):** Confirm action enforcement engine + execute integration
introduce no new vulnerabilities vs. the audited upstream version.

### P2 — Dual-cluster + security_txt (zero-audit, mechanical)

- `program/src/lib.rs` — embeds `security_txt!` with program-v2-specific URLs
  (only difference vs upstream is the URL strings — see `upstream-parity.txt`)
- `assertions/src/lib.rs`, `assertions/Cargo.toml`, `program/Cargo.toml` —
  Pattern D feature flags: `--features mainnet` embeds `LazorjRF…` (slot
  shared with upstream); `--features devnet` embeds `FLb7…`; no feature →
  `compile_error!` (prevents accidental cross-cluster deploy)

No on-chain logic change. Build-time configuration only.

### P3 — SDK consolidation (no audit)

Off-chain only. `program-v2/sdk/solita-client/` deleted; `program-v2/tests-sdk/`
migrated to `@lazorkit/sdk-legacy` from npm (the same SDK
`lazorkit-protocol` ships).

### P4 — Release infrastructure (no audit)

`CHANGELOG.md`, `docs/MAINNET_DEPLOY_RUNBOOK.md`, `.github/workflows/release.yml`.
Documentation + CI only.

### P5 — Consolidate: auth + processor port + IDL sync (delta-audit)

**Aligns the remaining processor + auth files with upstream so the slot-share
strategy works end-to-end.**

Files (all byte-identical with upstream after this phase, except where noted):
- `program/src/auth/secp256r1/{mod,webauthn,introspection}.rs` — port from
  upstream. Replaces older typeAndFlags-format auth (which reconstructed
  clientDataJSON server-side) with the format that embeds full raw
  clientDataJSON in the auth payload. **Byte-identical with upstream.**
- `program/src/processor/{create_wallet,manage_authority,transfer_ownership,
  execute_deferred,revoke_session}.rs` — port from upstream. Brings authority
  data layout to:
  `Secp256r1 authority = header(48) + cred_hash(32) + pubkey(33) + rpIdHash(32) = 145B`
  (previously variable-length raw rpId; now precomputed SHA256 digest at
  offset 113, saves one syscall per Execute). **Byte-identical with upstream.**
- `program/src/instruction.rs` — Shank IDL declarations resynced from
  upstream (account metadata: writable modifiers, positions, descriptions);
  5 fee instruction variants (disc 10–14) stripped. Runtime not affected
  (sdk-legacy uses hand-written builders, not generated IDL).

**Audit ask (P5):** Since these files are byte-identical with upstream,
confirm Accretion's prior review of the upstream files extends to this
program. Specifically:
- Auth payload format change (typeAndFlags → embedded clientDataJSON) — was
  this reviewed in the upstream audit? Any concerns specific to program-v2
  context?
- Authority layout change (raw rpId → rpIdHash) — same question.

---

## Files NOT changed since baseline (sanity)

- `program/src/auth/ed25519.rs` — unchanged
- `program/src/auth/mod.rs` — unchanged
- `program/src/auth/traits.rs` — unchanged
- `program/src/utils.rs` — unchanged
- `program/src/state/wallet.rs` — unchanged (verified byte-identical with
  upstream — wallet account layout is the cross-binary contract for
  slot-share)
- `program/src/state/authority.rs` — unchanged (same reasoning as wallet)
- `program/src/state/deferred.rs` — unchanged
- `program/src/processor/authorize.rs` — unchanged
- `program/src/processor/reclaim_deferred.rs` — unchanged
- `program/src/entrypoint.rs` — unchanged dispatch (still discs 0–9)

---

## Strip surface — what's NOT in `program-v2`

Per the slot-share strategy, the following are intentionally absent:

- `program/src/state/protocol_config.rs` — admin + fee config
- `program/src/state/integrator_record.rs` — FeeRecord
- `program/src/state/treasury_shard.rs` — treasury
- `program/src/processor/protocol/{initialize_protocol,update_protocol,
  register_integrator,withdraw_treasury,initialize_treasury_shard}.rs`
- Discriminators 10–14 in entrypoint dispatch
- `try_collect_fee` function in entrypoint
- `ProtocolError` enum (codes 4001–4007)
- `ProtocolConfig` / `FeeRecord` / `TreasuryShard` discriminator slots (5/6/7)
  in `state/mod.rs::AccountDiscriminator`

`scripts/check-no-fee.sh` enforces this via `scripts/fee-paths.txt`. CI fails
on any introduction. Verified clean at `audit-pending-v1`.

---

## What we want from this engagement

Specific questions for Accretion:

1. **P1 action enforcement** — is the `evaluate_pre_actions` /
   `evaluate_post_actions` engine + integration into `execute.rs` and
   `create_session.rs` introducing any new vulnerabilities the prior audit
   didn't cover? Action discriminator collision, integer overflow in
   recurring-limit window math, race conditions between snapshot/execute,
   vault invariant gaps?

2. **P5 auth port** — confirm the typeAndFlags → embedded-clientDataJSON
   format change is safe in this context. Particular attention to:
   - origin field validation (intentionally omitted — see [audit doc L1
     comment](../../program/src/auth/secp256r1/mod.rs))
   - challenge field base64url encoding/decoding round-trip
   - rpIdHash computation timing

3. **P5 authority layout change** — confirm the raw rpId → rpIdHash storage
   change preserves all binding properties. Specifically that the on-chain
   stored rpIdHash remains tied to the credential at registration.

4. **Slot-share compatibility** — given program-v2 will live at
   `LazorjRF…` mainnet slot and later be replaced in-place by
   `lazorkit-protocol`'s commercial binary, confirm:
   - State account layouts are forward-compatible (existing Wallet,
     Authority, Session, DeferredExec accounts created by program-v2 are
     readable + valid for the commercial binary)
   - Auth verification continues working for sessions/wallets created
     pre-swap

5. **Anything Accretion flagged in the prior audit that may have regressed**
   — full diff bundle at [program-src.diff](./program-src.diff) for
   line-by-line review.

---

## Deliverables included

- [DELTA_BRIEF.md](./DELTA_BRIEF.md) (this file)
- [program-src.diff](./program-src.diff) — full unified diff of `program/`
  between the two tags (~5600 lines)
- [program-src.diff.stat](./program-src.diff.stat) — per-file changed-line
  counts
- [upstream-parity.txt](./upstream-parity.txt) — byte-identity report vs
  `lazorkit-protocol`
- Git tags `audit-baseline-2026-02-accretion` (commit `d1eaaeb`) and
  `audit-pending-v1` (commit `9c97fe2`)
- This brief is intended to be sent alongside the on-chain `security_txt!`
  pointer to the prior PDF.

## Out of scope for this brief

- `tests-sdk/`, `sdk/`, `docs/`, `scripts/`, `.github/`, `Cargo.toml`,
  `assertions/Cargo.toml` — non-program changes (test infrastructure, build
  config). Available in the full `git diff audit-baseline-2026-02-accretion..audit-pending-v1`
  if Accretion requests but not part of the on-chain audit ask.
- Operational items (mainnet deploy, multisig setup, binary swap procedure)
  — handled in [docs/MAINNET_DEPLOY_RUNBOOK.md](../MAINNET_DEPLOY_RUNBOOK.md).

---

## Build reproducibility

```bash
git checkout audit-pending-v1
cd program && cargo build-sbf --features mainnet
shasum -a 256 ../target/deploy/lazorkit_program.so
# Hash should match the artifact in the eventual mainnet deploy.
```

CI workflow at `.github/workflows/release.yml` reproduces and publishes the
hash on each tag push.

---

## Contact

- Email: security@lazorkit.app
- GitHub Security Advisories: https://github.com/lazor-kit/program-v2/security/advisories/new
- On-chain pointer: `solana program show LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi`
  → `security_txt!` block links to this repo + audit PDF.
