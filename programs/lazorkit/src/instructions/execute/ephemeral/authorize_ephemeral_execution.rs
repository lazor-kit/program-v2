use anchor_lang::prelude::*;
use anchor_lang::solana_program::hash::Hasher;

use crate::instructions::AuthorizeEphemeralExecutionArgs;
use crate::security::validation;
use crate::state::{
    AuthorizeEphemeralExecutionMessage, EphemeralAuthorization, ProgramConfig, SmartWalletData,
    WalletDevice,
};
use crate::utils::{verify_authorization, PasskeyExt};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};

pub fn authorize_ephemeral_execution(
    ctx: Context<AuthorizeEphemeralExecution>,
    args: AuthorizeEphemeralExecutionArgs,
) -> Result<()> {
    // 0. Validate
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);

    // Validate input: for n instructions, we need n-1 split indices
    require!(
        !args.instruction_data_list.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );
    require!(
        args.instruction_data_list.len() == args.split_index.len() + 1,
        LazorKitError::InvalidInstructionData
    );

    // 1. Authorization -> typed AuthorizeEphemeralExecutionMessage
    let msg: AuthorizeEphemeralExecutionMessage =
        verify_authorization::<AuthorizeEphemeralExecutionMessage>(
            &ctx.accounts.ix_sysvar,
            &ctx.accounts.wallet_device,
            ctx.accounts.smart_wallet.key(),
            args.passkey_public_key,
            args.signature.clone(),
            &args.client_data_json_raw,
            &args.authenticator_data_raw,
            args.verify_instruction_index,
            ctx.accounts.smart_wallet_data.last_nonce,
        )?;

    // 2. Verify message fields match args
    require!(
        msg.ephemeral_key == args.ephemeral_public_key,
        LazorKitError::InvalidInstructionData
    );
    require!(
        msg.expires_at == args.expires_at,
        LazorKitError::InvalidInstructionData
    );

    // 3. Create combined hashes for verification
    // Hash all instruction data
    let serialized_cpi_data = args
        .instruction_data_list
        .try_to_vec()
        .map_err(|_| LazorKitError::InvalidInstructionData)?;
    let data_hash = anchor_lang::solana_program::hash::hash(&serialized_cpi_data).to_bytes();

    // Hash all accounts
    let mut all_accounts_hasher = Hasher::default();
    for acc in ctx.remaining_accounts.iter() {
        all_accounts_hasher.hash(acc.key.as_ref());
        all_accounts_hasher.hash(&[acc.is_signer as u8]);
        all_accounts_hasher.hash(&[acc.is_writable as u8]);
    }
    let accounts_hash = all_accounts_hasher.result().to_bytes();

    // 4. Verify hashes match message
    require!(
        data_hash == msg.data_hash,
        LazorKitError::InvalidInstructionData
    );
    require!(
        accounts_hash == msg.accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // 5. Validate expiration time (max 1 hour from now)
    let now = Clock::get()?.unix_timestamp;
    require!(args.expires_at > now, LazorKitError::InvalidInstructionData);
    require!(
        args.expires_at <= now + 3600, // Max 1 hour
        LazorKitError::InvalidInstructionData
    );

    // 6. Validate account ranges using split_index
    let mut account_ranges = Vec::new();
    let mut start = 0usize;

    // Calculate account ranges for each instruction using split indices
    for &split_point in args.split_index.iter() {
        let end = split_point as usize;
        require!(
            end > start && end <= ctx.remaining_accounts.len(),
            LazorKitError::AccountSliceOutOfBounds
        );
        account_ranges.push((start, end));
        start = end;
    }

    // Add the last instruction range (from last split to end)
    require!(
        start < ctx.remaining_accounts.len(),
        LazorKitError::AccountSliceOutOfBounds
    );
    account_ranges.push((start, ctx.remaining_accounts.len()));

    // Validate each instruction's programs for security
    for (_i, &(range_start, range_end)) in account_ranges.iter().enumerate() {
        let instruction_accounts = &ctx.remaining_accounts[range_start..range_end];

        require!(
            !instruction_accounts.is_empty(),
            LazorKitError::InsufficientCpiAccounts
        );

        // First account in each instruction slice is the program ID
        let program_account = &instruction_accounts[0];

        // Validate program is executable
        if !program_account.executable {
            return Err(LazorKitError::ProgramNotExecutable.into());
        }

        // Ensure program is not this program (prevent reentrancy)
        if program_account.key() == crate::ID {
            return Err(LazorKitError::ReentrancyDetected.into());
        }
    }

    // 7. Write ephemeral authorization
    let authorization = &mut ctx.accounts.ephemeral_authorization;
    authorization.owner_wallet_address = ctx.accounts.smart_wallet.key();
    authorization.ephemeral_public_key = args.ephemeral_public_key;
    authorization.expires_at = args.expires_at;
    authorization.fee_payer_address = ctx.accounts.payer.key();
    authorization.rent_refund_address = ctx.accounts.payer.key();
    authorization.vault_index = args.vault_index;
    authorization.instruction_data_hash = data_hash;
    authorization.accounts_metadata_hash = accounts_hash;

    // 8. Increment nonce
    ctx.accounts.smart_wallet_data.last_nonce = ctx
        .accounts
        .smart_wallet_data
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: AuthorizeEphemeralExecutionArgs)]
pub struct AuthorizeEphemeralExecution<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(seeds = [ProgramConfig::PREFIX_SEED], bump, owner = ID)]
    pub config: Box<Account<'info, ProgramConfig>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_data.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_data.bump,
        
    )]
    /// CHECK: PDA verified
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [SmartWalletData::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub smart_wallet_data: Box<Account<'info, SmartWalletData>>,

    #[account(
        seeds = [
            WalletDevice::PREFIX_SEED,
            smart_wallet.key().as_ref(),
            args.passkey_public_key.to_hashed_bytes(smart_wallet.key()).as_ref()
        ],
        bump = wallet_device.bump,
        owner = ID,
        constraint = wallet_device.smart_wallet_address == smart_wallet.key() @ LazorKitError::SmartWalletDataMismatch,
        constraint = wallet_device.passkey_public_key == args.passkey_public_key @ LazorKitError::PasskeyMismatch
    )]
    pub wallet_device: Box<Account<'info, WalletDevice>>,

    /// New ephemeral authorization account (rent payer: payer)
    #[account(
        init,
        payer = payer,
        space = 8 + EphemeralAuthorization::INIT_SPACE,
        seeds = [EphemeralAuthorization::PREFIX_SEED, smart_wallet.key().as_ref(), args.ephemeral_public_key.as_ref()],
        bump,
        owner = ID,
    )]
    pub ephemeral_authorization: Account<'info, EphemeralAuthorization>,

    /// CHECK: instructions sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
