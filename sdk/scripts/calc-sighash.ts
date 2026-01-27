import { sha256 } from "js-sha256";

const instructions = [
    "global:CreateWallet",
    "global:AddAuthority",
    "global:RemoveAuthority",
    "global:TransferOwnership",
    "global:Execute",
    "global:CreateSession"
];

instructions.forEach(ix => {
    const hash = sha256.digest(ix);
    const sighash = hash.slice(0, 8);
    console.log(`${ix}: [${sighash.join(", ")}]`);
});
