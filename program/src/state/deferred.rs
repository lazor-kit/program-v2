use no_padding::NoPadding;
use pinocchio::pubkey::Pubkey;

/// Deferred Execution Authorization Account.
///
/// Created during the `Authorize` instruction (tx1) to store a pre-authorized
/// set of instructions for later execution. The `ExecuteDeferred` instruction (tx2)
/// verifies the hashes and executes the instructions, then closes this account.
///
/// This enables large payloads (e.g., Jupiter swaps) that exceed the ~574 bytes
/// available in a single Secp256r1 Execute transaction.
#[repr(C, align(8))]
#[derive(NoPadding, Debug, Clone, Copy)]
pub struct DeferredExecAccount {
    /// Account discriminator (must be `4` for DeferredExec).
    pub discriminator: u8,
    /// Account version.
    pub version: u8,
    /// Bump seed for this PDA.
    pub bump: u8,
    /// Padding for alignment.
    pub _padding: [u8; 5],
    /// SHA256 of the serialized compact instructions bytes.
    pub instructions_hash: [u8; 32],
    /// SHA256 of all account pubkeys referenced by compact instructions.
    pub accounts_hash: [u8; 32],
    /// The wallet this authorization is for.
    pub wallet: Pubkey,
    /// The authority that created this authorization.
    pub authority: Pubkey,
    /// The payer who funded this account (receives rent refund on close).
    pub payer: Pubkey,
    /// Absolute slot at which this authorization expires.
    pub expires_at: u64,
}
// Layout: 1+1+1+5+32+32+32+32+32+8 = 176 bytes
