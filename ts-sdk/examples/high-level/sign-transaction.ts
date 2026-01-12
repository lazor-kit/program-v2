/**
 * Example: Sign and execute a transaction using high-level API
 * 
 * This example demonstrates how to sign and execute a transaction
 * using the high-level LazorkitWallet API.
 */

import { createSolanaRpc } from '@solana/kit';
import { 
  LazorkitWallet, 
  Ed25519Authority,
} from '@lazorkit/sdk';
import type { Address } from '@solana/kit';

async function main() {
  // 1. Setup RPC client
  const rpc = createSolanaRpc('https://api.mainnet-beta.solana.com');

  // 2. Load existing wallet
  const walletId = new Uint8Array(32); // Your wallet ID
  const keyPair = await crypto.subtle.generateKey(
    {
      name: 'Ed25519',
      namedCurve: 'Ed25519',
    },
    true,
    ['sign', 'verify']
  );
  const authority = new Ed25519Authority({ keypair: keyPair });
  const feePayer = '11111111111111111111111111111111' as Address;

  const wallet = await LazorkitWallet.initialize({
    rpc,
    walletId,
    authority,
    feePayer,
  });

  // 3. Build a transfer instruction (example)
  const transferInstruction = {
    programAddress: '11111111111111111111111111111111' as Address, // System Program
    accounts: [
      { address: wallet.getWalletVault(), role: 'writable' },
      { address: 'RecipientAddress' as Address, role: 'writable' },
    ],
    data: new Uint8Array([2, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 0]), // Transfer 100 lamports
  };

  // 4. Build Sign instruction
  const signInstruction = await wallet.buildSignInstruction({
    instructions: [transferInstruction],
    slot: await wallet.getCurrentSlot(),
  });

  console.log('Sign instruction built:', signInstruction);
  console.log('Next: Build transaction and send it using @solana/kit transaction APIs');
}

main().catch(console.error);
