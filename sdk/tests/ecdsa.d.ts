declare module 'ecdsa-secp256r1' {
    export default class ECDSA {
        static generateKey(): ECDSA;
        toCompressedPublicKey(): string;
        sign(message: Uint8Array | string): string; // returns base64 signature ideally
    }
}
