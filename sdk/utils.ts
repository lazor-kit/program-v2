import * as anchor from '@coral-xyz/anchor';

export function instructionToAccountMetas(
  ix: anchor.web3.TransactionInstruction,
  payer: anchor.web3.PublicKey
): anchor.web3.AccountMeta[] {
  return ix.keys.map((k) => ({
    pubkey: k.pubkey,
    isWritable: k.isWritable,
    isSigner: k.pubkey.equals(payer),
  }));
}
