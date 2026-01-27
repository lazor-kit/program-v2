import { SignatureBytes } from "@solana/keys";
import { Address } from "@solana/addresses";

export enum AuthType {
    Ed25519 = 0,
    Secp256r1 = 1,
}

export enum Role {
    Owner = 0,
    Admin = 1,
    Spender = 2,
}

// Low Level Interfaces

export interface CreateWalletParams {
    payer: Address;
    userSeed: Uint8Array; // 32 bytes
    authType: AuthType;
    // Ed25519: pubkey bytes (32)
    // Secp256r1: hash (32) + encoded pubkey (33)
    authData: Uint8Array;
}

export interface ExecuteParams {
    wallet: Address;
    authority: Address; // Signer (could be Session or Authority)
    instructions: Uint8Array; // Compact instruction bytes
}

// High Level Interfaces

export interface AddAuthorityParams {
    wallet: Address;
    adminAuthority: Address; // The existing authority approving this
    newAuthType: AuthType;
    newAuthRole: Role;
    newAuthData: Uint8Array;
}

export interface RemoveAuthorityParams {
    wallet: Address;
    adminAuthority: Address;
    targetAuthority: Address;
    refundDestination: Address;
}

export interface TransferOwnershipParams {
    wallet: Address;
    currentOwner: Address;
    newAuthType: AuthType;
    newAuthData: Uint8Array;
}

export interface CreateSessionParams {
    wallet: Address;
    authority: Address; // The authorizer
    sessionKey: Uint8Array; // 32 bytes public key of the session
    expiresAt: bigint; // u64
}
