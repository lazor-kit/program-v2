# Audit-Frozen Tag Checklist

Before submitting a `program-v2` revision to Accretion (or any auditor) for
review, walk this checklist and tag the resulting state. The tag becomes the
exact source the audit report references — a deployment that doesn't match
the tag falls outside the audit's scope.

## Naming convention

Tags use the format `audit-frozen-vN` where `N` increments per audit cycle.
Example: the first delta audit after the initial Accretion review is
`audit-frozen-v2`. The audit report should cite the tag in its scope section.

## Pre-tag checklist

### 1. Code state

- [ ] Working tree clean (`git status` empty).
- [ ] `git log --oneline main..HEAD` only contains commits intended for this
      audit. No experimental work, no half-finished refactors.
- [ ] No `TODO`, `FIXME`, `XXX`, or `unimplemented!` markers in `program/src/`
      or `assertions/src/`. Anything genuinely unfinished should be reverted
      from the audit branch.
- [ ] No `dbg!`, `println!`, `eprintln!`, or commented-out code in
      `program/src/` or `assertions/src/`.

```bash
git grep -nE "TODO|FIXME|XXX|unimplemented!|dbg!|println!|eprintln!" -- program/src assertions/src
```

### 2. Build verification

- [ ] `cargo build-sbf --features mainnet` succeeds. Record the SHA-256 of
      `target/deploy/lazorkit_program.so`.
- [ ] `cargo build-sbf --features devnet` succeeds. Record the SHA-256.
- [ ] The two hashes differ (verifies the dual-cluster mechanism is intact).
- [ ] `cargo build-sbf` (no features) fails with the expected
      `compile_error!` message containing "pick exactly one cluster".
- [ ] The CI `sbf-cluster-check` job is green for the tagged commit.

### 3. Test suite

- [ ] `cargo test --features devnet` — all unit + litesvm integration tests
      pass.
- [ ] `cd tests-sdk && npm test` (with the validator running and the
      devnet-built `.so` loaded) — all integration tests pass.
- [ ] No tests are skipped, ignored (`#[ignore]`), or commented out without
      a tracking issue.

```bash
git grep -nE "#\[ignore\]|it\.skip|xit\(|describe\.skip" -- program tests-sdk
```

### 4. Fee surface invariant (foundation build)

- [ ] `bash scripts/check-no-fee.sh` exits 0. The CI `check-no-fee` job is
      green for the tagged commit.
- [ ] Diff against the previous audit-frozen tag has no new files matching
      paths in `scripts/fee-paths.txt`.

```bash
git diff audit-frozen-v$(N-1)..HEAD -- 'program/src/state/protocol_config.rs' \
  'program/src/state/treasury_shard.rs' \
  'program/src/state/integrator_record.rs' \
  'program/src/processor/protocol/'
# Should print nothing.
```

### 5. Documentation alignment

- [ ] `CHANGELOG.md` has an entry under `[Unreleased]` describing every
      observable behavior change since the last tag.
- [ ] `docs/Architecture.md` reflects new state accounts / instructions.
- [ ] `docs/Costs.md` benchmarks have been re-measured if any instruction
      changed CU usage.
- [ ] `SECURITY.md` references the auditor that's about to look at this
      revision (or notes the audit is in progress).
- [ ] `program/src/lib.rs` `security_txt!` block — `auditors:` field still
      reflects the most recent finalised audit until this one ships.

### 6. Diff bounded

- [ ] Touched files are listed in the `Unreleased` CHANGELOG.
- [ ] Each touched file has a clear reason in the commit message that
      introduced the change.

```bash
git diff --name-only audit-frozen-v$(N-1)..HEAD | sort
```

### 7. Audit packet contents

When you push the tag, also produce the audit packet:

- [ ] PDF / Markdown summary of the changes since the last tag (1–2 pages).
- [ ] List of touched files with line counts.
- [ ] CHANGELOG diff.
- [ ] SHA-256 hashes of the `mainnet` and `devnet` SBF binaries.
- [ ] Output of `solana-verify get-program-hash` against the binaries (so
      the auditor can verify they correspond to the source).
- [ ] The two binaries themselves, if the auditor wants byte-level review.
- [ ] Note any out-of-scope changes (typos, README edits, dependency bumps
      that don't affect program logic).

## Tagging

When every box is checked:

```bash
git tag -a audit-frozen-vN -m "audit-frozen state submitted for Accretion review N"
git push origin audit-frozen-vN
```

The push to origin triggers `.github/workflows/release.yml`, which builds
both feature variants, attaches the binaries + their hashes to a GitHub
Release, and emits the `solana-verify` build attestation for reproducibility.

## Post-audit

When the audit comes back with findings:

1. Branch off the tagged state: `git checkout -b fix/audit-vN audit-frozen-vN`.
2. Land each fix as its own commit with a `Fixes: <auditor-finding-id>` line.
3. After all findings are addressed, repeat this checklist and tag
   `audit-frozen-vN.1` (or `vN+1` if the changes are substantial).
4. Update `program/src/lib.rs` `security_txt!` `auditors:` field to reference
   the finalised audit report URL once the auditor publishes.
