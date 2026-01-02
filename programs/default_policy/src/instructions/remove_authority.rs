use anchor_lang::prelude::*;
use lazorkit::{state::WalletAuthority, ID as LAZORKIT_ID};

use crate::{error::PolicyError, state::PolicyStruct};

/// Verify that a passkey is authorized for a smart wallet transaction
pub fn remove_authority(
    ctx: Context<RemoveAuthority>,
    policy_data: Vec<u8>,
    new_authority: Pubkey,
) -> Result<PolicyStruct> {
    let mut policy_struct = PolicyStruct::try_from_slice(&policy_data)?;

    require!(
        policy_struct.smart_wallet == ctx.accounts.smart_wallet.key(),
        PolicyError::InvalidSmartWallet
    );

    require!(
        policy_struct
            .authoritis
            .contains(&ctx.accounts.authority.key()),
        PolicyError::Unauthorized
    );

    policy_struct.authoritis.push(new_authority);

    Ok(policy_struct)
}

#[derive(Accounts)]
pub struct RemoveAuthority<'info> {
    #[account(
        signer,
        owner = LAZORKIT_ID,
        constraint = authority.smart_wallet == smart_wallet.key()
    )]
    pub authority: Account<'info, WalletAuthority>,

    pub smart_wallet: SystemAccount<'info>,
}
