
import { describe, it, expect, beforeAll } from "vitest";
import {
    type Address,
    generateKeyPairSigner,
    getAddressEncoder,
    getAddressDecoder,
} from "@solana/kit";
import { setupTest, processInstruction, type TestContext } from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda } from "../../sdk/lazorkit-ts/src";

function getRandomSeed() {
    return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

const HEADER_WALLET_OFFSET = 16;  // wallet pubkey in header
const DATA_OFFSET = 48;           // after header(48) — same for both authority types
const ED25519_DATA_OFFSET = 48;   // after header(48)

describe("Wallet Discovery", () => {
    let context: TestContext;
    let client: any;

    beforeAll(async () => {
        ({ context, client } = await setupTest());
    });

    async function getRawAccountData(address: Address): Promise<Uint8Array | null> {
        const { value: acc } = await context.rpc.getAccountInfo(address, { encoding: 'base64' }).send();
        if (!acc) return null;
        const data = Array.isArray(acc.data) ? acc.data[0] : acc.data;
        return new Uint8Array(Buffer.from(data, 'base64'));
    }

    it("Discovery: Secp256r1 — credential_id → PDA → wallet", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        // Simulate: user has a credentialId from WebAuthn
        const credentialId = getRandomSeed();
        // Simple hash mock for test
        const credentialIdHash = new Uint8Array(32).fill(0).map((_, i) => credentialId[i] ^ 0xFF);

        const p256Pubkey = new Uint8Array(33).map(() => Math.floor(Math.random() * 256));
        p256Pubkey[0] = 0x02;

        const [authPda, authBump] = await findAuthorityPda(walletPda, credentialIdHash);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
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
        // Step 1: Frontend has credentialIdHash
        const discoveryHash = credentialIdHash;

        // Step 2: Try derived PDA
        const [discoveredAuthPda] = await findAuthorityPda(walletPda, discoveryHash);

        // Step 3: Read the authority account
        const data = await getRawAccountData(discoveredAuthPda);
        expect(data).not.toBeNull();

        // Step 4: Verify it's an Authority account with Secp256r1
        expect(data![0]).toBe(2);  // discriminator = Authority
        expect(data![1]).toBe(1);  // authority_type = Secp256r1

        // Step 5: Extract credential_id_hash and verify it matches
        const storedCredHash = data!.subarray(DATA_OFFSET, DATA_OFFSET + 32);
        expect(Uint8Array.from(storedCredHash)).toEqual(Uint8Array.from(discoveryHash));

        // Step 6: Extract wallet pubkey from header
        const discoveredWalletBytes = data!.subarray(HEADER_WALLET_OFFSET, HEADER_WALLET_OFFSET + 32);
        const discoveredWallet = getAddressDecoder().decode(discoveredWalletBytes);
        expect(discoveredWallet).toBe(walletPda);
    });

    it("Discovery: Ed25519 — pubkey → PDA → wallet", async () => {
        const userSeed = getRandomSeed();
        const [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);

        const owner = await generateKeyPairSigner();
        const ownerPubkeyBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
        const [authPda, authBump] = await findAuthorityPda(walletPda, ownerPubkeyBytes);

        await processInstruction(context, client.createWallet({
            payer: context.payer,
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
        const [discoveredAuthPda] = await findAuthorityPda(walletPda, ownerPubkeyBytes);

        // Read and verify
        const data = await getRawAccountData(discoveredAuthPda);
        expect(data).not.toBeNull();
        expect(data![0]).toBe(2);  // Authority
        expect(data![1]).toBe(0);  // Ed25519

        // Verify stored pubkey
        const storedPubkey = data!.subarray(ED25519_DATA_OFFSET, ED25519_DATA_OFFSET + 32);
        expect(Uint8Array.from(storedPubkey)).toEqual(ownerPubkeyBytes);

        // Extract wallet from header
        const discoveredWalletBytes = data!.subarray(HEADER_WALLET_OFFSET, HEADER_WALLET_OFFSET + 32);
        const discoveredWallet = getAddressDecoder().decode(discoveredWalletBytes);
        expect(discoveredWallet).toBe(walletPda);
    });

    it("Discovery: Non-existent credential returns null", async () => {
        const fakeUserSeed = getRandomSeed();
        const [fakeWallet] = await findWalletPda(fakeUserSeed);
        const fakeCredHash = new Uint8Array(32).fill(0xEE);
        const [fakePda] = await findAuthorityPda(fakeWallet, fakeCredHash);

        const data = await getRawAccountData(fakePda);
        expect(data).toBeNull();
    });
});
