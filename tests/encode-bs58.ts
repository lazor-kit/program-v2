import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import fs from "node:fs";

const json = JSON.parse(fs.readFileSync("./lazorkit-wallet.json", "utf-8"));
const encoded = bs58.encode(json);
console.log(encoded);