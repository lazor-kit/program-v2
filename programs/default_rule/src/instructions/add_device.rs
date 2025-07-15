use crate::{state::Rule, ID};
use anchor_lang::prelude::*;
use lazorkit::{program::Lazorkit, state::SmartWalletAuthenticator};

pub fn add_device(ctx: Context<AddDevice>) -> Result<()> {
    let new_rule = &mut ctx.accounts.new_rule;

    new_rule.smart_wallet = ctx.accounts.rule.smart_wallet.key();
    new_rule.smart_wallet_authenticator = ctx.accounts.new_smart_wallet_authenticator.key();

    Ok(())
}

#[derive(Accounts)]
pub struct AddDevice<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        owner = lazorkit.key(),
        signer,
    )]
    pub smart_wallet_authenticator: Account<'info, SmartWalletAuthenticator>,

    #[account(
        owner = lazorkit.key(),
    )]
    /// CHECK:
    pub new_smart_wallet_authenticator: UncheckedAccount<'info>,

    #[account(
        seeds = [b"rule".as_ref(), smart_wallet_authenticator.key().as_ref()],
        bump,
        owner = ID,
        constraint = rule.smart_wallet_authenticator == smart_wallet_authenticator.key(),
    )]
    pub rule: Account<'info, Rule>,

    #[account(
        init,
        payer = payer,
        space = 8 + Rule::INIT_SPACE,
        seeds = [b"rule".as_ref(), new_smart_wallet_authenticator.key().as_ref()],
        bump,
    )]
    pub new_rule: Account<'info, Rule>,

    pub lazorkit: Program<'info, Lazorkit>,

    pub system_program: Program<'info, System>,
}
