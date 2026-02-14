
import {
    Rpc,
    TransactionSigner,
    Address,
    GetAccountInfoApi,
    Instruction,
} from "@solana/kit";
import {
    fetchWalletAccount,
    fetchAuthorityAccount,
    fetchSessionAccount
} from "./accounts";
import {
    getCreateWalletInstruction,
    getAddAuthorityInstruction,
    getExecuteInstruction,
    getCreateSessionInstruction,
    getRemoveAuthorityInstruction,
    getTransferOwnershipInstruction
} from "./instructions";
import {
    findWalletPda,
    findVaultPda,
    findAuthorityPda,
    findSessionPda
} from "./pdas";
import { LAZOR_KIT_PROGRAM_ID } from "./constants";
import { buildExecuteInstruction } from "./utils/transaction";

export class LazorClient {
    constructor(
        private readonly rpc: Rpc<GetAccountInfoApi>,
        private readonly programId: Address = LAZOR_KIT_PROGRAM_ID
    ) { }

    // --- PDAs ---

    async findWalletPda(userSeed: Uint8Array) {
        return findWalletPda(userSeed);
    }

    async findVaultPda(wallet: Address) {
        return findVaultPda(wallet);
    }

    async findAuthorityPda(wallet: Address, idHash: Uint8Array) {
        return findAuthorityPda(wallet, idHash);
    }

    async findSessionPda(wallet: Address, sessionKey: Address) {
        return findSessionPda(wallet, sessionKey);
    }

    // --- Account Fetching ---

    async getWallet(address: Address) {
        return fetchWalletAccount(this.rpc, address);
    }

    async getAuthority(address: Address) {
        return fetchAuthorityAccount(this.rpc, address);
    }

    async getSession(address: Address) {
        return fetchSessionAccount(this.rpc, address);
    }

    // --- Instructions ---

    createWallet(input: Parameters<typeof getCreateWalletInstruction>[0]): Instruction {
        return getCreateWalletInstruction(input);
    }

    addAuthority(input: Parameters<typeof getAddAuthorityInstruction>[0]): Instruction {
        return getAddAuthorityInstruction(input);
    }

    removeAuthority(input: Parameters<typeof getRemoveAuthorityInstruction>[0]): Instruction {
        return getRemoveAuthorityInstruction(input);
    }

    transferOwnership(input: Parameters<typeof getTransferOwnershipInstruction>[0]): Instruction {
        return getTransferOwnershipInstruction(input);
    }

    createSession(input: Parameters<typeof getCreateSessionInstruction>[0]): Instruction {
        return getCreateSessionInstruction(input);
    }

    execute(input: Parameters<typeof getExecuteInstruction>[0]): Instruction {
        return getExecuteInstruction(input);
    }

    buildExecute(input: Parameters<typeof buildExecuteInstruction>[0]): Instruction {
        return buildExecuteInstruction(input);
    }
}
