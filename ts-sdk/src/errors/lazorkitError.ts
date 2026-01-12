/**
 * Error codes for Lazorkit SDK operations
 */
export enum LazorkitErrorCode {
  /** Invalid wallet ID */
  InvalidWalletId = 'INVALID_WALLET_ID',
  /** Authority not found */
  AuthorityNotFound = 'AUTHORITY_NOT_FOUND',
  /** Invalid authority type */
  InvalidAuthorityType = 'INVALID_AUTHORITY_TYPE',
  /** Invalid role permission */
  InvalidRolePermission = 'INVALID_ROLE_PERMISSION',
  /** Odometer mismatch */
  OdometerMismatch = 'ODOMETER_MISMATCH',
  /** Signature reused */
  SignatureReused = 'SIGNATURE_REUSED',
  /** Plugin not found */
  PluginNotFound = 'PLUGIN_NOT_FOUND',
  /** Transaction failed */
  TransactionFailed = 'TRANSACTION_FAILED',
  /** Invalid account data */
  InvalidAccountData = 'INVALID_ACCOUNT_DATA',
  /** Invalid discriminator */
  InvalidDiscriminator = 'INVALID_DISCRIMINATOR',
  /** PDA derivation failed */
  PdaDerivationFailed = 'PDA_DERIVATION_FAILED',
  /** Serialization error */
  SerializationError = 'SERIALIZATION_ERROR',
  /** RPC error */
  RpcError = 'RPC_ERROR',
  /** Invalid instruction data */
  InvalidInstructionData = 'INVALID_INSTRUCTION_DATA',
  /** Session expired */
  SessionExpired = 'SESSION_EXPIRED',
  /** Permission denied */
  PermissionDenied = 'PERMISSION_DENIED',
}

/**
 * Custom error class for Lazorkit SDK
 */
export class LazorkitError extends Error {
  constructor(
    public code: LazorkitErrorCode,
    message: string,
    public cause?: Error
  ) {
    super(message);
    this.name = 'LazorkitError';
    
    // Maintains proper stack trace for where our error was thrown (only available on V8)
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, LazorkitError);
    }
  }

  /**
   * Create an error from an RPC error
   */
  static fromRpcError(error: unknown): LazorkitError {
    if (error instanceof Error) {
      return new LazorkitError(
        LazorkitErrorCode.RpcError,
        `RPC error: ${error.message}`,
        error
      );
    }
    return new LazorkitError(
      LazorkitErrorCode.RpcError,
      `Unknown RPC error: ${String(error)}`
    );
  }

  /**
   * Create an error from a transaction failure
   */
  static fromTransactionFailure(error: unknown, logs?: string[]): LazorkitError {
    const message = logs && logs.length > 0
      ? `Transaction failed: ${String(error)}\nLogs:\n${logs.join('\n')}`
      : `Transaction failed: ${String(error)}`;
    
    return new LazorkitError(
      LazorkitErrorCode.TransactionFailed,
      message,
      error instanceof Error ? error : undefined
    );
  }
}
