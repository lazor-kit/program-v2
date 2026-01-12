/**
 * Example: Session management using high-level API
 * 
 * This example demonstrates how to create and manage sessions
 * using the high-level LazorkitWallet API.
 */

import { createSolanaRpc } from '@solana/kit';
import { 
  LazorkitWallet, 
  Ed25519Authority,
  generateSessionKey,
  calculateSessionExpiration,
  isSessionExpired,
} from '@lazorkit/sdk';
import type { Address } from '@solana/kit';

async function main() {
  // 1. Setup
  const rpc = createSolanaRpc('https://api.mainnet-beta.solana.com');
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

  // 2. Create a session with auto-generated key
  const createSessionInstruction1 = await wallet.buildCreateSessionInstruction();
  console.log('Create session (auto key):', createSessionInstruction1);

  // 3. Create a session with custom key and duration
  const sessionKey = generateSessionKey();
  const duration = 2000n; // 2000 slots
  const createSessionInstruction2 = await wallet.buildCreateSessionInstruction({
    sessionKey,
    duration,
  });
  console.log('Create session (custom):', createSessionInstruction2);

  // 4. Check session expiration
  const currentSlot = await wallet.getCurrentSlot();
  const expirationSlot = calculateSessionExpiration(currentSlot, duration);
  const expired = isSessionExpired(expirationSlot, currentSlot);
  console.log('Session expired?', expired);
  console.log('Expiration slot:', expirationSlot);
}

main().catch(console.error);
