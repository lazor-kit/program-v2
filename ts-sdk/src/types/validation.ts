import { assertIsAddress, isAddress } from '@solana/kit';
import { AuthorityType } from './authority';
import { RolePermission } from './permission';
import { Discriminator } from './wallet';
import { LazorkitError, LazorkitErrorCode } from '../errors';

/**
 * Validate authority type
 */
export function isValidAuthorityType(value: number): value is AuthorityType {
  return Object.values(AuthorityType).includes(value as AuthorityType);
}

/**
 * Assert authority type is valid
 */
export function assertIsAuthorityType(value: number): asserts value is AuthorityType {
  if (!isValidAuthorityType(value)) {
    throw new LazorkitError(
      LazorkitErrorCode.InvalidAuthorityType,
      `Invalid authority type: ${value}. Must be between ${AuthorityType.None} and ${AuthorityType.ProgramExecSession}`
    );
  }
}

/**
 * Validate role permission
 */
export function isValidRolePermission(value: number): value is RolePermission {
  return Object.values(RolePermission).includes(value as RolePermission);
}

/**
 * Assert role permission is valid
 */
export function assertIsRolePermission(value: number): asserts value is RolePermission {
  if (!isValidRolePermission(value)) {
    throw new LazorkitError(
      LazorkitErrorCode.InvalidRolePermission,
      `Invalid role permission: ${value}. Must be between ${RolePermission.All} and ${RolePermission.ExecuteOnly}`
    );
  }
}

/**
 * Validate discriminator
 */
export function isValidDiscriminator(value: number): value is Discriminator {
  return Object.values(Discriminator).includes(value as Discriminator);
}

/**
 * Assert discriminator is valid
 */
export function assertIsDiscriminator(value: number): asserts value is Discriminator {
  if (!isValidDiscriminator(value)) {
    throw new LazorkitError(
      LazorkitErrorCode.InvalidDiscriminator,
      `Invalid discriminator: ${value}. Must be ${Discriminator.Uninitialized} or ${Discriminator.WalletAccount}`
    );
  }
}

/**
 * Validate wallet ID (must be 32 bytes)
 */
export function isValidWalletId(walletId: Uint8Array): boolean {
  return walletId.length === 32;
}

/**
 * Assert wallet ID is valid
 */
export function assertIsWalletId(walletId: Uint8Array): void {
  if (!isValidWalletId(walletId)) {
    throw new LazorkitError(
      LazorkitErrorCode.InvalidWalletId,
      `Invalid wallet ID: must be 32 bytes, got ${walletId.length} bytes`
    );
  }
}

/**
 * Validate address string
 */
export function validateAddress(address: string): boolean {
  return isAddress(address);
}

/**
 * Assert address is valid
 */
export function assertValidAddress(address: string): asserts address is import('@solana/kit').Address {
  assertIsAddress(address);
}

/**
 * Re-export address validation from @solana/kit
 */
export { assertIsAddress, isAddress } from '@solana/kit';
