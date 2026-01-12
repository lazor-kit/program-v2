/**
 * Example: Plugin management using high-level API
 * 
 * This example demonstrates how to add, update, and remove plugins
 * using the high-level LazorkitWallet API.
 */

import { createSolanaRpc } from '@solana/kit';
import { 
  LazorkitWallet, 
  Ed25519Authority,
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

  // 2. Add a plugin
  const pluginProgramId = '11111111111111111111111111111112' as Address;
  const pluginConfigAccount = '11111111111111111111111111111113' as Address;
  
  const addPluginInstruction = await wallet.buildAddPluginInstruction({
    pluginProgramId,
    pluginConfigAccount,
    priority: 0, // Highest priority
    enabled: true,
  });

  console.log('Add plugin instruction:', addPluginInstruction);

  // 3. Update a plugin
  const updatePluginInstruction = await wallet.buildUpdatePluginInstruction({
    pluginIndex: 0,
    priority: 1,
    enabled: false,
  });

  console.log('Update plugin instruction:', updatePluginInstruction);

  // 4. Remove a plugin
  const removePluginInstruction = await wallet.buildRemovePluginInstruction({
    pluginIndex: 0,
  });

  console.log('Remove plugin instruction:', removePluginInstruction);
}

main().catch(console.error);
