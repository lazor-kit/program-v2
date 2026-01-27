
/// <reference path="./ecdsa.d.ts" />
import { LazorKitClient } from "../src/client";
import { AuthType } from "../src/types";
import { createSolanaRpc } from "@solana/rpc";
import { createKeyPairSignerFromBytes } from "@solana/signers";
import { getBase64EncodedWireTransaction } from "@solana/transactions";
import { address } from "@solana/addresses";
import ECDSA from "ecdsa-secp256r1";
import * as dotenv from "dotenv";
import bs58 from "bs58";

// Helper to pack instructions for Execute
// See program/src/compact.rs
// byte format: [num_ixs(1)] [ [prog_idx(1)][num_accs(1)][acc_idxs...][data_len(2)][data...] ] ...
function packInstructions(
    instructions: {
        programId: string;
        keys: { pubkey: string; isSigner: boolean; isWritable: boolean }[];
        data: Uint8Array;
    }[],
    staticAccountKeys: string[] // [payer, wallet, authority, vault]
): { packed: Uint8Array; remainingAccounts: any[] } {
    const remainingAccountsMap = new Map<string, number>();
    const remainingAccountsList: any[] = [];

    // Initialize map with static accounts
    const allAccountsMap = new Map<string, number>();
    staticAccountKeys.forEach((key, idx) => allAccountsMap.set(key, idx));

    const packedInstructions: Uint8Array[] = [];

    for (const ix of instructions) {
        // Resolve Program ID Index
        let progIdx = allAccountsMap.get(ix.programId);
        if (progIdx === undefined) {
            progIdx = staticAccountKeys.length + remainingAccountsList.length;
            remainingAccountsList.push({ address: ix.programId, role: 0 }); // Readonly
            allAccountsMap.set(ix.programId, progIdx);
        }

        // Resolve Account Indexes
        const accIdxs: number[] = [];
        for (const acc of ix.keys) {
            let accIdx = allAccountsMap.get(acc.pubkey);
            if (accIdx === undefined) {
                accIdx = staticAccountKeys.length + remainingAccountsList.length;
                // Determine role: writable? signer?
                // For remaining accounts passed to Execute, signer isn't usually valid unless passed explicitly as signer meta.
                // But contract can sign for Vault.
                const role = acc.isWritable ? 1 : 0;
                // Note: The role in AccountMeta for remainingAccounts just needs to match what the contract needs.
                remainingAccountsList.push({ address: acc.pubkey, role });
                allAccountsMap.set(acc.pubkey, accIdx);
            }
            accIdxs.push(accIdx);
        }

        // Pack: [prog_idx][num_accs][acc_idxs...][data_len(2)][data...]
        const meta = new Uint8Array(2 + accIdxs.length + 2 + ix.data.length);
        let offset = 0;
        meta[offset++] = progIdx; // prog_idx
        meta[offset++] = accIdxs.length; // num_accs
        meta.set(new Uint8Array(accIdxs), offset);
        offset += accIdxs.length;

        // Data Len (u16 LE)
        meta[offset++] = ix.data.length & 0xFF;
        meta[offset++] = (ix.data.length >> 8) & 0xFF;

        // Data
        meta.set(ix.data, offset);

        packedInstructions.push(meta);
    }

    // Final Pack: [num_ixs] + [ix_bytes...]
    const totalLen = 1 + packedInstructions.reduce((acc, curr) => acc + curr.length, 0);
    const finalBuffer = new Uint8Array(totalLen);
    finalBuffer[0] = instructions.length;
    let offset = 1;
    for (const buf of packedInstructions) {
        finalBuffer.set(buf, offset);
        offset += buf.length;
    }

    return { packed: finalBuffer, remainingAccounts: remainingAccountsList };
}

async function waitForConfirmation(rpc: any, signature: string) {
    console.log(`    Waiting for confirmation...`);
    let retries = 30;
    while (retries > 0) {
        const response = await rpc.getSignatureStatuses([signature]).send();
        const status = response.value[0];
        if (status && (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized")) {
            if (status.err) throw new Error(`Transaction failed: ${JSON.stringify(status.err)}`);
            console.log("    Confirmed!");
            return;
        }
        await new Promise(resolve => setTimeout(resolve, 2000));
        retries--;
    }
    throw new Error("Transaction not confirmed");
}

dotenv.config();

// Mock describe/it for standalone execution
const describe = async (name: string, fn: () => Promise<void>) => {
    console.log(`\nRunning Test Suite: ${name}`);
    await fn();
};

const it = async (name: string, fn: () => Promise<void>) => {
    try {
        await fn();
        console.log(`  ✓ ${name}`);
    } catch (e) {
        console.error(`  ✗ ${name}`);
        console.error(e);
        process.exit(1);
    }
};

(async () => {
    await describe("LazorKit SDK E2E (Devnet)", async () => {
        const rpcUrl = process.env.RPC_URL;
        if (!rpcUrl) {
            console.error("Skipping test: RPC_URL not found in .env");
            return;
        }

        // Define Rpc type explicit or let inference handle it
        // Note: createRpc with http transport returns an Rpc<SolanaRpcApi> compatible object
        // but we might need to handle fetch implementation in Node if not global.
        // Modern Node has fetch.

        // Use createRpc from @solana/rpc which handles transport
        // Correct usage for v2: createRpc({ transport: http(url) }) or createJsonRpc(url)?
        // Let's check imports. createRpc is generic. createJsonRpc is a convenience?
        // Let's use createJsonRpc if available, or construct transport.
        // Actually, let's use the explicit transport construction to be safe with v2 patterns.
        // import { createHttpTransport } from "@solana/rpc-transport-http"; -- might be needed?
        // The @solana/rpc package usually exports everything.

        // Simplest v2:
        const rpc = createSolanaRpc(rpcUrl);

        const client = new LazorKitClient({ rpc });

        let payer: any;

        if (process.env.PRIVATE_KEY) {
            const secretKey = bs58.decode(process.env.PRIVATE_KEY);
            payer = await createKeyPairSignerFromBytes(secretKey);
            console.log("Using Payer:", payer.address);
        } else {
            console.warn("No PRIVATE_KEY provided. Tests will fail signatures.");
            return;
        }

        // ---------------------------------------------------------
        // Test State
        // ---------------------------------------------------------
        const userSeed = new Uint8Array(32);
        crypto.getRandomValues(userSeed);
        const { getAddressEncoder } = require("@solana/addresses");
        // We'll calculate PDAs manually or fetch them after creation
        let walletPda: any;
        let vaultPda: any;

        let authorityPda: any; // The initial owner authority
        let newAdminPubkey: Uint8Array; // To store for removal test
        let sessionSigner: any; // To store for session execution test

        // We need these for packing instructions later
        const SYSTEM_PROGRAM_ID = "11111111111111111111111111111111";

        await it("Create Wallet on Devnet", async () => {
            const authPubkey = getAddressEncoder().encode(payer.address);
            console.log("    User Seed:", Buffer.from(userSeed).toString('hex'));

            const tx = await client.createWallet({
                payer,
                userSeed,
                authType: AuthType.Ed25519,
                authPubkey,
                credentialHash: new Uint8Array(32)
            });

            console.log("    Sending Transaction...");
            const encoded = getBase64EncodedWireTransaction(tx);
            const sig = await rpc.sendTransaction(encoded, { encoding: "base64" }).send();
            console.log("    CreateWallet Signature:", sig);
            await waitForConfirmation(rpc, sig);
        });

        // Need imports for PDA derivation to get addresses for Execute
        const { findWalletPDA, findVaultPDA, findAuthorityPDA } = require("../src/utils");
        const { LAZORKIT_PROGRAM_ID } = require("../src/constants");

        await it("Setup PDAs", async () => {
            const authPubkey = getAddressEncoder().encode(payer.address);
            const [w] = await findWalletPDA(userSeed, LAZORKIT_PROGRAM_ID);
            const [v] = await findVaultPDA(w, LAZORKIT_PROGRAM_ID);
            // Ed25519 auth seed is pubkey
            const [a] = await findAuthorityPDA(w, authPubkey.slice(0, 32), LAZORKIT_PROGRAM_ID);
            walletPda = w;
            vaultPda = v;
            authorityPda = a;
            console.log("    Wallet:", walletPda);
            console.log("    Vault:", vaultPda);
            console.log("    Authority:", authorityPda);

            // Debug Owners
            const wInfo = await rpc.getAccountInfo(w).send();
            console.log("    Wallet Owner:", wInfo.value?.owner);
            const aInfo = await rpc.getAccountInfo(a).send();
            console.log("    Authority Owner:", aInfo.value?.owner);
            console.log("    Expected Program:", LAZORKIT_PROGRAM_ID);
        });

        await it("Fund Vault", async () => {
            console.log("    Funding Vault (Skipped - using Memo which needs no funding)...");
        });

        await it("Execute (System Transfer)", async () => {
            // Execute a 0-lamport transfer to verify CPI
            // Dest: Payer
            const SYSTEM_PROGRAM = "11111111111111111111111111111111";

            // Transfer Instruction: [2, 0, 0, 0, lamports(8 bytes)]
            // 0 lamports
            const data = new Uint8Array(4 + 8);
            data[0] = 2; // Transfer
            // Rest 0 is 0 lamports.

            // Accounts: [From (Vault), To (Payer)]
            // Vault is index 3. Payer is index 0.
            // But packInstructions handles this.

            const sysInfo = await rpc.getAccountInfo(address(SYSTEM_PROGRAM)).send();
            console.log("    SystemProgram Executable:", sysInfo.value?.executable);

            const staticKeys = [payer.address, walletPda, authorityPda, vaultPda];

            // Instruction keys
            // From: Vault (Writable, Signer - via CPI)
            // To: Payer (Writable)
            const transferKeys = [
                { pubkey: vaultPda, isSigner: true, isWritable: true },
                { pubkey: payer.address, isSigner: false, isWritable: true }
            ];

            const { packed, remainingAccounts } = packInstructions([{
                programId: SYSTEM_PROGRAM,
                keys: transferKeys,
                data: data
            }], staticKeys);

            console.log("    Packed Instructions (Hex):", Buffer.from(packed).toString('hex'));
            console.log("    Remaining Accounts:", remainingAccounts);

            const tx = await client.execute({
                payer,
                wallet: walletPda,
                authority: payer, // Using payer as authority (signer)
                instructions: packed,
                remainingAccounts
            });

            const encoded = getBase64EncodedWireTransaction(tx);
            const txBytes = Buffer.from(encoded, 'base64');
            console.log("    Execute Transfer Sig:", txBytes.byteLength < 1232 ? "✅" : "❌", `Tx Size: ${txBytes.byteLength} bytes`);
            if (txBytes.byteLength > 1232) {
                console.warn("    ⚠️ Transaction size exceeds Solana limit (1232 bytes).");
            }

            const sig = await rpc.sendTransaction(encoded, { encoding: "base64" }).send();
            console.log("    Execute Transfer Sig:", sig);
            await waitForConfirmation(rpc, sig);
        });

        await it("Add Authority (Admin)", async () => {
            // Add a new authority (Role: Admin)
            const newEdKeyBytes = new Uint8Array(32);
            crypto.getRandomValues(newEdKeyBytes); // Virtual Pubkey

            console.log("    Adding new Admin Authority...");

            const tx = await client.addAuthority({
                payer,
                wallet: walletPda,
                adminAuthority: payer, // Current owner
                newAuthType: AuthType.Ed25519, // Use Ed25519
                newAuthRole: 1, // Admin
                newPubkey: newEdKeyBytes,
                newHash: new Uint8Array(32), // Unused
            });

            newAdminPubkey = newEdKeyBytes;

            const encoded = getBase64EncodedWireTransaction(tx);
            const sig = await rpc.sendTransaction(encoded, { encoding: "base64" }).send();
            console.log("    AddAuthority Sig:", sig);
            await waitForConfirmation(rpc, sig);
        });

        await it("Remove Authority (Admin)", async () => {
            // Remove the admin we just added
            // We need to derive its PDA first.
            const [targetAuthPda] = await findAuthorityPDA(walletPda, newAdminPubkey, LAZORKIT_PROGRAM_ID);

            console.log("    Removing Authority:", targetAuthPda);

            const tx = await client.removeAuthority({
                payer,
                wallet: walletPda,
                adminAuthority: payer, // Owner removes Admin
                targetAuthority: targetAuthPda,
                refundDestination: payer.address // Refund rent to payer
            });

            const encoded = getBase64EncodedWireTransaction(tx);
            const sig = await rpc.sendTransaction(encoded, { encoding: "base64" }).send();
            console.log("    RemoveAuthority Sig:", sig);
            await waitForConfirmation(rpc, sig);
        });

        await it("Create Session", async () => {
            const { generateKeyPairSigner } = require("@solana/signers");
            // Generate a valid KeyPair for the session
            sessionSigner = await generateKeyPairSigner();

            // Ed25519 public key bytes as session key
            const sessionKey = getAddressEncoder().encode(sessionSigner.address);

            // 1 hour expiration
            const expiresAt = BigInt(Math.floor(Date.now() / 1000) + 3600);

            console.log("    Creating Session...");
            const tx = await client.createSession({
                payer,
                wallet: walletPda,
                authorizer: payer, // Owner
                sessionKey,
                expiresAt
            });

            const encoded = getBase64EncodedWireTransaction(tx);
            const sig = await rpc.sendTransaction(encoded, { encoding: "base64" }).send();
            console.log("    CreateSession Sig:", sig);
            await waitForConfirmation(rpc, sig);
        });

        await it("Execute (Session Key)", async () => {
            // Execute a transfer using the Session Key
            const SYSTEM_PROGRAM = "11111111111111111111111111111111";
            const data = new Uint8Array(12);
            data[0] = 2; // Transfer
            // 0 lamports

            const staticKeys = [payer.address, walletPda, authorityPda, vaultPda]; // Vault is index 3

            const transferKeys = [
                { pubkey: vaultPda, isSigner: true, isWritable: true },
                { pubkey: payer.address, isSigner: false, isWritable: true }
            ];

            const { packed, remainingAccounts } = packInstructions([{
                programId: SYSTEM_PROGRAM,
                keys: transferKeys,
                data: data
            }], staticKeys);

            console.log("    Executing with Session Key...");
            const tx = await client.execute({
                payer,
                wallet: walletPda,
                authority: sessionSigner, // Signing with Session Key
                instructions: packed,
                remainingAccounts,
                isSession: true // Flag to use findSessionPDA
            });

            const encoded = getBase64EncodedWireTransaction(tx);
            const sig = await rpc.sendTransaction(encoded, { encoding: "base64" }).send();
            console.log("    Execute (Session) Sig:", sig);
            await waitForConfirmation(rpc, sig);
        });

        await it("Transfer Ownership", async () => {
            const { generateKeyPairSigner } = require("@solana/signers");
            const newOwnerKp = await generateKeyPairSigner();
            const newOwnerBytes = getAddressEncoder().encode(newOwnerKp.address);

            console.log("    Transferring Ownership to:", newOwnerKp.address);
            const tx = await client.transferOwnership({
                payer,
                wallet: walletPda,
                currentOwner: payer,
                newType: AuthType.Ed25519,
                newPubkey: newOwnerBytes,
                newHash: new Uint8Array(32)
            });

            const encoded = getBase64EncodedWireTransaction(tx);
            const sig = await rpc.sendTransaction(encoded, { encoding: "base64" }).send();
            console.log("    TransferOwnership Sig:", sig);
            await waitForConfirmation(rpc, sig);
        });
    });
})();
