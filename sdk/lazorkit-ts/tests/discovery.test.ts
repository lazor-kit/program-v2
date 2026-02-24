
import { describe, it, expect, beforeAll } from "vitest";
import { PublicKey, Keypair } from "@solana/web3.js";
import { Address } from "@solana/kit";
import { setupTest, processInstruction } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../src";
import * as crypto from "crypto";

/**
 * Wallet Discovery Tests
 *
 * These tests verify the wallet discovery flow:
 * Given a credential_id (for Secp256r1) or pubkey (for Ed25519),
 * derive the Authority PDA and read the wallet pubkey from its data.
 *
 * This simulates what a Frontend would do via getProgramAccounts + memcmp
 * on a real RPC, but here we use direct PDA derivation + account read
 * because solana-bankrun doesn't support getProgramAccounts.
 *
 * Authority data layout:
 *   [0]  discriminator (1) — must be 2
 *   [1]  authority_type (1)
 *   [16..48] wallet pubkey (32 bytes)
 *   For Secp256r1: [52..84] credential_id_hash (32 bytes)
 *   For Ed25519: [48..80] pubkey (32 bytes)
 */

const HEADER_WALLET_OFFSET = 16;  // wallet pubkey in header
const SECP256R1_CRED_OFFSET = 52; // after header(48) + counter(4)
const ED25519_DATA_OFFSET = 48;   // after header(48)

describe("Wallet Discovery", () => {
    let context: any;
    let client: any;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    async function getRawAccountData(address: Address): Promise<Buffer | null> {
        const acc = await context.banksClient.getAccount(new PublicKey(address));
        if (!acc) return null;
        return Buffer.from(acc.data);
    }

    it("Discovery: Secp256r1 — credential_id → PDA → wallet", async () => {
        const userSeed = new Uint8Array(32).fill(200);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        // Simulate: user has a credentialId from WebAuthn
        const credentialId = Buffer.from(crypto.randomBytes(64));
        const credentialIdHash = crypto.createHash("sha256").update(credentialId).digest();

        const p256Pubkey = Buffer.alloc(33);
        p256Pubkey[0] = 0x02;
        crypto.randomBytes(32).copy(p256Pubkey, 1);

        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 1,
            authBump,
            authPubkey: p256Pubkey,
            credentialHash: credentialIdHash,
        }));

        // === Discovery Flow ===
        // Step 1: Frontend has credentialId → compute SHA256(credentialId)
        const discoveryHash = crypto.createHash("sha256").update(credentialId).digest();

        // Step 2: Try all known wallets (or use getProgramAccounts in production)
        // Here we simulate by trying the known wallet
        const [discoveredAuthPda] = await findAuthorityPda(walletPda, discoveryHash);

        // Step 3: Read the authority account
        const data = await getRawAccountData(discoveredAuthPda);
        expect(data).not.toBeNull();

        // Step 4: Verify it's an Authority account with Secp256r1
        expect(data![0]).toBe(2);  // discriminator = Authority
        expect(data![1]).toBe(1);  // authority_type = Secp256r1

        // Step 5: Extract credential_id_hash and verify it matches
        const storedCredHash = data!.subarray(SECP256R1_CRED_OFFSET, SECP256R1_CRED_OFFSET + 32);
        expect(Buffer.from(storedCredHash)).toEqual(discoveryHash);

        // Step 6: Extract wallet pubkey from header
        const discoveredWallet = new PublicKey(data!.subarray(HEADER_WALLET_OFFSET, HEADER_WALLET_OFFSET + 32));
        expect(discoveredWallet.toBase58()).toBe(new PublicKey(walletPda).toBase58());
    });

    it("Discovery: Ed25519 — pubkey → PDA → wallet", async () => {
        const userSeed = new Uint8Array(32).fill(201);
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const owner = Keypair.generate();
        const ownerPubkeyBytes = owner.publicKey.toBytes();
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerPubkeyBytes);

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: authPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: ownerPubkeyBytes,
            credentialHash: new Uint8Array(32),
        }));

        // === Discovery Flow ===
        // Step 1: Frontend has the Ed25519 pubkey
        // Step 2: Derive PDA using known wallet (or scan via getProgramAccounts)
        const [discoveredAuthPda] = await findAuthorityPda(walletPda, ownerPubkeyBytes);

        // Step 3: Read and verify
        const data = await getRawAccountData(discoveredAuthPda);
        expect(data).not.toBeNull();
        expect(data![0]).toBe(2);  // Authority
        expect(data![1]).toBe(0);  // Ed25519

        // Step 4: Verify stored pubkey
        const storedPubkey = data!.subarray(ED25519_DATA_OFFSET, ED25519_DATA_OFFSET + 32);
        expect(Buffer.from(storedPubkey)).toEqual(Buffer.from(ownerPubkeyBytes));

        // Step 5: Extract wallet from header
        const discoveredWallet = new PublicKey(data!.subarray(HEADER_WALLET_OFFSET, HEADER_WALLET_OFFSET + 32));
        expect(discoveredWallet.toBase58()).toBe(new PublicKey(walletPda).toBase58());
    });

    it("Discovery: Non-existent credential returns null", async () => {
        // Derive a PDA for a wallet + credential that was never created
        const fakeUserSeed = new Uint8Array(32).fill(202);
        const [fakeWallet] = await findWalletPda(fakeUserSeed);
        const fakeCredHash = Buffer.alloc(32, 0xFF);
        const [fakePda] = await findAuthorityPda(fakeWallet, fakeCredHash);

        const data = await getRawAccountData(fakePda);
        expect(data).toBeNull();
    });
});
