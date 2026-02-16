
import { describe, it, expect, beforeAll } from "vitest";
import { PublicKey, Keypair } from "@solana/web3.js";
import { Address } from "@solana/kit";
import { setupTest, processInstruction, tryProcessInstruction } from "../common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "../../src";

describe("Instruction: CreateSession", () => {
    let context: any;
    let client: any;
    let walletPda: Address;
    let owner: Keypair;
    let ownerAuthPda: Address;

    beforeAll(async () => {
        ({ context, client } = await setupTest());

        const userSeed = new Uint8Array(32).fill(40);
        [walletPda] = await findWalletPda(userSeed);
        const [vaultPda] = await findVaultPda(walletPda);
        owner = Keypair.generate();
        let authBump;
        [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, owner.publicKey.toBytes());

        await processInstruction(context, client.createWallet({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            vault: vaultPda,
            authority: ownerAuthPda,
            userSeed,
            authType: 0,
            authBump,
            authPubkey: owner.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
        }));
    });

    it("Success: Owner creates a session key", async () => {
        const sessionKey = Keypair.generate();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.publicKey.toBase58() as Address);

        await processInstruction(context, client.createSession({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            session: sessionPda,
            sessionKey: sessionKey.publicKey.toBytes(),
            expiresAt: 999999999n,
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        const sessionAcc = await client.getSession(sessionPda);
        expect(sessionAcc.discriminator).toBe(3); // Session
        expect(sessionAcc.sessionKey).toEqual(sessionKey.publicKey.toBase58());
    });

    it("Failure: Spender cannot create a session key", async () => {
        const spender = Keypair.generate();
        const [spenderPda] = await findAuthorityPda(walletPda, spender.publicKey.toBytes());
        await processInstruction(context, client.addAuthority({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: ownerAuthPda,
            newAuthority: spenderPda,
            authType: 0,
            newRole: 2,
            authPubkey: spender.publicKey.toBytes(),
            credentialHash: new Uint8Array(32),
            authorizerSigner: { address: owner.publicKey.toBase58() as Address } as any,
        }), [owner]);

        const sessionKey = Keypair.generate();
        const [sessionPda] = await findSessionPda(walletPda, sessionKey.publicKey.toBase58() as Address);

        const result = await tryProcessInstruction(context, client.createSession({
            payer: { address: context.payer.publicKey.toBase58() as Address } as any,
            wallet: walletPda,
            adminAuthority: spenderPda,
            session: sessionPda,
            sessionKey: sessionKey.publicKey.toBytes(),
            expiresAt: 999999999n,
            authorizerSigner: { address: spender.publicKey.toBase58() as Address } as any,
        }), [spender]);

        expect(result.result).toContain("custom program error: 0xbba");
    });
});
