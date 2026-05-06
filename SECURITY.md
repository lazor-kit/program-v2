# Security Policy

## Reporting Vulnerabilities

If you discover a security vulnerability in LazorKit, please report it responsibly:

1. **Do NOT open a public GitHub issue**
2. Use [GitHub's private vulnerability reporting](https://github.com/lazor-kit/program-v2/security/advisories/new)
3. Or email: security@lazorkit.app

### Response Timeline

- **48 hours**: Acknowledgment of report
- **7 days**: Initial assessment and severity classification
- **30 days**: Target for fix and disclosure

## Scope

The following are in scope:

- On-chain Solana program (`program/src/`)
- TypeScript SDK (`@lazorkit/sdk-legacy`, lives in sibling `lazorkit-protocol` repo)
- PDA derivation and signature verification logic
- Replay protection mechanisms

The following are out of scope:

- Frontend applications
- Third-party dependencies (report to upstream)
- Test files and scripts

## Audit Status

LazorKit V2 has been audited by Accretion.

**Status**: 17/17 security issues resolved

Reports available in the `audits/` directory.

## Security Features

- Odometer counter replay protection (monotonic u32 per authority)
- Clock-based slot freshness window (150 slots via `Clock::get()`)
- CPI stack_height reentrancy prevention
- Signature binding to payer, accounts, counter, and program_id
- Self-removal and owner removal protection
- Session expiry validation (future check + 30-day max)
- Discriminator checks on all PDA accounts
- Transfer-Allocate-Assign pattern (DoS prevention)
