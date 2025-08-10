import { AnchorError } from "@coral-xyz/anchor";

export class SDKError extends Error {
  constructor(
    message: string,
    readonly cause?: unknown,
    readonly logs?: string[]
  ) {
    super(message);
    this.name = "SDKError";
  }
}

export function decodeAnchorError(e: any): SDKError {
  if (e instanceof SDKError) return e;
  const logs: string[] | undefined =
    e?.logs || e?.transactionMessage || undefined;
  // AnchorError handling (works with provider.simulate or sendAndConfirm
  if (e instanceof AnchorError) {
    const code = e.error.errorCode.number;
    const msg = e.error.errorMessage || e.message;
    return new SDKError(`AnchorError ${code}: ${msg}`, e, logs);
  }
  // Fallback
  const message =
    typeof e?.message === "string" ? e.message : "Unknown SDK error";
  return new SDKError(message, e, logs);
}
