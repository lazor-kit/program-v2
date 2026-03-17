/**
 * LazorKit TypeScript SDK — web3.js v1 (Legacy) Edition
 *
 * Auto-generated instruction builders from Solita,
 * plus handwritten utilities for PDA derivation,
 * compact instruction packing, and the LazorWeb3Client wrapper.
 */

// Auto-generated from Solita (instructions, program constants)
export * from "./generated";

// Handwritten utilities
export {
  findWalletPda,
  findVaultPda,
  findAuthorityPda,
  findSessionPda,
  findConfigPda,
  findTreasuryShardPda,
} from "./utils/pdas";
export * from "./utils/packing";
export { LazorWeb3Client } from "./utils/client";

