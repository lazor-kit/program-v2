/**
 * Example: Low-level instruction building
 * 
 * This example demonstrates how to use the low-level LazorkitInstructionBuilder
 * for full control over instruction creation.
 */

import { 
  LazorkitInstructionBuilder,
  findWalletAccount,
  findWalletVault,
  serializeInstructions,
  buildAuthorityPayload,
  buildMessageHash,
  Ed25519Authority,
  AuthorityType,
  RolePermission,
} from '@lazorkit/sdk';
import type { Address } from '@solana/kit';

async function main() {
  // 1. Setup
  const walletId = new Uint8Array(32);
  crypto.getRandomValues(walletId);
  const builder = new LazorkitInstructionBuilder();

  // 2. Find PDAs
  const [walletAccount, bump] = await findWalletAccount(walletId);
  const [walletVault, walletBump] = await findWalletVault(walletAccount);

  // 3. Create authority
  const keyPair = await crypto.subtle.generateKey(
    {
      name: 'Ed25519',
      namedCurve: 'Ed25519',
    },
    true,
    ['sign', 'verify']
  );
  const authority = new Ed25519Authority({ keypair: keyPair });

  // 4. Build CreateSmartWallet instruction manually
  const authorityPublicKey = await authority.getPublicKey();
  const authorityData = authorityPublicKey instanceof Uint8Array 
    ? authorityPublicKey 
    : new Uint8Array(32); // Placeholder
  const createInstruction = builder.buildCreateSmartWalletInstruction({
    walletAccount,
    payer: '11111111111111111111111111111111' as Address,
    walletVault,
    args: {
      id: walletId,
      bump,
      walletBump,
      firstAuthorityType: AuthorityType.Ed25519,
      firstAuthorityDataLen: authorityData.length,
      numPluginRefs: 0,
      rolePermission: RolePermission.AllButManageAuthority,
    },
    firstAuthorityData: authorityData,
  });

  console.log('Create instruction:', createInstruction);

  // 5. Build Sign instruction manually
  const innerInstructions = [
    {
      programAddress: '11111111111111111111111111111111' as Address,
      accounts: [
        { address: walletVault, role: 'writable' },
        { address: '11111111111111111111111111111112' as Address, role: 'writable' },
      ],
      data: new Uint8Array([2, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0]),
    },
  ];

  const instructionPayload = await serializeInstructions(innerInstructions);
  const currentSlot = 1000n;
  const messageHash = await buildMessageHash({
    instructionPayload,
    authorityType: AuthorityType.Ed25519,
  });

  const authorityPayload = await buildAuthorityPayload({
    authority,
    message: messageHash,
  });

  const signInstruction = builder.buildSignInstruction({
    walletAccount,
    walletVault,
    args: {
      instructionPayloadLen: instructionPayload.length,
      authorityId: 0,
    },
    instructionPayload,
    authorityPayload,
  });

  console.log('Sign instruction:', signInstruction);
}

main().catch(console.error);
