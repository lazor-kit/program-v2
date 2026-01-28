
import {
    generateKeyPairSigner,
    AccountRole
} from "@solana/kit";
import {
    getCreateWalletInstruction,
} from "./src/generated";
import { Address } from "@solana/addresses";

(async () => {
    console.log("AccountRole Enum:");
    console.log(`  READONLY: ${AccountRole.READONLY}`);
    console.log(`  WRITABLE: ${AccountRole.WRITABLE}`);
    console.log(`  READONLY_SIGNER: ${AccountRole.READONLY_SIGNER}`);
    console.log(`  WRITABLE_SIGNER: ${AccountRole.WRITABLE_SIGNER}`);

    const payer = await generateKeyPairSigner();
    const wallet = "11111111111111111111111111111111" as Address;
    const vault = "11111111111111111111111111111111" as Address;
    const authority = "11111111111111111111111111111111" as Address;
    const userSeed = new Uint8Array(32);

    const ix = getCreateWalletInstruction({
        payer,
        wallet,
        vault,
        authority,
        userSeed,
        authType: 0,
        authBump: 255,
        padding: new Uint8Array(6),
        authPubkey: new Uint8Array(32)
    });

    console.log("\nInstruction Accounts:");
    ix.accounts.forEach((acc, i) => {
        console.log(`Account ${i}:`);
        console.log(`  Address: ${acc.address}`);
        console.log(`  Role: ${acc.role}`);
        if ('signer' in acc) {
            const s = (acc as any).signer;
            console.log(`  Signer Keys: ${Object.keys(s)}`);
        }
    });
})();
