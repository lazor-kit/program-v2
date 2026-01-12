/**
 * Example: Create a new Lazorkit wallet using high-level API
 * 
 * This example demonstrates how to create a new wallet with an Ed25519 authority
 * using the high-level LazorkitWallet API.
 */

import { createSolanaRpc } from '@solana/kit';
import { 
  LazorkitWallet, 
  Ed25519Authority,
  RolePermission,
} from '@lazorkit/sdk';
import type { Address } from '@solana/kit';

async function main() {
  // 1. Setup RPC client
  const rpc = createSolanaRpc('https://api.mainnet-beta.solana.com');

  // 2. Generate wallet ID (32 bytes)
  const walletId = new Uint8Array(32);
  crypto.getRandomValues(walletId);

  // 3. Create Ed25519 authority keypair
  const keyPair = await crypto.subtle.generateKey(
    {
      name: 'Ed25519',
      namedCurve: 'Ed25519',
    },
    true, // extractable
    ['sign', 'verify']
  );

  const authority = new Ed25519Authority({ keypair: keyPair });

  // 4. Fee payer address (you would get this from your wallet)
  const feePayer = '11111111111111111111111111111111' as Address; // Replace with actual address

  // 5. Initialize wallet (will create if doesn't exist)
  const wallet = await LazorkitWallet.initialize({
    rpc,
    walletId,
    authority,
    feePayer,
  });

  console.log('Wallet initialized!');
  console.log('Wallet Account:', wallet.getWalletAccount());
  console.log('Wallet Vault:', wallet.getWalletVault());
  console.log('Authority ID:', wallet.getAuthorityId());
}

main().catch(console.error);
