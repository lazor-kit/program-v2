# Contributing to LazorKit

Thank you for your interest in contributing to LazorKit.

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/program-v2.git`
3. Install prerequisites (see [DEVELOPMENT.md](DEVELOPMENT.md))
4. Build the program: `cargo build-sbf`
5. Run tests: `cd tests-sdk && npm test`

## Development Workflow

1. Create a feature branch: `git checkout -b feat/your-feature`
2. Make your changes
3. Ensure all tests pass:
   ```bash
   cargo test                              # Rust unit tests
   cd tests-sdk && npm test                # Integration tests
   ```
4. Submit a pull request

## Code Style

### Rust

- Follow `rustfmt` defaults
- All state structs must implement `NoPadding` via derive macro
- Use `#[repr(C, align(8))]` for account structures
- No Borsh serialization -- zero-copy only (pinocchio)

### TypeScript

- TypeScript strict mode
- Use `@solana/web3.js` v1 conventions

## Pull Request Guidelines

- Keep PRs focused on a single concern
- Include tests for new features
- Update documentation if behavior changes
- Reference any related issues

## Testing Requirements

All PRs must:

1. Pass `cargo test` (35+ Rust tests)
2. Pass `npm test` in `tests-sdk/` (56+ tests: integration, security, permissions, sessions)
3. Not break the benchmark script (`npm run benchmark`)

## Security

If you discover a vulnerability, please follow the process in [SECURITY.md](SECURITY.md). Do not open a public issue.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
