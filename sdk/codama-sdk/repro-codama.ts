
import {
    createSolanaRpc,
    createKeyPairSignerFromBytes,
    generateKeyPairSigner,
    createTransactionMessage,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    appendTransactionMessageInstruction,
    signTransactionMessageWithSigners,
    getBase64EncodedWireTransaction,
    getProgramDerivedAddress,
    getAddressEncoder,
    type Address,
    type TransactionSigner,
    type AccountMeta,
    AccountRole,
    lamports
} from "@solana/kit";
import {
    getCreateWalletInstruction,
    getAddAuthorityInstruction,
    getRemoveAuthorityInstruction,
    getCreateSessionInstruction,
    getExecuteInstruction
} from "./src/generated";
import bs58 from "bs58";
import dotenv from "dotenv";

dotenv.config({ path: "../lazorkit-sdk/.env" });

const LAZORKIT_PROGRAM_ID = "2r5xXopRxWYcKHVrrzGrwfRJb3N2DSBkMgG93k6Z8ZFC" as Address;
const SEEDS = {
    WALLET: "wallet",
    VAULT: "vault",
    AUTHORITY: "authority",
    SESSION: "session_marker"
};

const ENCODER = new TextEncoder();

async function findWalletPDA(userSeed: Uint8Array): Promise<Address> {
    const [pda] = await getProgramDerivedAddress({
        programAddress: LAZORKIT_PROGRAM_ID,
        seeds: [ENCODER.encode(SEEDS.WALLET), userSeed],
    });
    return pda;
}

async function findVaultPDA(walletPda: Address): Promise<Address> {
    const walletBytes = getAddressEncoder().encode(walletPda);
    const [pda] = await getProgramDerivedAddress({
        programAddress: LAZORKIT_PROGRAM_ID,
        seeds: [ENCODER.encode(SEEDS.VAULT), walletBytes],
    });
    return pda;
}

async function findAuthorityPDA(walletPda: Address, idSeed: Uint8Array): Promise<readonly [Address, number]> {
    const walletBytes = getAddressEncoder().encode(walletPda);
    return await getProgramDerivedAddress({
        programAddress: LAZORKIT_PROGRAM_ID,
        seeds: [ENCODER.encode(SEEDS.AUTHORITY), walletBytes as Uint8Array, idSeed],
    });
}

async function findSessionPDA(walletPda: Address, sessionKey: Uint8Array): Promise<readonly [Address, number]> {
    const walletBytes = getAddressEncoder().encode(walletPda);
    return await getProgramDerivedAddress({
        programAddress: LAZORKIT_PROGRAM_ID,
        seeds: [ENCODER.encode("session"), walletBytes as Uint8Array, sessionKey],
    });
}

function packInstructions(
    instructions: {
        programId: string;
        keys: { pubkey: string; isSigner: boolean; isWritable: boolean }[];
        data: Uint8Array;
    }[],
    staticAccountKeys: string[]
): { packed: Uint8Array; remainingAccounts: any[] } {
    const remainingAccountsList: any[] = [];
    const allAccountsMap = new Map<string, number>();
    staticAccountKeys.forEach((key, idx) => allAccountsMap.set(key, idx));

    const packedInstructions: Uint8Array[] = [];

    for (const ix of instructions) {
        let progIdx = allAccountsMap.get(ix.programId);
        if (progIdx === undefined) {
            progIdx = staticAccountKeys.length + remainingAccountsList.length;
            remainingAccountsList.push({ address: ix.programId, role: 0, isWritable: false, isSigner: false });
            allAccountsMap.set(ix.programId, progIdx);
        }

        const accIdxs: number[] = [];
        const accRoles: number[] = [];
        for (const acc of ix.keys) {
            let accIdx = allAccountsMap.get(acc.pubkey);
            if (accIdx === undefined) {
                accIdx = staticAccountKeys.length + remainingAccountsList.length;
                const role = acc.isWritable ? 1 : 0;
                remainingAccountsList.push({
                    address: acc.pubkey,
                    role,
                    isWritable: acc.isWritable,
                    isSigner: acc.isSigner
                });
                allAccountsMap.set(acc.pubkey, accIdx);
            }
            accIdxs.push(accIdx);

            // Bit 0: writable, Bit 1: signer
            let roleBits = 0;
            if (acc.isWritable) roleBits |= 1;
            if (acc.isSigner) roleBits |= 2;
            accRoles.push(roleBits);
        }

        const meta = new Uint8Array(2 + accIdxs.length + accRoles.length + 2 + ix.data.length);
        let offset = 0;
        meta[offset++] = progIdx;
        meta[offset++] = accIdxs.length;

        // Accounts
        meta.set(new Uint8Array(accIdxs), offset);
        offset += accIdxs.length;

        // Roles
        meta.set(new Uint8Array(accRoles), offset);
        offset += accRoles.length;

        // Data len
        meta[offset++] = ix.data.length & 0xFF;
        meta[offset++] = (ix.data.length >> 8) & 0xFF;

        // Data
        meta.set(ix.data, offset);
        packedInstructions.push(meta);
    }

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

(async () => {
    try {
        const rpc = createSolanaRpc("https://api.devnet.solana.com");

        let payer: TransactionSigner;
        if (process.env.PRIVATE_KEY) {
            payer = await createKeyPairSignerFromBytes(bs58.decode(process.env.PRIVATE_KEY));
        } else {
            console.log("No PRIVATE_KEY, generating random payer (requesting airdrop)");
            payer = await generateKeyPairSigner();
            try {
                const airdropSig = await rpc.requestAirdrop(payer.address, lamports(2_000_000_000n)).send();
                console.log("Airdrop requested:", airdropSig);
                // Wait for balance
                for (let i = 0; i < 10; i++) {
                    await new Promise(r => setTimeout(r, 1000));
                    const bal = await rpc.getBalance(payer.address).send();
                    if (bal.value > 0n) {
                        console.log("Funded:", bal.value);
                        break;
                    }
                }
            } catch (e) {
                console.warn("Airdrop failed (might be rate limited):", e);
            }
        }
        console.log("Payer:", payer.address);

        const userSeed = new Uint8Array(32);
        crypto.getRandomValues(userSeed);

        const walletPda = await findWalletPDA(userSeed);
        const vaultPda = await findVaultPDA(walletPda);

        const authSeed = getAddressEncoder().encode(payer.address) as Uint8Array;
        const [authorityPda, authBump] = await findAuthorityPDA(walletPda, authSeed);

        console.log("-- Config --");
        console.log("Wallet PDA:", walletPda);
        console.log("Vault PDA:", vaultPda);
        console.log("Owner Auth PDA:", authorityPda);

        // --- Step 1: Create Wallet ---
        console.log("\n[1/5] Creating Wallet...");
        const createWalletIx = getCreateWalletInstruction({
            payer,
            wallet: walletPda,
            vault: vaultPda,
            authority: authorityPda,
            userSeed,
            authType: 0, // Ed25519
            authBump,
            padding: new Uint8Array(6),
            authPubkey: authSeed
        });

        await sendAndConfirm(rpc, payer, [createWalletIx]);
        console.log("✅ Create Wallet Success!");

        // --- Step 1.5: Fund Vault ---
        console.log("\n[1.5/5] Funding Vault (0.01 SOL)...");
        const fundIx = {
            programAddress: "11111111111111111111111111111111" as Address,
            accounts: [
                { address: payer.address, role: AccountRole.WRITABLE_SIGNER, signer: payer },
                { address: vaultPda, role: AccountRole.WRITABLE }
            ],
            data: new Uint8Array([2, 0, 0, 0, 0x80, 0x96, 0x98, 0, 0, 0, 0, 0]) // 10,000,000 lamports (0.01 SOL)
        };
        await sendAndConfirm(rpc, payer, [fundIx as any]);
        console.log("✅ Fund Vault Success!");

        // Wait a bit and check balance
        await new Promise(r => setTimeout(r, 2000));
        const balance = await rpc.getBalance(vaultPda).send();
        console.log(`   Vault Balance: ${balance.value} lamports`);


        // --- Step 2: Add Secondary Authority (Admin) ---
        console.log("\n[2/5] Adding Secondary Authority (Admin)...");
        const adminSigner = await generateKeyPairSigner();
        const adminAuthSeed = getAddressEncoder().encode(adminSigner.address) as Uint8Array;
        const [adminAuthPda] = await findAuthorityPDA(walletPda, adminAuthSeed);

        const addAuthIx = getAddAuthorityInstruction({
            payer,
            wallet: walletPda,
            adminAuthority: authorityPda,
            newAuthority: adminAuthPda,
            authorityType: 0, // Ed25519
            newRole: 1, // Admin
            padding: new Uint8Array(6),
            newPubkey: adminAuthSeed
        });

        await sendAndConfirm(rpc, payer, [addAuthIx]);
        console.log("✅ Add Authority Success!");


        // --- Step 3: Create Session (authorized by Admin) ---
        console.log("\n[3/5] Creating Session (authorized by Admin)...");
        const sessionSigner = await generateKeyPairSigner();
        const sessionKey = getAddressEncoder().encode(sessionSigner.address) as Uint8Array;
        const [sessionPda] = await findSessionPDA(walletPda, sessionKey);
        const expiresAt = BigInt(Math.floor(Date.now() / 1000) + 3600);

        const createSessionIx = getCreateSessionInstruction({
            payer,
            wallet: walletPda,
            adminAuthority: adminAuthPda,
            session: sessionPda,
            sessionKey,
            expiresAt,
            authorizerSigner: adminSigner
        });

        await sendAndConfirm(rpc, payer, [createSessionIx]);
        console.log("✅ Create Session Success!");


        // --- Step 4: Execute Transfer (using Session) ---
        console.log("\n[4/5] Executing Transfer (using Session Key)...");
        const SYSTEM_PROGRAM = "11111111111111111111111111111111" as Address;
        const SYSVAR_INSTRUCTIONS = "Sysvar1nstructions1111111111111111111111111" as Address;

        const data = new Uint8Array(12);
        data[0] = 2; // Transfer
        data[4] = 0xE8; // 1000 lamports
        data[5] = 0x03;

        const staticKeys = [payer.address, walletPda, sessionPda, vaultPda, SYSVAR_INSTRUCTIONS];

        const transferKeys = [
            { pubkey: vaultPda, isSigner: true, isWritable: true },
            { pubkey: payer.address, isSigner: false, isWritable: true }
        ];

        const { packed, remainingAccounts } = packInstructions([{
            programId: SYSTEM_PROGRAM,
            keys: transferKeys,
            data: data
        }], staticKeys);

        const codamaRemainingAccounts: AccountMeta[] = remainingAccounts.map(acc => {
            let role = 0;
            if (acc.isWritable) {
                role = acc.isSigner ? 3 : 1;
            } else {
                role = acc.isSigner ? 2 : 0;
            }
            return { address: acc.address, role: role as any };
        });

        const executeIx = getExecuteInstruction({
            payer,
            wallet: walletPda,
            authority: sessionPda, // Session PDA Address
            vault: vaultPda,
            sysvarInstructions: SYSVAR_INSTRUCTIONS,
            instructions: packed
        }, { programAddress: LAZORKIT_PROGRAM_ID });

        const patchedExecuteIx = {
            ...executeIx,
            accounts: [
                ...executeIx.accounts,
                ...codamaRemainingAccounts,
                {
                    address: sessionSigner.address,
                    role: AccountRole.READONLY_SIGNER,
                    signer: sessionSigner
                } as any
            ]
        };

        await sendAndConfirm(rpc, payer, [patchedExecuteIx]);
        console.log("✅ Execute (Session) Success!");


        // --- Step 5: Remove Authority ---
        console.log("\n[5/5] Removing Authority (Admin)...");
        const removeAuthIx = getRemoveAuthorityInstruction({
            payer,
            wallet: walletPda,
            adminAuthority: authorityPda, // Use Owner PDA Address
            targetAuthority: adminAuthPda,
            refundDestination: payer.address,
            targetPubkey: adminAuthSeed,
            authorizerSigner: payer
        });

        await sendAndConfirm(rpc, payer, [removeAuthIx]);
        console.log("✅ Remove Authority Success!");

        console.log("\n🎉 All Tests Passed!");

    } catch (e: any) {
        console.error("❌ Error:", e);
        if (e.cause) console.error("Cause:", e.cause);
        if (e.logs) console.error("Logs:", e.logs);
        process.exit(1);
    }
})();

async function sendAndConfirm(rpc: any, payer: TransactionSigner, ixs: any[]) {
    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

    let msg = createTransactionMessage({ version: 0 });
    msg = setTransactionMessageFeePayerSigner(payer, msg);
    msg = await setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, msg);

    for (const ix of ixs) {
        msg = appendTransactionMessageInstruction(ix, msg);
    }

    const signedTx = await signTransactionMessageWithSigners(msg as any);
    const encoded = getBase64EncodedWireTransaction(signedTx);
    const sig = await rpc.sendTransaction(encoded, { encoding: "base64" }).send();
    console.log("    Sig:", sig);

    await waitForConfirmation(rpc, sig);
}

async function waitForConfirmation(rpc: any, signature: string) {
    let retries = 30;
    while (retries > 0) {
        const sr = await rpc.getSignatureStatuses([signature]).send();
        const status = sr.value[0];
        if (status && (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized")) {
            if (status.err) throw new Error(`Transaction failed: ${JSON.stringify(status.err)}`);
            return;
        }
        await new Promise(r => setTimeout(r, 1000));
        retries--;
    }
    throw new Error("Transaction timed out");
}
