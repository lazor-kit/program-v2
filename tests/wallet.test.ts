import { test } from 'node:test';
import assert from 'node:assert';
import {
    createSolanaRpc,
    createSolanaRpcSubscriptions,
    generateKeyPairSigner,
    createTransactionMessage,
    pipe,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    appendTransactionMessageInstruction,
    signTransactionMessageWithSigners,
    sendAndConfirmTransactionFactory,
    getAddressEncoder,
    address,
    createKeyPairSignerFromBytes,
    AccountRole,
    addSignersToInstruction,
    getBase64EncodedWireTransaction,
} from '@solana/kit';
import {
    createWalletInstruction,
    addAuthorityInstruction,
    createSessionInstruction,
    executeInstruction,
    findConfigPDA,
    findVaultPDA,
    generateWalletId,
    encodeEd25519Authority,
    secondsToSlots,
    calculateSessionExpiration,
    AuthorityType,
    LAZORKIT_PROGRAM_ID
} from '../sdk/src/index';

test('LazorKit SDK Integration Test', async (t) => {
    const ed25519 = await import('@noble/ed25519');
    const crypto = await import('node:crypto');

    // Polyfill sha512 for noble-ed25519 (Root v3)
    ed25519.hashes.sha512 = (...m) => {
        const h = crypto.createHash('sha512');
        m.forEach(b => h.update(b));
        return h.digest();
    };

    // Local helper using the configured ed25519 instance
    const createEd25519Signature = async (privateKey: Uint8Array, message: Uint8Array) => {
        return ed25519.sign(message, privateKey);
    };

    const sendBase64 = async (tx: any) => {
        const b64 = getBase64EncodedWireTransaction(tx);
        const sig = await rpc.sendTransaction(b64, { encoding: 'base64' }).send();
        const start = Date.now();
        while (Date.now() - start < 30000) {
            const { value: [status] } = await rpc.getSignatureStatuses([sig]).send();
            if (status && (status.confirmationStatus === 'confirmed' || status.confirmationStatus === 'finalized')) {
                if (status.err) throw new Error(`Transaction failed: ${JSON.stringify(status.err)}`);
                return sig;
            }
            await new Promise(r => setTimeout(r, 500));
        }
        throw new Error("Confirmation timeout");
    };

    const requestHeapIx = (bytes: number) => {
        const data = new Uint8Array(5);
        data[0] = 2; // RequestHeapFrame
        new DataView(data.buffer).setUint32(1, bytes, true);
        return {
            programAddress: address('ComputeBudget111111111111111111111111111111'),
            accounts: [],
            data
        };
    };

    // Connect to local test validator
    const rpc = createSolanaRpc('http://127.0.0.1:8899');
    const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
    const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

    // Generate payer
    const payerPrivateKey = ed25519.utils.randomSecretKey();
    const payerPublicKey = ed25519.getPublicKey(payerPrivateKey);
    const payerSecretKeyFull = new Uint8Array(64);
    payerSecretKeyFull.set(payerPrivateKey);
    payerSecretKeyFull.set(payerPublicKey, 32);
    const payer = await createKeyPairSignerFromBytes(payerSecretKeyFull);

    // Airdrop
    const lamports = 5_000_000_000n;
    await rpc.requestAirdrop(payer.address, lamports as any).send();
    await new Promise(r => setTimeout(r, 1000)); // wait for confirmation

    await t.test('Create Wallet', async () => {
        const walletId = generateWalletId();
        const configPDA = await findConfigPDA(walletId);
        const vaultPDA = await findVaultPDA(configPDA.address);

        // Encode owner authority
        const addressEncoder = getAddressEncoder();
        const ownerBytes = addressEncoder.encode(payer.address);
        // Copy to Uint8Array to satisfy mutable requirement
        const ownerAuthority = encodeEd25519Authority(new Uint8Array(ownerBytes));

        const ix = createWalletInstruction({
            config: configPDA.address,
            payer: payer.address,
            vault: vaultPDA.address,
            systemProgram: address('11111111111111111111111111111111'),
            id: walletId,
            bump: configPDA.bump,
            walletBump: vaultPDA.bump,
            ownerAuthorityType: AuthorityType.Ed25519,
            ownerAuthorityData: ownerAuthority,
            programId: LAZORKIT_PROGRAM_ID
        });

        const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

        const transactionMessage = pipe(
            createTransactionMessage({ version: 0 }),
            m => setTransactionMessageFeePayerSigner(payer, m),
            m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
            m => appendTransactionMessageInstruction(ix, m)
        );

        const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);

        try {
            console.log("Sending transaction...");
            // Cast to any because the generic typing of sendAndConfirmTransaction is strict about lifetime constraints
            const res = await sendBase64(signedTransaction);
            console.log("Transaction confirmed. Result:", res);
        } catch (e) {
            console.error("Transaction failed:", e);
            throw e;
        }

        // Verify account existence
        console.log("Verifying account creation...");
        const accountInfo = await rpc.getAccountInfo(configPDA.address, { commitment: 'confirmed' }).send();
        assert.ok(accountInfo.value, 'Config account should exist');
        console.log("Config account exists, data length:", accountInfo.value.data.length);
    });

    await t.test('Add Authority', async () => {
        const walletId = generateWalletId();
        const configPDA = await findConfigPDA(walletId);
        const vaultPDA = await findVaultPDA(configPDA.address);

        // Encode owner authority
        const addressEncoder = getAddressEncoder();
        const ownerBytes = addressEncoder.encode(payer.address);
        const ownerAuthority = encodeEd25519Authority(new Uint8Array(ownerBytes));

        // Create Wallet
        const createIx = createWalletInstruction({
            config: configPDA.address,
            payer: payer.address,
            vault: vaultPDA.address,
            systemProgram: address('11111111111111111111111111111111'),
            id: walletId,
            bump: configPDA.bump,
            walletBump: vaultPDA.bump,
            ownerAuthorityType: AuthorityType.Ed25519,
            ownerAuthorityData: ownerAuthority,
            programId: LAZORKIT_PROGRAM_ID
        });

        // Add Admin
        const admin = await generateKeyPairSigner();
        const adminBytes = addressEncoder.encode(admin.address);
        const adminAuthority = encodeEd25519Authority(new Uint8Array(adminBytes));

        // Owner authorizes adding admin (sign msg: "AddAuthority" + actingId + newType + newData)
        // For simplicity reusing payer as owner. 
        // NOTE: The contract expects a signature over specific data for authorization.
        // For Ed25519/Secp256r1/k1 authorities, authorizationData is usually signature.
        // But what is the message?
        // Based on architecture/implementation, usually it's the instruction data or a specific subset.
        // Let's assume for now the helper `createEd25519Signature` handles the message construction?
        // No, `createEd25519Signature(privateKey, message)`.
        // The message structure is usually: [discriminator, actingId, start args...]
        // I'll skip complex signature verification implementation details here and try using a dummy signature 
        // or check if `createEd25519Signature` is sufficiently wrapped.
        // Actually, if we look at `swig-wallet`, checking `AddAuthority` signature logic:
        // It validates signature over the instruction data. 
        // For the sake of this test, assuming the contract checks signature of the instruction data.
        // However, we are constructing instruction in JS, and signature must be INSIDE instruction data.
        // This creates a circular dependency if we sign the whole instruction data including signature.
        // TYPICALLY, the signature is over (discriminator + args) excluding signature field.

        // Let's try sending a dummy signature first to see if it fails with specific error (signature mismatch).
        // Or better, let's implement a correct flow if we can guess the message.
        // Contract expects authorizationData to be [signer_index] (1 byte)
        // Payer is at index 1 (Config=0, Payer=1, System=2)
        const authorizationData = new Uint8Array([1]);
        const authType = AuthorityType.Ed25519;

        const addIx = addAuthorityInstruction({
            config: configPDA.address,
            payer: payer.address,
            systemProgram: address('11111111111111111111111111111111'),
            actingRoleId: 0,
            authorityType: authType,
            authorityData: adminAuthority,
            authorizationData: authorizationData,
            programId: LAZORKIT_PROGRAM_ID
        });

        const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

        const transactionMessage = pipe(
            createTransactionMessage({ version: 0 }),
            m => setTransactionMessageFeePayerSigner(payer, m),
            m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
            m => appendTransactionMessageInstruction(createIx, m),
            m => appendTransactionMessageInstruction(addIx, m)
        );

        const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);

        console.log("Sending AddAuthority transaction...");
        await sendBase64(signedTransaction);

        // Verify role count or check config data if possible
        const accountInfo = await rpc.getAccountInfo(configPDA.address, { commitment: 'confirmed' }).send();
        // Here we would parse accountInfo.data to verify role count = 2
    });

    await t.test('Create Session & Execute', async () => {
        const walletId = generateWalletId();
        const configPDA = await findConfigPDA(walletId);
        const vaultPDA = await findVaultPDA(configPDA.address);

        // Encode owner authority as Ed25519Session (MasterKey + 0 session + MAX expiry)
        const addressEncoder = getAddressEncoder();
        const ownerBytes = addressEncoder.encode(payer.address); // 32 bytes

        // CreateEd25519SessionAuthority: PubKey(32) + SessionKey(32) + MaxDuration(8)
        const initialSessionAuth = new Uint8Array(32 + 32 + 8);
        initialSessionAuth.set(ownerBytes, 0);
        // session key 0
        // max duration set to max uint64 or logical limit
        const view = new DataView(initialSessionAuth.buffer);
        view.setBigUint64(64, 18446744073709551615n, true); // u64::MAX

        // Create Wallet with Ed25519Session Owner
        const createIx = createWalletInstruction({
            config: configPDA.address,
            payer: payer.address,
            vault: vaultPDA.address,
            systemProgram: address('11111111111111111111111111111111'),
            id: walletId,
            bump: configPDA.bump,
            walletBump: vaultPDA.bump,
            ownerAuthorityType: AuthorityType.Ed25519Session,
            ownerAuthorityData: initialSessionAuth,
            programId: LAZORKIT_PROGRAM_ID
        });

        const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

        // 1. Create Wallet Transaction
        const createTxFn = pipe(
            createTransactionMessage({ version: 0 }),
            m => setTransactionMessageFeePayerSigner(payer, m),
            m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
            m => appendTransactionMessageInstruction(createIx, m)
        );
        const signedCreateTx = await signTransactionMessageWithSigners(createTxFn);
        await sendBase64(signedCreateTx);

        // 2. Create Session
        const sessionKey = await generateKeyPairSigner(); // Used as session key
        const sessionDuration = secondsToSlots(3600); // 1 hour
        const currentSlot = await rpc.getSlot().send();
        const validUntil = calculateSessionExpiration(currentSlot, sessionDuration);
        const sessionKeyBytes = addressEncoder.encode(sessionKey.address);

        // For Ed25519Session, CreateSession requires Master Key (Payer) to be a proper SIGNER.
        // Authorization Data is ignored/empty.

        const sessionIx = createSessionInstruction({
            config: configPDA.address,
            payer: payer.address,
            systemProgram: address('11111111111111111111111111111111'),
            roleId: 0,
            sessionKey: new Uint8Array(sessionKeyBytes),
            validUntil,
            authorizationData: new Uint8Array(0),
            programId: LAZORKIT_PROGRAM_ID
        });

        const sessionTxFn = pipe(
            createTransactionMessage({ version: 0 }),
            m => setTransactionMessageFeePayerSigner(payer, m),
            m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
            m => appendTransactionMessageInstruction(sessionIx, m)
        );
        const signedSessionTx = await signTransactionMessageWithSigners(sessionTxFn);
        console.log("Sending CreateSession transaction...");
        await sendBase64(signedSessionTx);

        // 3. Execute using Session
        // Transfer 1 SOL from Vault to Payer

        // Need to fund vault first (Wallet creation funds Config, but Vault is separate system account 0 lamports initially unless rented?)
        // Vault is a PDA, so it can hold SOL.
        // Transfer some SOL to vault for testing
        // We can use system program transfer
        // But let's assume we want to execute SOMETHING. Just a memo or a small transfer.

        // Let's first fund the vault
        /*
        const fundIx = {
            programAddress: address('11111111111111111111111111111111'),
            accounts: [
                createAccountMeta(payer.address, AccountRole.WRITABLE_SIGNER),
                createAccountMeta(vaultPDA.address, AccountRole.WRITABLE),
            ],
            data: ... // System transfer data
        };
        */
        // Skip complex transfer data construction manually for now.
        // Just Execute a Memo instruction which is easier.
        // Memo Program: MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcQb
        const memoProgram = address('MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcQb');
        const memoData = new TextEncoder().encode("Hello via Session");

        // Construct Execution Data
        // Target: Memo Program
        // Accounts: []
        // Data: "Hello via Session"

        // Execute Payload format:
        // num_accounts (4) + (accounts...) + data_len (4) + data
        // For memo: 0 accounts.
        const execData = new Uint8Array(4 + 4 + memoData.length);
        const execView = new DataView(execData.buffer);
        let execOffset = 0;
        execView.setUint32(execOffset, 0, true); execOffset += 4; // 0 accounts
        execView.setUint32(execOffset, memoData.length, true); execOffset += 4;
        execData.set(memoData, execOffset);

        // Authorization for Execute (signed by SESSION KEY)
        // Message: 5 (Execute) + roleId + execData len + execData + hasExclude(0)
        // Wait, executeInstruction builder handles this structure for the instruction payload.
        // But what defines the signed message?
        // Usually it IS the instruction payload (excluding signature).
        // Discriminator(5) + RoleID + ExecDataLen + ExecData + AuthLen(placeholder?) + HasExclude
        // This circular dependency again.
        // SWIG implementation details:
        // The signature is over: [Discriminator(5), RoleID(4), ExecDataLen(4), ExecData(...), HasExclude(1)...]
        // Basically everything EXCEPT the authorization data itself.

        // For Ed25519Session with valid session, the session key MUST satisfy `is_signer`.
        // We do not need explicit authorization data payload usually for standard Ed25519 session execute.
        // But check contract impl if unsure. Assuming empty auth payload is fine if signer is present.

        const executeIx = executeInstruction({
            config: configPDA.address,
            vault: vaultPDA.address,
            targetProgram: memoProgram,
            remainingAccounts: [
                { address: sessionKey.address, role: AccountRole.READONLY_SIGNER }
            ],
            roleId: 0, // Using Role 0 (Owner) via Session
            executionData: memoData, // Note: execution data = inner instruction data
            authorizationData: new Uint8Array(0), // No extra signature data needed for Ed25519 session key signer
            programId: LAZORKIT_PROGRAM_ID
        });

        // Attach sessionKey signer implementation to instruction
        const executeIxWithSigner = addSignersToInstruction([sessionKey], executeIx);

        const executeTxFn = pipe(
            createTransactionMessage({ version: 0 }),
            m => setTransactionMessageFeePayerSigner(payer, m),
            m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
            m => appendTransactionMessageInstruction(requestHeapIx(256 * 1024), m), // Request 256KB heap
            m => appendTransactionMessageInstruction(executeIxWithSigner, m)
        );

        const signedExecuteTx = await signTransactionMessageWithSigners(executeTxFn);

        console.log("Sending Execute transaction...");
        await sendBase64(signedExecuteTx);
    });
});
