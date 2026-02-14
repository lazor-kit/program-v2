
import {
    Address,
    Instruction,
    TransactionSigner,
    AccountRole
} from "@solana/kit";
import { LAZOR_KIT_PROGRAM_ID } from "../constants";
import { CompactInstruction, packCompactInstructions } from "./packing";

/**
 * Helper to build a complex Execute instruction from standard Solana instructions.
 * 
 * This tool:
 * 1. Takes the wallet's vault and authority.
 * 2. Takes a list of "inner" instructions (e.g. SPL Token transfer).
 * 3. Extracts all accounts from inner instructions and deduplicates them.
 * 4. Maps inner instructions to the compact format based on the unified account list.
 * 5. Returns a standard Execute instruction with all required accounts.
 */
export interface ExecuteInstructionBuilderParams {
    payer: TransactionSigner;
    wallet: Address;
    authority: Address;
    vault: Address;
    innerInstructions: Instruction[];
    sysvarInstructions?: Address;
    authorizerSigner?: TransactionSigner;
    signature?: Uint8Array;
}

export function buildExecuteInstruction(params: ExecuteInstructionBuilderParams): Instruction {
    const { payer, wallet, authority, vault, innerInstructions, sysvarInstructions, authorizerSigner, signature } = params;

    const baseAccounts: Address[] = [
        payer.address,
        wallet,
        authority,
        vault
    ];

    const innerAccountMap = new Map<Address, number>();
    baseAccounts.forEach((addr, idx) => innerAccountMap.set(addr, idx));

    const extraAccounts: Address[] = [];
    const accountRoles = new Map<Address, AccountRole>();
    accountRoles.set(payer.address, AccountRole.WRITABLE_SIGNER);
    accountRoles.set(wallet, AccountRole.READONLY);
    accountRoles.set(authority, AccountRole.WRITABLE); // Promotion
    accountRoles.set(vault, AccountRole.WRITABLE); // Promotion

    const compactIxs: CompactInstruction[] = [];

    for (const ix of innerInstructions) {
        if (!innerAccountMap.has(ix.programAddress)) {
            innerAccountMap.set(ix.programAddress, baseAccounts.length + extraAccounts.length);
            extraAccounts.push(ix.programAddress);
            accountRoles.set(ix.programAddress, AccountRole.READONLY);
        }
        const programIdIndex = innerAccountMap.get(ix.programAddress)!;

        const accountIndexes: number[] = [];
        const accountsToMap = ix.accounts || [];
        for (const acc of accountsToMap) {
            if (!innerAccountMap.has(acc.address)) {
                innerAccountMap.set(acc.address, baseAccounts.length + extraAccounts.length);
                extraAccounts.push(acc.address);
            }
            accountIndexes.push(innerAccountMap.get(acc.address)!);

            const currentRole = accountRoles.get(acc.address) || AccountRole.READONLY;
            if (acc.role > currentRole) {
                accountRoles.set(acc.address, acc.role);
            }
        }

        compactIxs.push({
            programIdIndex,
            accountIndexes,
            data: ix.data as Uint8Array,
        });
    }

    const packedInstructions = packCompactInstructions(compactIxs);

    const accounts = [
        { address: payer.address, role: AccountRole.WRITABLE_SIGNER, signer: payer },
        { address: wallet, role: AccountRole.READONLY },
        { address: authority, role: AccountRole.WRITABLE },
        { address: vault, role: AccountRole.WRITABLE },
        ...extraAccounts.map(addr => ({
            address: addr,
            role: accountRoles.get(addr)!
        }))
    ];

    if (sysvarInstructions) {
        accounts.push({ address: sysvarInstructions, role: AccountRole.READONLY });
    }
    if (authorizerSigner) {
        accounts.push({ address: authorizerSigner.address, role: AccountRole.READONLY_SIGNER, signer: authorizerSigner });
    }

    const finalData = new Uint8Array(1 + packedInstructions.length + (signature?.length || 0));
    finalData[0] = 4; // Execute discriminator
    finalData.set(packedInstructions, 1);
    if (signature) {
        finalData.set(signature, 1 + packedInstructions.length);
    }

    return {
        programAddress: LAZOR_KIT_PROGRAM_ID,
        accounts,
        data: finalData,
    };
}
