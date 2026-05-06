# Mainnet Deployment Runbook (Foundation Build)

This runbook covers deploying `program-v2` (the no-fee foundation build) to the
LazorKit mainnet program slot, and the binary swap back to `lazorkit-protocol`
(the commercial build) at the end of the foundation contract.

The same on-chain slot — `LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi` — hosts
both binaries at different times. dApp integrators keep using one stable
program ID through the transition; only the on-chain behavior changes when
the binary is swapped.

## Why this scheme

- **Foundation contract** requires a no-profit deploy (no admin, no protocol
  fees). `program-v2` ships exactly that.
- **Brand continuity** — the mainnet program ID was published before the
  foundation contract. Switching IDs would force every integrating dApp to
  redeploy and migrate their on-chain references.
- **Reversibility** — Solana program upgrades replace the binary at a slot.
  The same upgrade authority can swap from `program-v2` → `lazorkit-protocol`
  in one transaction once the foundation contract ends.

## Prerequisites

- Solana CLI 3.0.4 (pinned in `Cargo.toml [workspace.metadata.cli]`).
- The mainnet program **upgrade authority** — a multisig keypair held jointly
  by the foundation and the lazorkit team. (If a single key holds the
  authority, transition to multisig before deploy — losing it locks the slot.)
- The mainnet program **address keypair** for `LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi`
  (only needed for the *initial* deploy; subsequent upgrades use the upgrade
  authority).
- A funded mainnet wallet for rent + transaction fees (~5 SOL leaves room).
- Audit sign-off on the exact source revision being deployed (see
  `docs/AUDIT_PREP.md` for the pre-tag checklist).

## Initial deploy (program-v2 → mainnet slot)

1. Verify the source matches the audit-frozen tag:

   ```bash
   git fetch --tags
   git checkout audit-frozen-vN     # whatever tag was audited
   git status      # must be clean
   ```

2. Confirm the workspace is on the right toolchain:

   ```bash
   cat Cargo.toml | grep -A1 'metadata.cli'   # → solana = "3.0.4"
   solana --version                            # → must be 3.0.4
   rustup show active-toolchain                # matches rust-toolchain.toml
   ```

3. Build the mainnet binary:

   ```bash
   cd program
   cargo build-sbf --features mainnet
   cd ..
   ```

4. Verify the embedded program ID:

   ```bash
   solana-keygen pubkey target/deploy/lazorkit_program-keypair.json
   # → must print LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi
   ```

   If the printed pubkey is anything else, the build picked up the wrong
   keypair. Replace `target/deploy/lazorkit_program-keypair.json` with the
   keypair file for `LazorjRF…` before deploying. **Do not deploy with a
   mismatched ID** — the binary's compile-time `crate::ID` reads from
   `assertions/src/lib.rs` (the `LazorjRF…` constant under the `mainnet`
   feature) and will fail every PDA derivation if loaded into a different
   slot.

5. Record the binary hash for the release artifact:

   ```bash
   shasum -a 256 target/deploy/lazorkit_program.so
   # Compare against the hash published in the GitHub Release for this tag.
   ```

6. Deploy:

   ```bash
   solana program deploy target/deploy/lazorkit_program.so -u m \
     --program-id <path/to/LazorjRF-keypair.json> \
     --upgrade-authority <path/to/upgrade-authority.json>
   ```

7. Verify the deployed program:

   ```bash
   solana program show LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi -u m
   # Look for: ProgramData Address, Authority, Last Deployed In Slot, Data Len
   ```

   The `Authority` line must match the multisig pubkey that should retain
   upgrade control.

8. Smoke-test on-chain:

   - Create a wallet via the SDK pointing at the foundation flavor.
   - Verify no fee transfer occurs (compare lamport balances pre/post).
   - Run `solana account <walletPda>` and confirm the discriminator is `1`.

## Subsequent upgrades (program-v2 patch deploy)

For a routine upgrade of the foundation build (e.g., a security fix):

```bash
git checkout <new-audit-frozen-tag>
cd program && cargo build-sbf --features mainnet && cd ..
solana program deploy target/deploy/lazorkit_program.so -u m \
  --program-id LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi \
  --upgrade-authority <path/to/upgrade-authority.json>
```

Document each upgrade in `CHANGELOG.md` and update the on-chain `security.txt`
metadata via the next build (the macro embeds `GITHUB_SHA` automatically when
built in CI).

## Binary swap at contract end (program-v2 → lazorkit-protocol)

When the foundation contract ends and the slot needs to host the commercial
build again:

1. In the **`lazorkit-protocol`** repo, check out the audit-frozen tag for
   the commercial build:

   ```bash
   cd ../lazorkit-protocol
   git checkout audit-frozen-commercial-vN
   ```

2. Build the commercial mainnet binary:

   ```bash
   cd program && cargo build-sbf --features mainnet && cd ..
   ```

3. Verify it embeds the same program ID (`LazorjRF…`):

   ```bash
   solana-keygen pubkey target/deploy/lazorkit_program-keypair.json
   ```

4. Deploy the upgrade. **No `--program-id` flag** — the slot already exists,
   we're upgrading it:

   ```bash
   solana program deploy target/deploy/lazorkit_program.so -u m \
     --program-id LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi \
     --upgrade-authority <path/to/upgrade-authority.json>
   ```

5. Initialise the protocol config (commercial-only instruction; no equivalent
   exists in the foundation build, so this is a fresh state):

   ```bash
   # Use the lazorkit-protocol SDK to send InitializeProtocol + InitializeTreasuryShard.
   # This step is only needed the first time the commercial build is on this
   # slot; subsequent upgrades preserve the existing ProtocolConfig PDA.
   ```

6. Smoke-test:

   - Create a wallet — the fee transfer to the treasury shard should succeed.
   - Verify the SDK consumer receives `protocolConfigPda` etc. without error.

## Pre-existing wallet accounts across the swap

Wallet, Vault, Authority, Session, and DeferredExec PDAs created under the
foundation build remain valid and accessible under the commercial build —
they all derive from the same program ID and the same seed schema, and the
account-discriminator layout is identical between the two builds.

What changes:

- Fee accounts (`ProtocolConfig`, `FeeRecord`, `TreasuryShard`) start
  appearing once the commercial binary is live. Existing wallets continue to
  work; the fee is charged on subsequent `CreateWallet` / `Execute` /
  `ExecuteDeferred` calls if the SDK appends fee accounts.
- Sessions created with action permissions under the foundation build remain
  enforced under the commercial build — action handling is identical.
- The on-chain `security.txt` updates to advertise `lazorkit-protocol` as the
  source repo. Integrators querying it should re-fetch.

## Rollback

If a deployed upgrade misbehaves:

```bash
solana program deploy target/deploy/<previous-good-binary>.so -u m \
  --program-id LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi \
  --upgrade-authority <path/to/upgrade-authority.json>
```

Solana program slots are upgrade-replaceable (so long as the upgrade
authority hasn't been frozen). Keep the previous mainnet binary archived
locally + in the GitHub Release for at least one audit cycle so a rollback
is one command, not a rebuild.

## Locking the upgrade authority

After the post-contract swap to `lazorkit-protocol` is stable and you no
longer want any further upgrades:

```bash
solana program set-upgrade-authority \
  LazorjRFNavitUaBu5m3WaNPjU1maipvSW2rZfAFAKi \
  --upgrade-authority <path/to/current-upgrade-authority.json> \
  --final
```

This is **irreversible**. Only do this after a long stability window and
explicit foundation/team agreement — once finalised, the binary cannot be
patched, security advisories included.
