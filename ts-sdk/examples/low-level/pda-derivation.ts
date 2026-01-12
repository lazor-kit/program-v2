/**
 * Example: PDA derivation utilities
 * 
 * This example demonstrates how to use PDA utilities
 * for wallet account and vault derivation.
 */

import { 
  findWalletAccount,
  findWalletVault,
  createWalletAccountSignerSeeds,
  createWalletVaultSignerSeeds,
  LAZORKIT_PROGRAM_ID,
} from '@lazorkit/sdk';

async function main() {
  // 1. Generate wallet ID
  const walletId = new Uint8Array(32);
  crypto.getRandomValues(walletId);

  // 2. Find wallet account PDA
  const [walletAccount, bump] = await findWalletAccount(walletId);
  console.log('Wallet Account:', walletAccount);
  console.log('Bump:', bump);

  // 3. Find wallet vault PDA
  const [walletVault, walletBump] = await findWalletVault(walletAccount);
  console.log('Wallet Vault:', walletVault);
  console.log('Wallet Bump:', walletBump);

  // 4. Create signer seeds for wallet account
  const accountSeeds = createWalletAccountSignerSeeds(walletId, bump);
  console.log('Account signer seeds:', accountSeeds);

  // 5. Create signer seeds for wallet vault
  const vaultSeeds = createWalletVaultSignerSeeds(walletAccount, walletBump);
  console.log('Vault signer seeds:', vaultSeeds);

  // 6. Use custom program ID
  const customProgramId = 'CustomProgramId' as any;
  const [customWalletAccount, customBump] = await findWalletAccount(
    walletId,
    customProgramId
  );
  console.log('Custom Wallet Account:', customWalletAccount);
}

main().catch(console.error);
