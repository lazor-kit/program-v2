# @lazorkit/sdk

TypeScript SDK for the LazorKit Smart Contract Wallet on Solana.

Built with **Solana Kit v2** (modern Solana development framework).

## Features

- ✅ Built with modern Solana Kit (@solana/kit v2)
- ✅ Type-safe instruction builders
- ✅ Full RBAC support (Owner, Admin, Spender roles)
- ✅ Multi-signature wallet with session keys
- ✅ Ed25519 and Secp256r1 authority types
- ✅ CPI execution via wallet vault
- ✅ Zero legacy dependencies (no web3.js v1)

## Installation

```bash
npm install @lazorkit/sdk
# or
pnpm install @lazorkit/sdk
```

## Requirements

- `@solana/kit` ^2.0.0
- `@solana/web3.js` ^2.0.0 (new version)

## Quick Start

```typescript
import { Client, createDefaultRpcTransport } from '@solana/kit';
import { createKeyPairSignerFromBytes } from '@solana/signers';
import { 
  createWalletInstruction,
  findConfigPDA,
  findVaultPDA,
  encodeEd25519Authority,
  AuthorityType,
  generateWalletId,
  LAZORKIT_PROGRAM_ID
} from '@lazorkit/sdk';

// Initialize client
const rpc = createDefaultRpcTransport({ url: 'https://api.devnet.solana.com' });
const client = new Client({ rpc });

// Generate wallet
const walletId = generateWalletId();
const configPDA = await findConfigPDA(walletId);
const vaultPDA = await findVaultPDA(configPDA.address);

// Create instruction
const instruction = createWalletInstruction({
  config: configPDA.address,
  payer: owner.address,
  vault: vaultPDA.address,
  systemProgram: '11111111111111111111111111111111',
  id: walletId,
  bump: configPDA.bump,
  walletBump: vaultPDA.bump,
  ownerAuthorityType: AuthorityType.Ed25519,
  ownerAuthorityData: encodeEd25519Authority(ownerBytes),
  programId: LAZORKIT_PROGRAM_ID
});

// Send transaction
await client.sendAndConfirmTransaction({
  instructions: [instruction],
  signers: [owner]
});
```

## API Reference

### PDA Helpers

- `findConfigPDA(id: Uint8Array): Promise<ProgramDerivedAddress>`
- `findVaultPDA(configAddress: Address): Promise<ProgramDerivedAddress>`
- `generateWalletId(): Uint8Array`

### Authority Encoding

- `encodeEd25519Authority(publicKey: Uint8Array): Uint8Array`
- `encodeEd25519SessionAuthority(master, session, validUntil): Uint8Array`
- `encodeSecp256r1Authority(publicKey: Uint8Array): Uint8Array`

### Instruction Builders

All instruction builders use Solana Kit v2 API:

- `createWalletInstruction(params): Instruction`
- `addAuthorityInstruction(params): Instruction`
- `removeAuthorityInstruction(params): Instruction`
- `executeInstruction(params): Instruction`
- `createSessionInstruction(params): Instruction`
- `updateAuthorityInstruction(params): Instruction`
- `transferOwnershipInstruction(params): Instruction`

## Examples

See `/examples` for complete usage:

- `create-wallet.ts` - Create a LazorKit wallet
- `execute-transfer.ts` - Execute SOL transfer via wallet

## Migration from web3.js v1

This SDK uses **Solana Kit v2**, not legacy web3.js. Key differences:

- `Connection` → `Client`
- `PublicKey` → `Address`
- `Transaction` → `sendAndConfirmTransaction()`
- Async PDA derivation
- New instruction format

## License

MIT
