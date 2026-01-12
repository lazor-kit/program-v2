/**
 * Deployment script for Lazorkit V2 contracts
 * 
 * This script deploys:
 * 1. Main Lazorkit V2 program
 * 2. Sol Limit plugin
 * 3. Program Whitelist plugin
 * 
 * Usage:
 *   ENABLE_DEPLOYMENT=true SOLANA_RPC_URL=http://localhost:8899 npm run deploy
 */

import { readFileSync, existsSync, writeFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { execSync } from 'child_process';
import { createSolanaRpc } from '@solana/kit';
import type { Address } from '@solana/kit';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

const RPC_URL = process.env.SOLANA_RPC_URL || 'http://localhost:8899';
const ENABLE_DEPLOYMENT = process.env.ENABLE_DEPLOYMENT === 'true';

interface DeploymentResult {
  programId: Address;
  signature: string;
  keypairPath: string;
}

/**
 * Load keypair from JSON file
 */
function loadKeypair(keypairPath: string): Uint8Array {
  if (!existsSync(keypairPath)) {
    throw new Error(`Keypair file not found: ${keypairPath}`);
  }

  const keypairData = JSON.parse(readFileSync(keypairPath, 'utf-8'));
  return new Uint8Array(keypairData);
}

/**
 * Get program ID from keypair
 */
async function getProgramIdFromKeypair(keypairPath: string): Promise<Address> {
  try {
    // Use solana-keygen to get public key from keypair
    const output = execSync(`solana-keygen pubkey ${keypairPath}`, { encoding: 'utf-8' });
    return output.trim() as Address;
  } catch (error) {
    throw new Error(`Failed to get program ID from keypair: ${error instanceof Error ? error.message : String(error)}`);
  }
}

/**
 * Deploy a program
 */
async function deployProgram(
  programName: string,
  soPath: string,
  keypairPath: string
): Promise<DeploymentResult> {
  console.log(`\n📦 Deploying ${programName}...`);
  console.log(`   SO Path: ${soPath}`);
  console.log(`   Keypair: ${keypairPath}`);

  if (!existsSync(soPath)) {
    throw new Error(`Program SO file not found: ${soPath}`);
  }

  if (!existsSync(keypairPath)) {
    throw new Error(`Keypair file not found: ${keypairPath}`);
  }

  // Get program ID
  const programId = await getProgramIdFromKeypair(keypairPath);
  console.log(`   Program ID: ${programId}`);

  // Deploy using solana CLI
  try {
    const deployCommand = `solana program deploy ${soPath} --program-id ${keypairPath} --url ${RPC_URL}`;
    console.log(`   Running: ${deployCommand}`);

    const output = execSync(deployCommand, { encoding: 'utf-8', stdio: 'pipe' });
    console.log(`   Output: ${output}`);

    // Extract signature from output
    const signatureMatch = output.match(/Program Id: (\w+)/);
    const deployedProgramId = signatureMatch ? signatureMatch[1] : programId;

    console.log(`✅ ${programName} deployed successfully!`);
    console.log(`   Program ID: ${deployedProgramId}`);

    return {
      programId: deployedProgramId as Address,
      signature: deployedProgramId, // Use program ID as signature identifier
      keypairPath,
    };
  } catch (error) {
    const errorMessage = error instanceof Error ? error.message : String(error);
    console.error(`❌ Failed to deploy ${programName}:`, errorMessage);
    throw error;
  }
}

/**
 * Main deployment function
 */
async function main() {
  if (!ENABLE_DEPLOYMENT) {
    console.log('⚠️  Deployment disabled. Set ENABLE_DEPLOYMENT=true to enable.');
    return;
  }

  console.log('🚀 Starting Lazorkit V2 Contract Deployment');
  console.log(`📍 RPC URL: ${RPC_URL}`);

  const projectRoot = join(__dirname, '../../..');
  const fixturesDir = join(__dirname, '../fixtures/keypairs');
  const targetDeployDir = join(projectRoot, 'target/deploy');

  const deployments: Record<string, DeploymentResult> = {};

  try {
    // 1. Deploy main Lazorkit V2 program
    const mainKeypairPath = join(fixturesDir, 'lazorkit-v2-keypair.json');
    const mainSoPath = join(targetDeployDir, 'lazorkit_v2.so');

    // Generate keypair if it doesn't exist
    if (!existsSync(mainKeypairPath)) {
      console.log('📝 Generating main program keypair...');
      execSync(`solana-keygen new --outfile ${mainKeypairPath} --no-bip39-passphrase`, { cwd: fixturesDir });
    }

    deployments.main = await deployProgram('Lazorkit V2', mainSoPath, mainKeypairPath);

    // 2. Deploy Sol Limit plugin
    const solLimitKeypairPath = join(fixturesDir, 'sol-limit-plugin-keypair.json');
    const solLimitSoPath = join(targetDeployDir, 'lazorkit_plugin_sol_limit.so');

    if (!existsSync(solLimitKeypairPath)) {
      console.log('📝 Generating Sol Limit plugin keypair...');
      execSync(`solana-keygen new --outfile ${solLimitKeypairPath} --no-bip39-passphrase`, { cwd: fixturesDir });
    }

    deployments.solLimit = await deployProgram('Sol Limit Plugin', solLimitSoPath, solLimitKeypairPath);

    // 3. Deploy Program Whitelist plugin
    const whitelistKeypairPath = join(fixturesDir, 'program-whitelist-plugin-keypair.json');
    const whitelistSoPath = join(targetDeployDir, 'lazorkit_plugin_program_whitelist.so');

    if (!existsSync(whitelistKeypairPath)) {
      console.log('📝 Generating Program Whitelist plugin keypair...');
      execSync(`solana-keygen new --outfile ${whitelistKeypairPath} --no-bip39-passphrase`, { cwd: fixturesDir });
    }

    deployments.whitelist = await deployProgram('Program Whitelist Plugin', whitelistSoPath, whitelistKeypairPath);

    // Save deployment info
    const deploymentInfo = {
      rpcUrl: RPC_URL,
      deployedAt: new Date().toISOString(),
      programs: {
        main: {
          programId: deployments.main.programId,
          keypairPath: deployments.main.keypairPath,
        },
        solLimit: {
          programId: deployments.solLimit.programId,
          keypairPath: deployments.solLimit.keypairPath,
        },
        whitelist: {
          programId: deployments.whitelist.programId,
          keypairPath: deployments.whitelist.keypairPath,
        },
      },
    };

    const deploymentInfoPath = join(fixturesDir, 'deployment-info.json');
    writeFileSync(deploymentInfoPath, JSON.stringify(deploymentInfo, null, 2));

    console.log('\n✅ All contracts deployed successfully!');
    console.log('\n📋 Deployment Summary:');
    console.log(`   Main Program: ${deployments.main.programId}`);
    console.log(`   Sol Limit Plugin: ${deployments.solLimit.programId}`);
    console.log(`   Program Whitelist Plugin: ${deployments.whitelist.programId}`);
    console.log(`\n💾 Deployment info saved to: ${deploymentInfoPath}`);

  } catch (error) {
    console.error('\n❌ Deployment failed:', error);
    process.exit(1);
  }
}

main().catch(console.error);
