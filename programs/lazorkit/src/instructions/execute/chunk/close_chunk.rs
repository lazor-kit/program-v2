use anchor_lang::prelude::*;

use crate::error::LazorKitError;
use crate::state::{Chunk, SmartWalletConfig};
use crate::{constants::SMART_WALLET_SEED, ID};

/// Close an expired chunk to refund rent
///
/// This instruction allows closing a chunk that has expired (timestamp too old)
/// without executing the CPI instructions. This is useful for cleanup when
/// a chunk session has timed out.
pub fn close_chunk(ctx: Context<CloseChunk>) -> Result<()> {
    let chunk = &ctx.accounts.chunk;
    
    // Verify the chunk belongs to the correct smart wallet
    require!(
        chunk.owner_wallet_address == ctx.accounts.smart_wallet.key(),
        LazorKitError::InvalidAccountOwner
    );

    // Check if the chunk session has expired based on timestamp
    let now = Clock::get()?.unix_timestamp;
    require!(
        chunk.authorized_timestamp < now - crate::security::MAX_SESSION_TTL_SECONDS,
        LazorKitError::TransactionTooOld
    );

    msg!("Closing expired chunk: wallet={}, nonce={}, expired_at={}", 
         ctx.accounts.smart_wallet.key(), 
         chunk.authorized_nonce,
         chunk.authorized_timestamp);
    
    Ok(())
}

#[derive(Accounts)]
pub struct CloseChunk<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_config.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_config.bump,
    )]
    /// CHECK: PDA verified
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        seeds = [SmartWalletConfig::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_config: Box<Account<'info, SmartWalletConfig>>,

    /// Expired chunk to close and refund rent
    #[account(
        mut,
        seeds = [
            Chunk::PREFIX_SEED,
            smart_wallet.key.as_ref(),
            &chunk.authorized_nonce.to_le_bytes(),
        ], 
        close = session_refund,
        owner = ID,
        bump,
    )]
    pub chunk: Account<'info, Chunk>,

    /// CHECK: rent refund destination (stored in session)
    #[account(mut, address = chunk.rent_refund_address)]
    pub session_refund: UncheckedAccount<'info>,
}
