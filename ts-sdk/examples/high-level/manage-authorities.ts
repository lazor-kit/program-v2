/**
 * Example: Manage wallet authorities using high-level API
 * 
 * This example demonstrates how to add, update, and remove authorities
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

  // 2. Add a new Ed25519 authority
  const newKeyPair = await crypto.subtle.generateKey(
    {
      name: 'Ed25519',
      namedCurve: 'Ed25519',
    },
    true,
    ['sign', 'verify']
  );
  const newAuthority = new Ed25519Authority({ keypair: newKeyPair });
  const newAuthorityPublicKey = await newAuthority.getPublicKey();
  const newAuthorityData = newAuthorityPublicKey instanceof Uint8Array 
    ? newAuthorityPublicKey 
    : new Uint8Array(32); // Placeholder

  const addAuthorityInstruction = await wallet.buildAddAuthorityInstruction({
    newAuthorityType: 1, // Ed25519
    newAuthorityData,
    numPluginRefs: 0,
    rolePermission: RolePermission.ExecuteOnly,
  });

  console.log('Add authority instruction:', addAuthorityInstruction);

  // 3. Remove an authority (by ID)
  const removeAuthorityInstruction = await wallet.buildRemoveAuthorityInstruction({
    authorityToRemoveId: 1, // Authority ID to remove
  });

  console.log('Remove authority instruction:', removeAuthorityInstruction);
}

main().catch(console.error);
