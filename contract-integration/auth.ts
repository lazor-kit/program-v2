import * as anchor from '@coral-xyz/anchor';
import { buildSecp256r1VerifyIx } from './webauthn/secp256r1';
import { sha256 } from 'js-sha256';
import { PasskeySignature } from './types';

/**
 * Builds a Secp256r1 signature verification instruction for passkey authentication
 */
export function buildPasskeyVerificationInstruction(
  passkeySignature: PasskeySignature
): anchor.web3.TransactionInstruction {
  const authenticatorDataRaw = Buffer.from(
    passkeySignature.authenticatorDataRaw64,
    'base64'
  );
  const clientDataJsonRaw = Buffer.from(
    passkeySignature.clientDataJsonRaw64,
    'base64'
  );

  return buildSecp256r1VerifyIx(
    Buffer.concat([
      authenticatorDataRaw,
      Buffer.from(sha256.arrayBuffer(clientDataJsonRaw)),
    ]),
    passkeySignature.passkeyPublicKey,
    Buffer.from(passkeySignature.signature64, 'base64')
  );
}

/**
 * Converts passkey signature data to the format expected by smart contract instructions
 */
export function convertPasskeySignatureToInstructionArgs(
  passkeySignature: PasskeySignature
): {
  passkeyPublicKey: number[];
  signature: number[];
  clientDataJsonRaw: Buffer;
  authenticatorDataRaw: Buffer;
} {
  return {
    passkeyPublicKey: passkeySignature.passkeyPublicKey,
    signature: Array.from(Buffer.from(passkeySignature.signature64, 'base64')),
    clientDataJsonRaw: Buffer.from(
      passkeySignature.clientDataJsonRaw64,
      'base64'
    ),
    authenticatorDataRaw: Buffer.from(
      passkeySignature.authenticatorDataRaw64,
      'base64'
    ),
  };
}
