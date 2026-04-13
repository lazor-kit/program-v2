/**
 * @deprecated Import from './client' instead. This file is a backward-compat shim.
 *
 * The old monolithic wrapper has been split into focused modules:
 *   - client.ts   — LazorKitClient (unified API with discriminated signer types)
 *   - signing.ts  — Secp256r1 signing helpers & data payload builders
 *   - compact.ts  — buildCompactLayout (TransactionInstruction → CompactInstruction)
 *   - types.ts    — Signer type definitions (AdminSigner, ExecuteSigner, etc.)
 */
export { LazorKitClient } from './client';
