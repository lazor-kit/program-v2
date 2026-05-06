<!--
Keep this short. The squash-merge commit body uses the PR description, so
prose written here ends up in `git log`. Aim for what a reviewer (or your
future self bisecting) needs to understand the change.

If this PR cherry-picks from `lazorkit-protocol`: cite the upstream
commit / PR and confirm `bash scripts/check-no-fee.sh` passes.
-->

## Summary

<!-- 1–3 bullets: what this PR does and the user-visible reason. -->

## Changes

<!-- Per-file or per-area highlights. Skip if `Summary` already covers it. -->

## Test plan

- [ ] CI passes (`check-no-fee`, `sbf-cluster-check`)
- [ ] `cargo test --features devnet` passes
- [ ] `cargo build-sbf --features devnet` and `--features mainnet` both build
- [ ] `npm test` in `tests-sdk` (against a live validator) passes
- [ ] Updates docs / CHANGELOG when public behavior changes

## Audit / security notes

<!-- Skip if N/A. Otherwise: error codes touched, account layout changes,
authority/auth flow changes, vault-invariant defenses, anything that
needs Accretion follow-up. Note new entries against the
`audit-baseline-*` tag. -->

## Cherry-pick provenance (if applicable)

<!-- Upstream commit / PR being mirrored from lazorkit-protocol, plus
confirmation that the fee surface was correctly stripped:
- [ ] `bash scripts/check-no-fee.sh` clean
- [ ] No symbols added that match `scripts/fee-paths.txt`
-->

## Related

<!-- Linked issues, prior PRs, audit findings, design docs. -->
