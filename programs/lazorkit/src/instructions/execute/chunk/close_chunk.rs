use anchor_lang::prelude::*;

use crate::error::LazorKitError;
use crate::state::{Chunk, WalletState};
use crate::{constants::SMART_WALLET_SEED, ID};

/// Close an expired chunk to refund rent
pub fn close_chunk(ctx: Context<CloseChunk>) -> Result<()> {
    let chunk = &ctx.accounts.chunk;

    require!(
        chunk.owner_wallet_address == ctx.accounts.smart_wallet.key(),
        LazorKitError::InvalidAccountOwner
    );

    let now = Clock::get()?.unix_timestamp;
    let session_end = chunk.authorized_timestamp + crate::security::MAX_SESSION_TTL_SECONDS;
    let is_expired = now > session_end;
    require!(is_expired, LazorKitError::TransactionTooOld);

    Ok(())
}

#[derive(Accounts)]
pub struct CloseChunk<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, wallet_state.wallet_id.to_le_bytes().as_ref()],
        bump = wallet_state.bump,
    )]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        mut,
        seeds = [
            Chunk::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            &chunk.authorized_nonce.to_le_bytes(),
        ], 
        close = session_refund,
        owner = ID,
        bump,
    )]
    pub chunk: Account<'info, Chunk>,

    #[account(mut, address = chunk.rent_refund_address)]
    /// CHECK: Validated to match chunk.rent_refund_address
    pub session_refund: UncheckedAccount<'info>,
}
