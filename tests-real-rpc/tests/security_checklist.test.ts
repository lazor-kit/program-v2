import { describe, it, expect, beforeAll } from "vitest";
import {
  type Address,
  generateKeyPairSigner,
  getAddressEncoder,
  type TransactionSigner,
} from "@solana/kit";
import {
  setupTest,
  processInstruction,
  tryProcessInstruction,
  type TestContext,
} from "./common";
import { findWalletPda, findVaultPda, findAuthorityPda, findSessionPda } from "@lazorkit/codama-client/src";

function getRandomSeed() {
  return new Uint8Array(32).map(() => Math.floor(Math.random() * 256));
}

describe("Security Checklist Gaps", () => {
  let context: TestContext;
  let client: any;
  let walletPda: Address;
  let vaultPda: Address;
  let owner: TransactionSigner;
  let ownerAuthPda: Address;

  beforeAll(async () => {
    ({ context, client } = await setupTest());

    const userSeed = getRandomSeed();
    [walletPda] = await findWalletPda(userSeed);
    [vaultPda] = await findVaultPda(walletPda);
    owner = await generateKeyPairSigner();
    const ownerBytes = Uint8Array.from(getAddressEncoder().encode(owner.address));
    let authBump;
    [ownerAuthPda, authBump] = await findAuthorityPda(walletPda, ownerBytes);

    await processInstruction(
      context,
      client.createWallet({
        config: context.configPda,
        treasuryShard: context.treasuryShard,
        payer: context.payer,
        wallet: walletPda,
        vault: vaultPda,
        authority: ownerAuthPda,
        userSeed,
        authType: 0,
        authBump,
        authPubkey: ownerBytes,
        credentialHash: new Uint8Array(32),
      }),
    );
  }, 180_000);

  it("CreateSession rejects System Program spoofing", async () => {
    const sessionKey = await generateKeyPairSigner();
    const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
    const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

    const ix = client.createSession({
      config: context.configPda,
      treasuryShard: context.treasuryShard,
      payer: context.payer,
      wallet: walletPda,
      adminAuthority: ownerAuthPda,
      session: sessionPda,
      sessionKey: sessionKeyBytes,
      expiresAt: 999999999n,
      authorizerSigner: owner,
    });

    // SystemProgram is at index 4 in LazorClient.createSession()
    const spoofedSystemProgram = (await generateKeyPairSigner()).address;
    ix.accounts = (ix.accounts || []).map((a: any, i: number) =>
      i === 4 ? { ...a, address: spoofedSystemProgram } : a,
    );

    const result = await tryProcessInstruction(context, ix, [owner]);
    expect(result.result).toMatch(/IncorrectProgramId|simulation failed/i);
  });

  it("CloseSession: protocol admin cannot close an active session without wallet auth", async () => {
    const sessionKey = await generateKeyPairSigner();
    const sessionKeyBytes = Uint8Array.from(getAddressEncoder().encode(sessionKey.address));
    const [sessionPda] = await findSessionPda(walletPda, sessionKey.address);

    // Create a session far in the future => active
    await processInstruction(
      context,
      client.createSession({
        config: context.configPda,
        treasuryShard: context.treasuryShard,
        payer: context.payer,
        wallet: walletPda,
        adminAuthority: ownerAuthPda,
        session: sessionPda,
        sessionKey: sessionKeyBytes,
        expiresAt: BigInt(2 ** 62),
        authorizerSigner: owner,
      }),
      [owner],
    );

    // Call CloseSession with payer == config.admin (setupTest uses payer as admin),
    // but do NOT provide wallet authorizer accounts. Should be rejected unless expired.
    const closeIx = client.closeSession({
      payer: context.payer,
      wallet: walletPda,
      session: sessionPda,
      config: context.configPda,
    });

    const result = await tryProcessInstruction(context, closeIx, [context.payer]);
    expect(result.result).toMatch(/PermissionDenied|0xbba|3002|simulation failed/i);
  });
});

