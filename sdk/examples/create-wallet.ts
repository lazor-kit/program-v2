import {
    createSolanaRpc,
    createKeyPairSignerFromBytes,
    getAddressEncoder,
    generateKeyPairSigner,
    pipe,
    createTransactionMessage,
    setTransactionMessageFeePayerSigner,
    setTransactionMessageLifetimeUsingBlockhash,
    appendTransactionMessageInstruction,
    signTransactionMessageWithSigners,
    sendAndConfirmTransactionFactory,
    createSolanaRpcSubscriptions
} from '@solana/kit';
import {
    createWalletInstruction,
    findConfigPDA,
    findVaultPDA,
    encodeEd25519Authority,
    AuthorityType,
    generateWalletId,
    LAZORKIT_PROGRAM_ID
} from '../src/index.js';

/**
 * Example: Create a LazorKit wallet using Solana Kit
 */
async function main() {
    // Initialize Solana Kit client
    // Use localhost for development
    const rpc = createSolanaRpc('http://127.0.0.1:8899');
    const rpcSubscriptions = createSolanaRpcSubscriptions('ws://127.0.0.1:8900');
    const sendAndConfirmTransaction = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });

    // Generate or load a keypair (using random for example)
    const owner = await generateKeyPairSigner();
    console.log('Owner:', owner.address);

    // Request airdrop if on devnet/localhost (this might fail on public devnet due to limits)
    try {
        console.log('Requesting airdrop...');
        // Cast to any for Lamports nominal type
        await rpc.requestAirdrop(owner.address, 1_000_000_000n as any).send();
        // Wait for airdrop confirmation in a real scenario
        await new Promise(r => setTimeout(r, 2000));
    } catch (e) {
        console.warn('Airdrop failed (expected if non-local):', e);
    }

    // Generate wallet ID
    const walletId = generateWalletId();
    console.log('Wallet ID:', Buffer.from(walletId).toString('hex'));

    // Find PDAs
    const configPDA = await findConfigPDA(walletId);
    const vaultPDA = await findVaultPDA(configPDA.address);

    console.log('Config PDA:', configPDA.address);
    console.log('Vault PDA:', vaultPDA.address);

    // Encode owner authority
    const addressEncoder = getAddressEncoder();
    const ownerBytes = addressEncoder.encode(owner.address);
    // Copy to Uint8Array for mutability
    const ownerAuthority = encodeEd25519Authority(new Uint8Array(ownerBytes));

    // Create instruction
    const instruction = createWalletInstruction({
        config: configPDA.address,
        payer: owner.address,
        vault: vaultPDA.address,
        systemProgram: '11111111111111111111111111111111' as any, // System program
        id: walletId,
        bump: configPDA.bump,
        walletBump: vaultPDA.bump,
        ownerAuthorityType: AuthorityType.Ed25519,
        ownerAuthorityData: ownerAuthority,
        programId: LAZORKIT_PROGRAM_ID
    });

    // Send transaction
    console.log('Creating wallet...');

    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

    const transactionMessage = pipe(
        createTransactionMessage({ version: 0 }),
        m => setTransactionMessageFeePayerSigner(owner, m),
        m => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, m),
        m => appendTransactionMessageInstruction(instruction, m)
    );

    const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);

    const signature = await sendAndConfirmTransaction(signedTransaction as any, { commitment: 'confirmed' });

    console.log('✅ Wallet created!');
    console.log('   Signature:', signature);
    console.log('   Config:', configPDA.address);
    console.log('   Vault:', vaultPDA.address);
}

main().catch(console.error);
