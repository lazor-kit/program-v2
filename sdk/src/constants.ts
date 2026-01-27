import { address } from "@solana/addresses";

// Update this with the actual deployed program ID
export const LAZORKIT_PROGRAM_ID = address("2r5xXopRxWYcKHVrrzGrwfRJb3N2DSBkMgG93k6Z8ZFC");

export const SEEDS = {
    WALLET: "wallet",
    VAULT: "vault",
    AUTHORITY: "authority",
    SESSION: "session",
} as const;
