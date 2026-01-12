/**
 * Program IDs for testing
 * 
 * These are loaded from deployment info or keypair files
 */

import { readFileSync, existsSync } from 'fs';
import { join } from 'path';
import type { Address } from '@solana/kit';
import { execSync } from 'child_process';

const FIXTURES_DIR = join(__dirname, '../fixtures/keypairs');
const DEPLOYMENT_INFO_PATH = join(FIXTURES_DIR, 'deployment-info.json');

/**
 * Get program ID from keypair file
 */
function getProgramIdFromKeypair(keypairPath: string): Address {
  if (!existsSync(keypairPath)) {
    throw new Error(`Keypair file not found: ${keypairPath}`);
  }

  try {
    const output = execSync(`solana-keygen pubkey ${keypairPath}`, { encoding: 'utf-8' });
    return output.trim() as Address;
  } catch (error) {
    throw new Error(`Failed to get program ID from keypair: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/**
 * Load program IDs from deployment info or keypair files
 */
export function loadProgramIds(): {
  main: Address;
  solLimit: Address;
  whitelist: Address;
} {
  // Try to load from deployment info first
  if (existsSync(DEPLOYMENT_INFO_PATH)) {
    try {
      const deploymentInfo = JSON.parse(readFileSync(DEPLOYMENT_INFO_PATH, 'utf-8'));
      return {
        main: deploymentInfo.programs.main.programId as Address,
        solLimit: deploymentInfo.programs.solLimit.programId as Address,
        whitelist: deploymentInfo.programs.whitelist.programId as Address,
      };
    } catch (error) {
      console.warn('Failed to load deployment info, falling back to keypair files:', error);
    }
  }

  // Fallback to keypair files
  const mainKeypairPath = join(FIXTURES_DIR, 'lazorkit-v2-keypair.json');
  const solLimitKeypairPath = join(FIXTURES_DIR, 'sol-limit-plugin-keypair.json');
  const whitelistKeypairPath = join(FIXTURES_DIR, 'program-whitelist-plugin-keypair.json');

  return {
    main: getProgramIdFromKeypair(mainKeypairPath),
    solLimit: getProgramIdFromKeypair(solLimitKeypairPath),
    whitelist: getProgramIdFromKeypair(whitelistKeypairPath),
  };
}

/**
 * Get main program ID (for use in tests)
 */
export function getMainProgramId(): Address {
  try {
    const programIds = loadProgramIds();
    return programIds.main;
  } catch (error) {
    // Fallback to default mainnet program ID if keypairs not found
    console.warn('Using default mainnet program ID. Run deployment script to use test program IDs.');
    return 'BAXwCwbBbs5WmdUkG9EEtFoLsYq2vRADBkdShbRN7w1P' as Address;
  }
}
