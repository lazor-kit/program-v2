import { PublicKey } from "@solana/web3.js";

export const RULE_SEED = Buffer.from("rule");

export function deriveRulePda(
  programId: PublicKey,
  smartWalletAuthenticator: PublicKey
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [RULE_SEED, smartWalletAuthenticator.toBuffer()],
    programId
  )[0];
}
