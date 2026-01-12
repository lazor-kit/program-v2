/**
 * Authority types supported by Lazorkit V2
 * 
 * These match the AuthorityType enum in the Rust state crate.
 */
export enum AuthorityType {
  /** No authority (invalid state) */
  None = 0,
  /** Standard Ed25519 authority */
  Ed25519 = 1,
  /** Session-based Ed25519 authority */
  Ed25519Session = 2,
  /** Standard Secp256k1 authority */
  Secp256k1 = 3,
  /** Session-based Secp256k1 authority */
  Secp256k1Session = 4,
  /** Standard Secp256r1 authority (for passkeys) */
  Secp256r1 = 5,
  /** Session-based Secp256r1 authority */
  Secp256r1Session = 6,
  /** Program execution authority */
  ProgramExec = 7,
  /** Session-based Program execution authority */
  ProgramExecSession = 8,
}

/**
 * Check if an authority type supports session-based authentication
 */
export function isSessionBased(authorityType: AuthorityType): boolean {
  return [
    AuthorityType.Ed25519Session,
    AuthorityType.Secp256k1Session,
    AuthorityType.Secp256r1Session,
    AuthorityType.ProgramExecSession,
  ].includes(authorityType);
}

/**
 * Get the standard (non-session) version of an authority type
 */
export function getStandardAuthorityType(authorityType: AuthorityType): AuthorityType {
  switch (authorityType) {
    case AuthorityType.Ed25519Session:
      return AuthorityType.Ed25519;
    case AuthorityType.Secp256k1Session:
      return AuthorityType.Secp256k1;
    case AuthorityType.Secp256r1Session:
      return AuthorityType.Secp256r1;
    case AuthorityType.ProgramExecSession:
      return AuthorityType.ProgramExec;
    default:
      return authorityType;
  }
}
