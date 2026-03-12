# LazorKit V2 — Audit-Style Checklist (w/ TypeScript Tests)

Tài liệu này là checklist theo kiểu audit: mỗi instruction có **accounts**, **invariants**, **các kiểm tra bắt buộc**, **ý tưởng tấn công (threat model)** và **test TypeScript** tương ứng (nằm trong `tests-real-rpc/tests/`).

> Scope: đối chiếu với code trong `program/src/processor/*`, `program/src/auth/*`, `program/src/state/*`, `program/src/utils.rs`.

## Global invariants (áp dụng mọi instruction)

- **PDA seed correctness**: mọi PDA quan trọng phải được re-derive/verify bằng `find_program_address` và so sánh `key`.
- **Owner checks**: các account state của chương trình (`Wallet/Authority/Session/Config`) phải có `owner == program_id`.
- **Discriminator checks**: byte đầu của data phải đúng loại account (`Wallet=1`, `Authority=2`, `Session=3`, `Config=4`).
- **No CPI for Secp256r1 auth**: nếu dùng secp256r1 auth, phải fail khi bị gọi qua CPI (`stack_height > 1`).

## Instruction: `InitializeConfig` (disc = 6)

### Accounts
- `admin` (signer, writable)
- `config` (writable) — PDA `["config"]`
- `system_program`
- `rent`

### Invariants / required checks
- `admin` ký.
- `config` đúng seeds `["config"]` và chưa init.
- `num_shards >= 1`.
- Khởi tạo theo transfer-allocate-assign, set discriminator/version/admin/fees.

### Attack ideas
- **Config spoofing**: đưa 1 account khác không phải `["config"]`.
- **Re-init**: cố init lại config đã tồn tại.

### Tests
- `tests-real-rpc/tests/config.test.ts`

## Instruction: `UpdateConfig` (disc = 7)

### Accounts
- `admin` (signer)
- `config` (writable) — PDA `["config"]`

### Invariants / required checks
- `admin == config.admin`.
- Không cho giảm `num_shards` (tránh stranded funds).

### Attack ideas
- Non-admin update.
- Giảm shards để “mất” shard cũ.

### Tests
- `tests-real-rpc/tests/config.test.ts`

## Instruction: `InitTreasuryShard` (disc = 11)

### Accounts
- `payer` (signer, writable)
- `config` — PDA `["config"]`
- `treasury_shard` — PDA `["treasury", shard_id]`
- `system_program`
- `rent`

### Invariants / required checks
- `shard_id < config.num_shards`.
- Seeds đúng.
- Khởi tạo shard theo transfer-allocate-assign (0 bytes) để chống pre-fund DoS.

### Tests
- `tests-real-rpc/tests/config.test.ts`

## Instruction: `SweepTreasury` (disc = 10)

### Accounts
- `admin` (signer)
- `config`
- `treasury_shard` (writable)
- `destination` (writable)

### Invariants / required checks
- `admin == config.admin`.
- Seeds shard đúng.
- **Preserve rent floor**: shard giữ lại minimum balance cho `space` hiện tại.

### Attack ideas
- Sweep shard ngoài range.
- Sweep xuống dưới rent-exempt để làm shard “brick” và không nhận fee được nữa.

### Tests
- `tests-real-rpc/tests/audit_regression.test.ts` (Regression 1)
- `tests-real-rpc/tests/config.test.ts`

## Instruction: `CreateWallet` (disc = 0)

### Accounts
- `payer` (signer, writable)
- `wallet` (writable) — PDA `["wallet", user_seed]`
- `vault` (writable) — PDA `["vault", wallet]`
- `authority` (writable) — PDA `["authority", wallet, id_seed]` (id_seed = Ed25519 pubkey hoặc Secp credential hash)
- `system_program`
- `rent`
- `config`
- `treasury_shard`

### Invariants / required checks
- Seeds wallet/vault/authority đúng, chưa init.
- Collect `wallet_fee` vào đúng treasury shard của `payer`.
- `Vault` được “mark initialized” bằng allocate(0) và owned by System Program (để nhận SOL transfer chuẩn).
- Authority header: `role=Owner(0)`, `wallet=wallet_pda`.

### Attack ideas
- **Duplicate wallet**: tạo lại cùng seed.
- **Non-canonical authority bump**: cố dùng bump khác (chương trình re-derive canonical).
- **Treasury shard spoof**: đưa shard PDA sai.

### Tests
- `tests-real-rpc/tests/wallet.test.ts` (duplicate seed, discovery)

## Instruction: `AddAuthority` (disc = 1)

### Accounts
- `payer` (signer)
- `wallet`
- `admin_authority` (signer-ish, program-owned, writable với secp)
- `new_authority` (writable)
- `system_program`
- optional `authorizer_signer` (Ed25519 signer)
- `config`
- `treasury_shard`

### Invariants / required checks
- `admin_authority.wallet == wallet`.
- RBAC:
  - Owner add bất kỳ role.
  - Admin chỉ add Spender.
- `new_authority` seeds đúng `["authority", wallet, id_seed]`, chưa init.
- Fee collection đúng shard.

### Attack ideas
- Cross-wallet add (authority wallet A add vào wallet B).
- Admin add Admin/Owner.

### Tests
- `tests-real-rpc/tests/authority.test.ts`

## Instruction: `RemoveAuthority` (disc = 2)

### Accounts
- `payer` (signer)
- `wallet`
- `admin_authority` (writable)
- `target_authority` (writable)
- `refund_destination` (writable)
- `system_program`
- optional `authorizer_signer`
- `config`
- `treasury_shard`

### Invariants / required checks
- `admin_authority.wallet == wallet`.
- `target_authority.wallet == wallet` (**chống cross-wallet deletion**).
- RBAC:
  - Owner remove bất kỳ.
  - Admin chỉ remove Spender.
- Close pattern: move lamports + zero data.

### Tests
- `tests-real-rpc/tests/authority.test.ts` (cross-wallet remove, RBAC)

## Instruction: `TransferOwnership` (disc = 3)

### Accounts
- `payer` (signer)
- `wallet`
- `current_owner_authority` (writable)
- `new_owner_authority` (writable)
- `system_program`
- `rent`
- optional `authorizer_signer`
- `config`
- `treasury_shard`

### Invariants / required checks
- `current_owner.role == Owner(0)` và `wallet` match.
- `new_owner_authority` seeds đúng, chưa init.
- Prevent zero-id transfer (id_seed all zeros).
- Atomic swap: create new owner + close current owner (refund rent to payer).

### Attack ideas
- Admin cố transfer ownership.
- Transfer sang “zero key”.

### Tests
- `tests-real-rpc/tests/wallet.test.ts` (admin cannot transfer, zero transfer)

## Instruction: `Execute` (disc = 4)

### Accounts (fixed prefix)
- `payer` (signer)
- `wallet`
- `authority_or_session` (program-owned; writable trong impl hiện tại)
- `vault` — PDA `["vault", wallet]`
- `config`
- `treasury_shard`
- `system_program`
- optional `sysvar_instructions` (bắt buộc cho secp256r1)
- … dynamic inner accounts (theo compact instructions)

### Invariants / required checks
- Fee collection `action_fee` đúng shard.
- `wallet` discriminator đúng.
- Authority path:
  - `authority.wallet == wallet`.
  - Ed25519: signer khớp pubkey trong authority data.
  - Secp256r1: require sysvar introspection + slothashes nonce + account-binding hash.
- Session path:
  - `session.wallet == wallet`
  - `Clock.slot <= expires_at`
  - require signer khớp `session_key`.
- `vault` seeds đúng (chống “sign nhầm vault”).
- Reject self-reentrancy (không CPI vào chính program).

### Attack ideas
- Cross-wallet execute (authority wallet A điều khiển vault wallet B).
- Wrong vault seeds.
- Self-reentrancy CPI.
- Secp256r1: bỏ precompile instruction hoặc sysvars → phải fail.

### Tests
- `tests-real-rpc/tests/execute.test.ts`

## Instruction: `CreateSession` (disc = 5)

### Accounts
- `payer` (signer, writable)
- `wallet`
- `admin_authority` (writable)
- `session` (writable) — PDA `["session", wallet, session_key]`
- `system_program`
- `rent`
- optional `authorizer_signer`
- `config`
- `treasury_shard`

### Invariants / required checks
- `admin_authority` là Authority discriminator và `wallet` match.
- RBAC: chỉ Owner/Admin.
- Seeds session đúng và chưa init.
- **System Program must be real** (anti-spoof).

### Attack ideas
- Spender tạo session.
- Session PDA giả dạng authority để tạo session.
- **System program spoofing**: đưa program id khác ở vị trí System Program.

### Tests
- `tests-real-rpc/tests/session.test.ts`
- `tests-real-rpc/tests/security_checklist.test.ts` (System Program spoofing)

## Instruction: `CloseSession` (disc = 8)

### Accounts
- `payer` (signer, writable)
- `wallet`
- `session` (writable)
- `config`
- optional `authorizer` (wallet authority PDA)
- optional `authorizer_signer`
- optional `sysvar_instructions`
- (SDK còn append System Program, nhưng on-chain hiện không dùng)

### Invariants / required checks
- `session.wallet == wallet` và seeds session re-derive đúng.
- Authorization:
  - Protocol admin (`payer == config.admin`) **chỉ được close expired**.
  - Wallet owner/admin có thể close active hoặc expired (có auth).
- Close: refund toàn bộ lamports session về `payer`, zero data.

### Attack ideas
- Protocol admin cố close session còn active (rent theft / grief).
- Config spoofing.

### Tests
- `tests-real-rpc/tests/audit_regression.test.ts` (Config spoofing)
- `tests-real-rpc/tests/security_checklist.test.ts` (protocol admin active close rejected)

## Instruction: `CloseWallet` (disc = 9)

### Accounts
- `payer` (signer)
- `wallet` (writable)
- `vault` (writable)
- `owner_authority` (program-owned authority PDA, role=0)
- `destination` (writable)
- optional `owner_signer` / `sysvar_instructions`
- System Program (SDK always includes)

### Invariants / required checks
- Destination != wallet/vault.
- Owner role == 0.
- Vault seeds đúng.
- Drain vault via SystemProgram transfer (vault signs with PDA seeds).
- Drain wallet lamports → destination; zero wallet data.

### Attack ideas
- Destination swap (đã bind vào payload đối với auth path).
- Self-transfer burn (destination = vault/wallet).

### Tests
- `tests-real-rpc/tests/audit_regression.test.ts` (Regression 2)

