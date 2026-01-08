//! Instruction definitions for the Lazorkit V2 wallet program.

use num_enum::{FromPrimitive, IntoPrimitive};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError};
use shank::{ShankContext, ShankInstruction};

/// Instructions supported by the Lazorkit V2 wallet program.
#[derive(Clone, Copy, Debug, ShankContext, ShankInstruction, FromPrimitive, IntoPrimitive)]
#[rustfmt::skip]
#[repr(u16)]
pub enum LazorkitInstruction {
    /// Creates a new Lazorkit wallet.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account to create
    /// 2. `[writable, signer]` Payer account for rent
    /// 3. `[writable]` Smart wallet PDA to create
    /// 4. `[writable]` System program account
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, writable, signer, name="payer", desc="the payer")]
    #[account(2, writable, name="smart_wallet", desc="the smart wallet PDA")]
    #[account(3, name="system_program", desc="the system program")]
    #[num_enum(default)]
    CreateSmartWallet = 0,
    
    /// Signs and executes a transaction with plugin checks.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account
    /// 2. `[writable, signer]` Smart wallet PDA
    /// 3. `[]` WalletAuthority account
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, writable, signer, name="smart_wallet", desc="the smart wallet PDA")]
    #[account(2, name="wallet_authority", desc="the wallet authority")]
    Sign = 1,
    
    /// Adds a new authority to the wallet.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account
    /// 2. `[writable, signer]` Payer account
    /// 3. `[writable]` System program account
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, writable, signer, name="payer", desc="the payer")]
    #[account(2, name="system_program", desc="the system program")]
    AddAuthority = 2,
    
    /// Updates an existing authority in the wallet.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account
    /// 2. `[signer]` Smart wallet PDA
    /// 3. `[]` Acting WalletAuthority account
    /// 4. `[writable]` Authority to update
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, signer, name="smart_wallet", desc="the smart wallet PDA")]
    #[account(2, name="acting_authority", desc="the acting wallet authority")]
    #[account(3, writable, name="authority_to_update", desc="the authority to update")]
    UpdateAuthority = 6,
    
    /// Removes an authority from the wallet.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account
    /// 2. `[writable, signer]` Payer account (to receive refunded lamports)
    /// 3. `[signer]` Smart wallet PDA
    /// 4. `[]` Acting WalletAuthority account
    /// 5. `[writable]` Authority to remove
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, writable, signer, name="payer", desc="the payer")]
    #[account(2, signer, name="smart_wallet", desc="the smart wallet PDA")]
    #[account(3, name="acting_authority", desc="the acting wallet authority")]
    #[account(4, writable, name="authority_to_remove", desc="the authority to remove")]
    RemoveAuthority = 7,
    
    /// Adds a plugin to the wallet's plugin registry.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account
    /// 2. `[writable, signer]` Payer account
    /// 3. `[signer]` Smart wallet PDA
    /// 4. `[]` Acting WalletAuthority account
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, writable, signer, name="payer", desc="the payer")]
    #[account(2, signer, name="smart_wallet", desc="the smart wallet PDA")]
    #[account(3, name="acting_authority", desc="the acting wallet authority")]
    AddPlugin = 3,
    
    /// Removes a plugin from the wallet's plugin registry.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account
    /// 2. `[signer]` Smart wallet PDA
    /// 3. `[]` Acting WalletAuthority account
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, signer, name="smart_wallet", desc="the smart wallet PDA")]
    #[account(2, name="acting_authority", desc="the acting wallet authority")]
    RemovePlugin = 4,
    
    /// Updates a plugin in the wallet's plugin registry.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account
    /// 2. `[signer]` Smart wallet PDA
    /// 3. `[]` Acting WalletAuthority account
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, signer, name="smart_wallet", desc="the smart wallet PDA")]
    #[account(2, name="acting_authority", desc="the acting wallet authority")]
    UpdatePlugin = 5,
    
    /// Creates a new authentication session for a wallet authority.
    ///
    /// Required accounts:
    /// 1. `[writable]` WalletState account
    /// 2. `[writable]` WalletAuthority account to create session for
    #[account(0, writable, name="wallet_state", desc="the wallet state account")]
    #[account(1, writable, name="wallet_authority", desc="the wallet authority to create session for")]
    CreateSession = 8,
}
