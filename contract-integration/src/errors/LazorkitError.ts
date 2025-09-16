/**
 * Custom error types for LazorKit operations
 */

export class LazorkitError extends Error {
  constructor(
    message: string,
    public readonly code: string,
    public readonly context?: Record<string, any>
  ) {
    super(message);
    this.name = 'LazorkitError';
  }
}

export class WalletNotFoundError extends LazorkitError {
  constructor(passkeyPublicKey: number[]) {
    super(
      `Smart wallet not found for passkey: ${passkeyPublicKey.join(',')}`,
      'WALLET_NOT_FOUND',
      { passkeyPublicKey }
    );
  }
}

export class InvalidSignatureError extends LazorkitError {
  constructor(reason: string) {
    super(`Invalid signature: ${reason}`, 'INVALID_SIGNATURE', { reason });
  }
}

export class TransactionFailedError extends LazorkitError {
  constructor(transactionId: string, reason: string) {
    super(
      `Transaction failed: ${reason}`,
      'TRANSACTION_FAILED',
      { transactionId, reason }
    );
  }
}

export class InsufficientFundsError extends LazorkitError {
  constructor(required: string, available: string) {
    super(
      `Insufficient funds: required ${required}, available ${available}`,
      'INSUFFICIENT_FUNDS',
      { required, available }
    );
  }
}

export class PolicyError extends LazorkitError {
  constructor(message: string, policyType?: string) {
    super(`Policy error: ${message}`, 'POLICY_ERROR', { policyType });
  }
}

export class ValidationError extends LazorkitError {
  constructor(field: string, value: any, reason: string) {
    super(
      `Validation error for ${field}: ${reason}`,
      'VALIDATION_ERROR',
      { field, value, reason }
    );
  }
}
