import { createSolanaRpc, createSolanaRpcSubscriptions } from "@solana/kit";
import { LazorClient } from "../@lazorkit/codama-client/src";
import dotenv from "dotenv";

dotenv.config();

const RPC_URL = process.env.RPC_URL || "http://127.0.0.1:8899";
const WS_URL = process.env.WS_URL || "ws://127.0.0.1:8900";

export const rpc = createSolanaRpc(RPC_URL);
export const rpcSubscriptions = createSolanaRpcSubscriptions(WS_URL);

export const client = new LazorClient(rpc as any);

export { RPC_URL };
