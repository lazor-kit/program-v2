
import {
    fetchEncodedAccount,
    fetchJsonParsedAccount,
    Address,
    Rpc,
    GetAccountInfoApi,
    assertAccountExists
} from "@solana/kit";
import {
    authorityAccountHeaderCodec,
    sessionAccountCodec,
    walletAccountCodec,
    AuthorityAccountHeader,
    SessionAccount,
    WalletAccount
} from "./types";

export async function fetchWalletAccount(
    rpc: Rpc<GetAccountInfoApi>,
    address: Address
): Promise<WalletAccount> {
    const account = await fetchEncodedAccount(rpc, address);
    assertAccountExists(account);
    return walletAccountCodec.decode(account.data);
}

export async function fetchAuthorityAccount(
    rpc: Rpc<GetAccountInfoApi>,
    address: Address
): Promise<AuthorityAccountHeader> {
    const account = await fetchEncodedAccount(rpc, address);
    assertAccountExists(account);
    return authorityAccountHeaderCodec.decode(account.data);
}

export async function fetchSessionAccount(
    rpc: Rpc<GetAccountInfoApi>,
    address: Address
): Promise<SessionAccount> {
    const account = await fetchEncodedAccount(rpc, address);
    assertAccountExists(account);
    return sessionAccountCodec.decode(account.data);
}

// Wallet account has no specific data structure defined in IDL other than being a wallet?
// Actually create_wallet says "Wallet PDA".
// The code in `create_wallet.rs` initializes it?
// Let's check `create_wallet.rs` again.
// It writes `WalletAccount`.
// struct WalletAccount { discriminator, bump, version, _padding }
// I should define WalletAccount codec in types.ts and fetcher here.

