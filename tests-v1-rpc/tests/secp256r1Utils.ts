import { PublicKey, TransactionInstruction } from '@solana/web3.js';
import * as crypto from 'crypto';
// @ts-ignore
import ECDSA from 'ecdsa-secp256r1';

export const SECP256R1_PROGRAM_ID = new PublicKey("Secp256r1SigVerify1111111111111111111111111");

export interface MockSecp256r1Signer {
    privateKey: any; // ecdsa-secp256r1 key object
    publicKeyBytes: Uint8Array; // 33 byte compressed 
    credentialIdHash: Uint8Array; // 32 byte hash
}

export async function generateMockSecp256r1Signer(credentialIdHash?: Uint8Array): Promise<MockSecp256r1Signer> {
    const privateKey = await ECDSA.generateKey();
    const pubKeyBase64 = privateKey.toCompressedPublicKey();
    const compressedPubKey = new Uint8Array(Buffer.from(pubKeyBase64, 'base64'));

    const credHash = credentialIdHash || new Uint8Array(32).map(() => Math.floor(Math.random() * 256));

    return {
        privateKey,
        publicKeyBytes: compressedPubKey,
        credentialIdHash: credHash,
    };
}

export async function signWithSecp256r1(signer: MockSecp256r1Signer, message: Uint8Array): Promise<Uint8Array> {
    const signatureBase64 = await signer.privateKey.sign(Buffer.from(message));
    const rawSig = new Uint8Array(Buffer.from(signatureBase64, 'base64'));

    // Solana secp256r1 precompile STRICTLY requires low-S signatures.
    // SECP256R1 curve order n
    const SECP256R1_N = 0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551n;
    const HALF_N = SECP256R1_N / 2n;

    let rBuffer: Uint8Array;
    let sBufferLocal: Uint8Array;

    // Check if signature is DER encoded (starts with 0x30)
    if (rawSig[0] === 0x30) {
        // DER decode: 30 <len> 02 <r_len> <r_bytes> 02 <s_len> <s_bytes>
        let offset = 2; // skip 30 <len>
        if (rawSig[offset] !== 0x02) throw new Error("Invalid DER: expected 0x02 for r");
        offset++;
        const rLen = rawSig[offset]; offset++;
        const rRaw = rawSig.slice(offset, offset + rLen); offset += rLen;
        if (rawSig[offset] !== 0x02) throw new Error("Invalid DER: expected 0x02 for s");
        offset++;
        const sLen = rawSig[offset]; offset++;
        const sRaw = rawSig.slice(offset, offset + sLen);

        // Pad/trim r and s to exactly 32 bytes
        rBuffer = new Uint8Array(32);
        if (rRaw.length > 32) {
            rBuffer.set(rRaw.slice(rRaw.length - 32));
        } else {
            rBuffer.set(rRaw, 32 - rRaw.length);
        }
        sBufferLocal = new Uint8Array(32);
        if (sRaw.length > 32) {
            sBufferLocal.set(sRaw.slice(sRaw.length - 32));
        } else {
            sBufferLocal.set(sRaw, 32 - sRaw.length);
        }
    } else if (rawSig.length >= 64) {
        // Raw r||s format (64 bytes)
        rBuffer = rawSig.slice(0, 32);
        sBufferLocal = rawSig.slice(32, 64);
    } else {
        throw new Error(`Unexpected signature format: length=${rawSig.length}, first byte=0x${rawSig[0]?.toString(16)}`);
    }

    // convert s to bigint
    let sBigInt = 0n;
    for (let i = 0; i < 32; i++) {
        sBigInt = (sBigInt << 8n) + BigInt(sBufferLocal[i]);
    }

    if (sBigInt > HALF_N) {
        // Enforce low S: s = n - s
        sBigInt = SECP256R1_N - sBigInt;

        // Write low S back to sBufferLocal
        for (let i = 31; i >= 0; i--) {
            sBufferLocal[i] = Number(sBigInt & 0xffn);
            sBigInt >>= 8n;
        }
    }

    // Return 64-byte raw r||s
    const result = new Uint8Array(64);
    result.set(rBuffer, 0);
    result.set(sBufferLocal, 32);
    return result;
}

export async function createSecp256r1Instruction(signer: MockSecp256r1Signer, message: Uint8Array): Promise<TransactionInstruction> {
    const signature = await signWithSecp256r1(signer, message);

    const SIGNATURE_OFFSETS_SERIALIZED_SIZE = 14;
    const SIGNATURE_OFFSETS_START = 2; // [num_sigs(1), padding(1)]
    const DATA_START = SIGNATURE_OFFSETS_SERIALIZED_SIZE + SIGNATURE_OFFSETS_START; // 16
    const SIGNATURE_SERIALIZED_SIZE = 64;
    const COMPRESSED_PUBKEY_SERIALIZED_SIZE = 33;

    const signatureOffset = DATA_START;
    const publicKeyOffset = signatureOffset + SIGNATURE_SERIALIZED_SIZE; // 80
    const messageDataOffset = publicKeyOffset + COMPRESSED_PUBKEY_SERIALIZED_SIZE + 1; // 114 (padding included)

    const totalSize = messageDataOffset + message.length;
    const instructionData = new Uint8Array(totalSize);

    // Number of signatures + padding
    instructionData[0] = 1;
    instructionData[1] = 0;

    const offsetsView = new DataView(instructionData.buffer, instructionData.byteOffset + SIGNATURE_OFFSETS_START, 14);
    offsetsView.setUint16(0, signatureOffset, true);
    offsetsView.setUint16(2, 0xffff, true);
    offsetsView.setUint16(4, publicKeyOffset, true);
    offsetsView.setUint16(6, 0xffff, true);
    offsetsView.setUint16(8, messageDataOffset, true);
    offsetsView.setUint16(10, message.length, true);
    offsetsView.setUint16(12, 0xffff, true);

    instructionData.set(signature, signatureOffset);
    instructionData.set(signer.publicKeyBytes, publicKeyOffset);
    instructionData.set(message, messageDataOffset);

    return new TransactionInstruction({
        programId: SECP256R1_PROGRAM_ID,
        keys: [],
        data: Buffer.from(instructionData),
    });
}

export function generateAuthenticatorData(rpId: string = "example.com"): Uint8Array {
    const rpIdHash = crypto.createHash('sha256').update(rpId).digest();
    const authenticatorData = new Uint8Array(37);
    authenticatorData.set(rpIdHash, 0); // 32 bytes rpIdHash
    authenticatorData[32] = 0x01; // User Present flag
    // Counter is the last 4 bytes (0)
    return authenticatorData;
}

function bytesToBase64UrlNoPad(bytes: Uint8Array): string {
    const base64 = Buffer.from(bytes).toString("base64");
    return base64.replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
}

export function buildSecp256r1AuthPayload(
    sysvarInstructionsIndex: number,
    sysvarSlothashesIndex: number,
    authenticatorDataRaw: Uint8Array,
    slot: bigint = 0n
): Uint8Array {
    const rpIdStr = "example.com";
    const rpIdBytes = new TextEncoder().encode(rpIdStr);

    const payloadLen = 12 + rpIdBytes.length + authenticatorDataRaw.length;
    const payloadFull = new Uint8Array(payloadLen);
    const view = new DataView(payloadFull.buffer, payloadFull.byteOffset, payloadFull.byteLength);

    view.setBigUint64(0, slot, true);

    payloadFull[8] = sysvarInstructionsIndex;
    payloadFull[9] = sysvarSlothashesIndex;

    // 0x10 = webauthn.get (0x10) | https:// (0x00)
    payloadFull[10] = 0x10;

    payloadFull[11] = rpIdBytes.length;
    payloadFull.set(rpIdBytes, 12);

    const authDataOffset = 12 + rpIdBytes.length;
    payloadFull.set(authenticatorDataRaw, authDataOffset);

    return payloadFull;
}

export function getSecp256r1MessageToSign(
    discriminator: Uint8Array,
    authPayload: Uint8Array,
    signedPayload: Uint8Array,
    payer: Uint8Array,
    programId: Uint8Array,
    authenticatorDataRaw: Uint8Array,
    slotBytes: Uint8Array
): Uint8Array {
    const hasherHash = crypto.createHash("sha256");
    hasherHash.update(discriminator);
    hasherHash.update(authPayload);
    hasherHash.update(signedPayload);
    hasherHash.update(slotBytes);
    hasherHash.update(payer);
    hasherHash.update(programId);
    const challengeHash = hasherHash.digest();

    const clientDataJsonRaw = Buffer.from(
        new Uint8Array(
            new TextEncoder().encode(
                JSON.stringify({
                    type: "webauthn.get",
                    challenge: bytesToBase64UrlNoPad(new Uint8Array(challengeHash)),
                    origin: "https://example.com",
                    crossOrigin: false
                })
            ).buffer
        )
    );

    const message = Buffer.concat([
        authenticatorDataRaw,
        Buffer.from(crypto.createHash("sha256").update(clientDataJsonRaw).digest()),
    ]);

    return new Uint8Array(message);
}
