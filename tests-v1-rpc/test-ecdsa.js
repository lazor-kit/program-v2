const ECDSA = require('ecdsa-secp256r1');

async function main() {
    const key = await ECDSA.generateKey();
    const pub = key.toCompressedPublicKey();
    console.log("pub base64 len:", pub.length, pub);
    console.log("pub buf len:", Buffer.from(pub, 'base64').length);

    const msg = Buffer.alloc(32, 1);
    const sig = await key.sign(msg);
    console.log("sig base64:", sig);
    console.log("sig buffer len:", Buffer.from(sig, 'base64').length);
}

main().catch(console.error);
