
import { describe, it, expect, beforeAll } from "vitest";
import {
    Address,
    AccountRole,
} from "@solana/kit";
import { start } from "solana-bankrun";
import { PublicKey, Keypair } from "@solana/web3.js";
import { LazorClient, findWalletPda, findVaultPda, findAuthorityPda } from "../src";
import * as fs from "fs";
import * as path from "path";

const PROGRAM_ID_STR = "Btg4mLUdMd3ov8PBtmuuFMAimLAdXyew9XmsGtuY9VcP";
const PROGRAM_ID = new PublicKey(PROGRAM_ID_STR);
const PROGRAM_SO_PATH = path.join(__dirname, "../../../target/deploy/lazorkit_program.so");

describe("SDK Integration", () => {
    let context: any;

    beforeAll(async () => {
        console.log("Starting bankrun...");
        try {
            context = await start(
                [{ name: "lazorkit_program", programId: PROGRAM_ID }],
                []
            );
            console.log("Bankrun started!");
        } catch (e) {
            console.error("Start failed:", e);
            throw e;
        }
    }, 60000);

    it("Simple Check", async () => {
        expect(context).toBeDefined();
    });
});
