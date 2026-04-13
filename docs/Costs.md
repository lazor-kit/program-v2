# LazorKit Cost Analysis

This document provides comprehensive cost data for the LazorKit smart wallet program on Solana. All compute unit (CU) measurements are from real transactions against a local validator. Rent costs use Solana's standard formula.

> Program ID: `FLb7fyAtkfA4TSa2uYcAT8QKHd2pkoMHgmqfnXFXo7ao`

---

## Compute Units & Transaction Size

| Instruction | CU | Tx Size (bytes) | Ix Data (bytes) | Accounts | Instructions |
|---|---|---|---|---|---|
| Normal SOL Transfer (baseline) | 150 | 215 | 12 | 2 | 1 |
| CreateWallet (Ed25519) | 15,187 | 408 | 73 | 6 | 1 |
| CreateWallet (Secp256r1) | 13,687 | 453 | 118 | 6 | 1 |
| AddAuthority (Ed25519 admin) | 5,846 | 473 | 41 | 7 | 1 |
| Execute Secp256r1 (SOL transfer) | 12,441 | 658 | 254 | 7 | 2 |
| Execute Session (SOL transfer) | 8,983 | 452 | 20 | 7 | 1 |
| CreateSession (Ed25519) | 6,015 | 473 | 41 | 7 | 1 |

**Notes:**
- CU values are from real transactions on a local validator
- Secp256r1 Execute requires 2 instructions (precompile verification + execute)
- Session Execute is cheaper -- only 1 instruction, no precompile, no auth payload
- All operations fit well within Solana's 200,000 CU default budget
- Transaction sizes are well within Solana's 1,232-byte limit

### Deferred Execution (Large Payloads)

For operations exceeding the ~574 bytes available in a single Secp256r1 Execute transaction:

| Instruction | CU | Tx Size (bytes) | Accounts | Instructions |
|---|---|---|---|---|
| Authorize (TX1) | 11,709 | 705 | 7 | 2 |
| ExecuteDeferred (TX2, 1 inner ix) | 6,904 | 356 | 7 | 1 |

| Metric | Immediate Execute | Deferred (2 txs) |
|---|---|---|
| Total CU | 12,441 | 18,613 (11,709 + 6,904) |
| Inner Ix Capacity | ~574 bytes | ~1,100 bytes (1.9x) |
| Tx Fee | 0.000005 SOL | 0.00001 SOL |
| Temp Rent | -- | 0.002116 SOL (refunded) |

The deferred path trades ~50% more total CU and 2x the tx fee for 1.9x the inner instruction space. The DeferredExec account rent (0.002116 SOL) is temporary -- it is refunded when TX2 executes or when the payer reclaims an expired authorization.

---

## Transaction Size Optimization (v2)

The Secp256r1 Execute transaction was optimized from **708 bytes to 658 bytes** (50 bytes saved) via three changes:

| Optimization | Bytes Saved | Details |
|---|---|---|
| Drop SlotHashes sysvar | ~32 bytes | Use `Clock::get()` for slot freshness instead of SlotHashes sysvar lookup. Removes 1 account from the transaction. |
| u32 counter (was u64) | ~4 bytes | 4 billion operations per authority is sufficient. Saves 4 bytes in auth payload + 4 bytes in challenge hash. |
| rpId stored on-chain | ~14 bytes | rpId (e.g. "example.com") stored on authority account at creation time, no longer sent per-tx. |

**Security impact:** None. The odometer counter remains the primary replay protection. Slot freshness via `Clock::get()` provides the same age check (150-slot window) without requiring the SlotHashes sysvar account.

---

## LazorKit vs Normal SOL Transfer

| Metric | Normal Transfer | LazorKit Secp256r1 | LazorKit Session | Notes |
|---|---|---|---|---|
| Compute Units | 150 | 12,441 | 8,983 | Session uses Ed25519 signer (no precompile) |
| Transaction Size | 215 bytes | 658 bytes | 452 bytes | Session tx is smaller (no precompile ix) |
| Instruction Data | 12 bytes | 254 bytes | 20 bytes | Session has no auth payload |
| Accounts | 2 | 7 | 7 | Secp256r1 uses sysvar_instructions only |
| Instructions per Tx | 1 | 2 | 1 | Session needs only 1 instruction |
| Transaction Fee | 0.000005 SOL | 0.000005 SOL | 0.000005 SOL | Same base fee |

**Why the overhead is acceptable:**
- 12,441 CU (Secp256r1) is only **6.2%** of the 200,000 CU default budget
- 8,983 CU (Session) is only **4.5%** of the 200,000 CU default budget
- 658 bytes (Secp256r1) is **53%** of the 1,232-byte transaction limit, leaving **574 bytes** for inner instructions
- Deferred Execution provides ~1,100 bytes for inner instructions when needed (1.9x)
- 452 bytes (Session) is only **37%** of the 1,232-byte limit
- Transaction fee is identical (base fee is per-signature, not per-CU)
- The overhead buys: passkey auth, RBAC, replay protection, session keys, multi-sig

**Session keys** are ideal for frequent transactions (gaming, DeFi) -- they're faster, cheaper, and only need a one-time setup cost.

---

## Rent-Exempt Costs

Solana requires accounts to maintain a minimum balance (rent-exempt) based on data size. The formula is `(128 + data_size) * 3,480 * 2` lamports.

| Account | Data (bytes) | Rent-Exempt (SOL) | Rent-Exempt (lamports) |
|---|---|---|---|
| Wallet PDA | 8 | 0.000946560 | 946,560 |
| Authority (Ed25519) | 80 | 0.001447680 | 1,447,680 |
| Authority (Secp256r1) | ~125 | 0.001760880 | 1,760,880 |
| Session | 80 | 0.001447680 | 1,447,680 |
| DeferredExec | 176 | 0.002116320 | 2,116,320 |
| Vault PDA | 0 | 0 | 0 |

**Notes:**
- Secp256r1 authority size is variable: 48 (header) + 32 (cred hash) + 33 (pubkey) + 1 (rpIdLen) + N (rpId). For `rpId = "example.com"` (11 bytes), total = 125 bytes.
- **DeferredExec** rent is temporary -- refunded when ExecuteDeferred closes the account or when ReclaimDeferred reclaims an expired authorization.
- **Vault PDA** is not initialized as a program-owned account. It simply receives SOL via transfer. No rent cost.

---

## Total Wallet Creation Cost

Creating a wallet involves allocating a Wallet PDA and the first Authority PDA.

| Auth Type | Wallet Rent | Authority Rent | Tx Fee | Total |
|---|---|---|---|---|
| Ed25519 | 0.000947 SOL | 0.001448 SOL | 0.000005 SOL | **0.002399 SOL** |
| Secp256r1 (Passkey) | 0.000947 SOL | 0.001761 SOL | 0.000005 SOL | **0.002713 SOL** |

At $150/SOL, wallet creation costs approximately **$0.36 - $0.41 USD**.

---

## Ongoing Transaction Costs

| Operation | Cost per Transaction |
|---|---|
| Execute (SOL transfer) | 0.000005 SOL (base fee only) |
| Execute (token transfer) | 0.000005 SOL (base fee only) |
| Deferred Execute (2 txs) | 0.00001 SOL (2x base fee) + 0.002116 SOL temp rent (refunded) |
| Add Authority | 0.000005 SOL + authority rent |
| Remove Authority | 0.000005 SOL (rent refunded) |
| Create Session | 0.000005 SOL + session rent |
| Reclaim Deferred | 0.000005 SOL (expired DeferredExec rent refunded) |

**Key points:**
- No per-transaction rent costs for Execute
- No additional fees for odometer counter (stored in existing authority account)
- RemoveAuthority refunds the full rent-exempt balance to a specified destination
- Session accounts can be reclaimed after expiry

---

## Session Key Cost

Session keys enable cheap, fast transactions without passkey re-authentication on every operation.

| Item | Cost |
|---|---|
| Session account rent (one-time) | 0.001448 SOL |
| CreateSession tx fee | 0.000005 SOL |
| **Total setup cost** | **0.001453 SOL** |
| Execute via session (per tx) | 0.000005 SOL |

At $150/SOL, session setup costs ~$0.22 USD. Each subsequent execute costs $0.00075.

**Rent recovery:** Session rent (0.001448 SOL) is refundable after the session expires. The session account can be closed and lamports returned to the payer.

---

## Account Data Sizes

| Account | Header | Variable Data | Total |
|---|---|---|---|
| WalletAccount | 8 bytes | 0 | **8 bytes** |
| Authority (Ed25519) | 48 bytes | 32 bytes (pubkey) | **80 bytes** |
| Authority (Secp256r1) | 48 bytes | 32 (cred_hash) + 33 (pubkey) + 1 (rpIdLen) + N (rpId) | **114+ bytes** |
| SessionAccount | 80 bytes | 0 | **80 bytes** |
| DeferredExecAccount | 176 bytes | 0 | **176 bytes** |

The compact data sizes are achieved through:
- `#[repr(C, align(8))]` with `NoPadding` derive macro
- 33-byte compressed Secp256r1 public keys (not 64-byte uncompressed)
- No Borsh serialization overhead
- rpId stored on authority account (saves per-tx payload bytes)

---

## Reproducing These Numbers

```bash
# Start local validator
cd tests-sdk && npm run validator:start

# Run benchmarks
npm run benchmark
```

The benchmark script (`tests-sdk/tests/benchmark.ts`) sends real transactions and extracts CU consumption from transaction metadata.
