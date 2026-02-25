/**
 * High-level Execute instruction builder.
 * 
 * Takes standard Solana instructions, deduplicates accounts,
 * maps them to compact format, and returns a single Execute instruction.
 */

import {
    type Address,
    type Instruction,
    type TransactionSigner,
    AccountRole,
} from "@solana/kit";
import {
    LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
    EXECUTE_DISCRIMINATOR,
} from "../generated";
import { type CompactInstruction, packCompactInstructions } from "./packing";

export interface ExecuteInstructionBuilderParams {
    /** Transaction fee payer */
    payer: TransactionSigner;
    /** Wallet PDA address */
    wallet: Address;
    /** Authority or Session PDA address */
    authority: Address;
    /** Vault PDA address */
    vault: Address;
    /** Inner instructions to execute (e.g. SPL Token transfers) */
    innerInstructions: Instruction[];
    /** Required for Secp256r1 authentication */
    sysvarInstructions?: Address;
    /** Ed25519 signer */
    authorizerSigner?: TransactionSigner;
    /** Secp256r1 signature bytes */
    signature?: Uint8Array;
}

/**
 * Builds a complex Execute instruction from standard Solana instructions.
 * 
 * This function:
 * 1. Extracts all accounts from inner instructions
 * 2. Deduplicates and merges account roles (promoting to highest privilege)
 * 3. Maps inner instructions to compact format
 * 4. Returns a standard LazorKit Execute instruction
 */
export function buildExecuteInstruction(params: ExecuteInstructionBuilderParams): Instruction {
    const { payer, wallet, authority, vault, innerInstructions, sysvarInstructions, authorizerSigner, signature } = params;

    // Base accounts always present in Execute
    const baseAccounts: Address[] = [
        payer.address,
        wallet,
        authority,
        vault
    ];

    const accountMap = new Map<Address, number>();
    baseAccounts.forEach((addr, idx) => accountMap.set(addr, idx));

    const extraAccounts: Address[] = [];
    const accountRoles = new Map<Address, AccountRole>();
    accountRoles.set(payer.address, AccountRole.WRITABLE_SIGNER);
    accountRoles.set(wallet, AccountRole.READONLY);
    accountRoles.set(authority, AccountRole.WRITABLE);
    accountRoles.set(vault, AccountRole.WRITABLE);

    const compactIxs: CompactInstruction[] = [];

    for (const ix of innerInstructions) {
        // Ensure program ID is in the account list
        if (!accountMap.has(ix.programAddress)) {
            accountMap.set(ix.programAddress, baseAccounts.length + extraAccounts.length);
            extraAccounts.push(ix.programAddress);
            accountRoles.set(ix.programAddress, AccountRole.READONLY);
        }
        const programIdIndex = accountMap.get(ix.programAddress)!;

        // Map all instruction accounts
        const accountIndexes: number[] = [];
        const accountsToMap = ix.accounts || [];
        for (const acc of accountsToMap) {
            if (!accountMap.has(acc.address)) {
                accountMap.set(acc.address, baseAccounts.length + extraAccounts.length);
                extraAccounts.push(acc.address);
            }
            accountIndexes.push(accountMap.get(acc.address)!);

            // Promote account role to highest privilege
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

    // Build the accounts list
    const accounts: any[] = [
        { address: payer.address, role: AccountRole.WRITABLE_SIGNER, signer: payer },
        { address: wallet, role: AccountRole.READONLY },
        { address: authority, role: AccountRole.WRITABLE },
        { address: vault, role: AccountRole.WRITABLE },
        ...extraAccounts.map(addr => ({
            address: addr,
            role: accountRoles.get(addr)!,
        })),
    ];

    if (sysvarInstructions) {
        accounts.push({ address: sysvarInstructions, role: AccountRole.READONLY });
    }
    if (authorizerSigner) {
        accounts.push({ address: authorizerSigner.address, role: AccountRole.READONLY_SIGNER, signer: authorizerSigner });
    }

    // Build instruction data: [discriminator(1)] [packed_instructions] [signature?]
    const finalData = new Uint8Array(1 + packedInstructions.length + (signature?.length || 0));
    finalData[0] = EXECUTE_DISCRIMINATOR;
    finalData.set(packedInstructions, 1);
    if (signature) {
        finalData.set(signature, 1 + packedInstructions.length);
    }

    return {
        programAddress: LAZORKIT_PROGRAM_PROGRAM_ADDRESS,
        accounts,
        data: finalData,
    };
}
