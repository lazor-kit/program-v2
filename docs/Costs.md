# LazorKit Cost Analysis

This document provides comprehensive cost data for the LazorKit smart wallet program on Solana. All compute unit (CU) measurements are from real transactions against a local validator. Rent costs use Solana's standard formula.

> Program ID: `2m47smrvCRpuqAyX2dLqPxpAC1658n1BAQga1wRCsQiT`

---

## Compute Units & Transaction Size

| Instruction | CU | Tx Size (bytes) | Ix Data (bytes) | Accounts | Instructions |
|---|---|---|---|---|---|
| Normal SOL Transfer (baseline) | 150 | 215 | 12 | 2 | 1 |
| CreateWallet (Ed25519) | 16,687 | 408 | 73 | 6 | 1 |
| CreateWallet (Secp256r1) | 15,185 | 441 | 106 | 6 | 1 |
| AddAuthority (Ed25519 admin) | 13,342 | 473 | 41 | 7 | 1 |
| Execute Secp256r1 (SOL transfer) | 10,816 | 708 | 271 | 8 | 2 |
| CreateSession (Ed25519) | 6,015 | 473 | 41 | 7 | 1 |

**Notes:**
- CU values are median from real transactions
- Secp256r1 Execute requires 2 instructions (precompile verification + execute)
- All operations fit well within Solana's 200,000 CU default budget
- Transaction sizes are well within Solana's 1,232-byte limit

---

## LazorKit vs Normal SOL Transfer

| Metric | Normal Transfer | LazorKit Secp256r1 | Overhead |
|---|---|---|---|
| Compute Units | 150 | 10,816 | 72x |
| Transaction Size | 215 bytes | 708 bytes | +493 bytes |
| Instruction Data | 12 bytes | 271 bytes | +259 bytes |
| Accounts | 2 | 8 | +6 |
| Instructions per Tx | 1 | 2 | +1 |
| Transaction Fee | 0.000005 SOL | 0.000005 SOL | Same |

**Why the overhead is acceptable:**
- 10,816 CU is only **5.4%** of the 200,000 CU default budget
- 708 bytes is **57%** of the 1,232-byte transaction limit
- Transaction fee is identical (base fee is per-signature, not per-CU)
- The overhead buys: passkey auth, RBAC, replay protection, session keys, multi-sig

---

## Rent-Exempt Costs

Solana requires accounts to maintain a minimum balance (rent-exempt) based on data size. The formula is `(128 + data_size) * 3,480 * 2` lamports.

| Account | Data (bytes) | Rent-Exempt (SOL) | Rent-Exempt (lamports) |
|---|---|---|---|
| Wallet PDA | 8 | 0.000946560 | 946,560 |
| Authority (Ed25519) | 80 | 0.001447680 | 1,447,680 |
| Authority (Secp256r1) | 113 | 0.001677360 | 1,677,360 |
| Session | 80 | 0.001447680 | 1,447,680 |
| Vault PDA | 0 | 0 | 0 |

**Vault PDA** is not initialized as a program-owned account. It simply receives SOL via transfer. No rent cost.

---

## Total Wallet Creation Cost

Creating a wallet involves allocating a Wallet PDA and the first Authority PDA.

| Auth Type | Wallet Rent | Authority Rent | Tx Fee | Total |
|---|---|---|---|---|
| Ed25519 | 0.000947 SOL | 0.001448 SOL | 0.000005 SOL | **0.002399 SOL** |
| Secp256r1 (Passkey) | 0.000947 SOL | 0.001677 SOL | 0.000005 SOL | **0.002629 SOL** |

At $150/SOL, wallet creation costs approximately **$0.36 - $0.39 USD**.

---

## Ongoing Transaction Costs

| Operation | Cost per Transaction |
|---|---|
| Execute (SOL transfer) | 0.000005 SOL (base fee only) |
| Execute (token transfer) | 0.000005 SOL (base fee only) |
| Add Authority | 0.000005 SOL + authority rent |
| Remove Authority | 0.000005 SOL (rent refunded) |
| Create Session | 0.000005 SOL + session rent |

**Key points:**
- No per-transaction rent costs for Execute
- No additional fees for odometer counter (stored in existing authority account)
- RemoveAuthority refunds the full rent-exempt balance to a specified destination
- Session accounts can be reclaimed after expiry

---

## Account Data Sizes

| Account | Header | Variable Data | Total |
|---|---|---|---|
| WalletAccount | 8 bytes | 0 | **8 bytes** |
| Authority (Ed25519) | 48 bytes | 32 bytes (pubkey) | **80 bytes** |
| Authority (Secp256r1) | 48 bytes | 65 bytes (cred_hash + compressed_pubkey) | **113 bytes** |
| SessionAccount | 80 bytes | 0 | **80 bytes** |

The compact data sizes are achieved through:
- `#[repr(C, align(8))]` with `NoPadding` derive macro
- 33-byte compressed Secp256r1 public keys (not 64-byte uncompressed)
- No Borsh serialization overhead

---

## Reproducing These Numbers

```bash
# Start local validator
cd tests-sdk && npm run validator:start

# Run benchmarks
npm run benchmark
```

The benchmark script (`tests-sdk/tests/benchmark.ts`) sends real transactions and extracts CU consumption from transaction metadata.
