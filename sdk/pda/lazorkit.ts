import { PublicKey } from "@solana/web3.js";

// Mirror on-chain seeds
export const CONFIG_SEED = Buffer.from("config");
export const WHITELIST_RULE_PROGRAMS_SEED = Buffer.from(
  "whitelist_rule_programs"
);
export const SMART_WALLET_SEED = Buffer.from("smart_wallet");
export const SMART_WALLET_CONFIG_SEED = Buffer.from("smart_wallet_config");
export const SMART_WALLET_AUTHENTICATOR_SEED = Buffer.from(
  "smart_wallet_authenticator"
);
export const CPI_COMMIT_SEED = Buffer.from("cpi_commit");

export function deriveConfigPda(programId: PublicKey): PublicKey {
  return PublicKey.findProgramAddressSync([CONFIG_SEED], programId)[0];
}

export function deriveWhitelistRuleProgramsPda(
  programId: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [WHITELIST_RULE_PROGRAMS_SEED],
    programId
  )[0];
}

export function deriveSmartWalletPda(
  programId: PublicKey,
  walletId: bigint
): PublicKey {
  const idBytes = new Uint8Array(8);
  new DataView(idBytes.buffer).setBigUint64(0, walletId, true);
  return PublicKey.findProgramAddressSync(
    [SMART_WALLET_SEED, idBytes],
    programId
  )[0];
}

export function deriveSmartWalletConfigPda(
  programId: PublicKey,
  smartWallet: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [SMART_WALLET_CONFIG_SEED, smartWallet.toBuffer()],
    programId
  )[0];
}

// Must match on-chain: sha256(passkey(33) || wallet(32))
export function hashPasskeyWithWallet(
  passkeyCompressed33: Uint8Array,
  wallet: PublicKey
): Buffer {
  const { sha256 } = require("js-sha256");
  const buf = Buffer.alloc(65);
  Buffer.from(passkeyCompressed33).copy(buf, 0);
  wallet.toBuffer().copy(buf, 33);
  return Buffer.from(sha256.arrayBuffer(buf)).subarray(0, 32);
}

export function deriveSmartWalletAuthenticatorPda(
  programId: PublicKey,
  smartWallet: PublicKey,
  passkeyCompressed33: Uint8Array
): [PublicKey, number] {
  const hashed = hashPasskeyWithWallet(passkeyCompressed33, smartWallet);
  return PublicKey.findProgramAddressSync(
    [SMART_WALLET_AUTHENTICATOR_SEED, smartWallet.toBuffer(), hashed],
    programId
  );
}

export function deriveCpiCommitPda(
  programId: PublicKey,
  smartWallet: PublicKey,
  authorizedNonceLe8: Buffer
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [CPI_COMMIT_SEED, smartWallet.toBuffer(), authorizedNonceLe8],
    programId
  )[0];
}
