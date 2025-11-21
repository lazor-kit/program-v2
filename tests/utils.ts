import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from '@solana/spl-token';
import { Connection, Keypair, PublicKey, Signer } from '@solana/web3.js';
import { sha256 } from 'js-sha256';
import { getRandomBytes } from '../contract-integration';

export const fundAccountSOL = async (
  connection: Connection,
  publicKey: PublicKey,
  amount: number
) => {
  let fundSig = await connection.requestAirdrop(publicKey, amount);

  return getTxDetails(connection, fundSig);
};

export const getTxDetails = async (connection: Connection, sig) => {
  const latestBlockHash = await connection.getLatestBlockhash('processed');

  await connection.confirmTransaction(
    {
      blockhash: latestBlockHash.blockhash,
      lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
      signature: sig,
    },
    'confirmed'
  );

  return await connection.getTransaction(sig, {
    maxSupportedTransactionVersion: 0,
    commitment: 'confirmed',
  });
};

export const createNewMint = async (
  connection: Connection,
  creator: Signer,
  decimals: number,
  keypair?: Keypair
): Promise<PublicKey> => {
  const tokenMint = await createMint(
    connection,
    creator, // payer
    creator.publicKey, // mintAuthority
    creator.publicKey, // freezeAuthority
    decimals, // decimals,
    keypair
  );
  return tokenMint;
};

export const mintTokenTo = async (
  connection: Connection,
  tokenMint: PublicKey,
  mintAuthority: Signer,
  payer: Signer,
  to: PublicKey,
  amount: number
): Promise<PublicKey> => {
  const userTokenAccount = await getOrCreateAssociatedTokenAccount(
    connection,
    payer,
    tokenMint,
    to,
    true
  );

  await mintTo(
    connection,
    payer,
    tokenMint,
    userTokenAccount.address,
    mintAuthority,
    amount
  );

  return userTokenAccount.address;
};

export async function buildFakeMessagePasskey(data: Buffer<ArrayBufferLike>) {
  const clientDataJsonRaw = Buffer.from(
    new Uint8Array(
      new TextEncoder().encode(
        JSON.stringify({
          type: 'webauthn.get',
          challenge: bytesToBase64UrlNoPad(asUint8Array(data)),
          origin: 'https://example.com',
        })
      ).buffer
    )
  );

  // random authenticator data 37 bytes
  const authenticatorDataRaw = Buffer.from(getRandomBytes(36));

  const message = Buffer.concat([
    authenticatorDataRaw,
    Buffer.from(sha256.arrayBuffer(clientDataJsonRaw)),
  ]);

  return {
    message,
    clientDataJsonRaw64: clientDataJsonRaw.toString('base64'),
    authenticatorDataRaw64: authenticatorDataRaw.toString('base64'),
  };
}

function asUint8Array(
  input: Buffer | ArrayBuffer | ArrayBufferView | Uint8Array
): Uint8Array {
  // Node Buffer?
  // @ts-ignore
  if (
    typeof Buffer !== 'undefined' &&
    typeof (Buffer as any).isBuffer === 'function' &&
    // @ts-ignore
    (Buffer as any).isBuffer(input)
  ) {
    return new Uint8Array(input as any);
  }
  // Đã là Uint8Array
  if (input instanceof Uint8Array) return input;
  // TypedArray/DataView
  if (ArrayBuffer.isView(input)) {
    const v = input as ArrayBufferView;
    return new Uint8Array(v.buffer, v.byteOffset, v.byteLength);
  }
  // ArrayBuffer thuần
  if (input instanceof ArrayBuffer) return new Uint8Array(input);
  throw new TypeError('Unsupported byte input');
}

function bytesToBase64UrlNoPad(u8: Uint8Array): string {
  // @ts-ignore
  if (typeof Buffer !== 'undefined') {
    return Buffer.from(u8)
      .toString('base64')
      .replace(/\+/g, '-')
      .replace(/\//g, '_')
      .replace(/=+$/g, '');
  }
  // Browser fallback
  let bin = '';
  for (let i = 0; i < u8.length; i++) bin += String.fromCharCode(u8[i]);
  return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}
