use anchor_lang::prelude::*;

use crate::instructions::CreateDeferredExecutionArgs;
use crate::security::validation;
use crate::state::{
    ExecuteSessionMessage, PolicyProgramRegistry, ProgramConfig, SmartWalletData,
    TransactionSession, WalletDevice,
};
use crate::utils::{
    execute_cpi, get_wallet_device_signer, sighash, verify_authorization, PasskeyExt,
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError, ID};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn create_deferred_execution(
    ctx: Context<CreateDeferredExecution>,
    args: CreateDeferredExecutionArgs,
) -> Result<()> {
    // 0. Validate
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;
    validation::validate_policy_data(&args.policy_data)?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);

    // 1. Authorization -> typed ExecuteMessage
    let msg: ExecuteSessionMessage = verify_authorization::<ExecuteSessionMessage>(
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

    // 2. In session mode, all remaining accounts are for policy checking
    let policy_accounts = &ctx.remaining_accounts[..];

    // 3. Optional policy-check now (bind policy & validate hashes)
    // Ensure policy program matches config and registry
    validation::validate_program_executable(&ctx.accounts.policy_program)?;
    require!(
        ctx.accounts.policy_program.key() == ctx.accounts.smart_wallet_data.policy_program_id,
        LazorKitError::InvalidProgramAddress
    );
    crate::utils::check_whitelist(
        &ctx.accounts.policy_program_registry,
        &ctx.accounts.policy_program.key(),
    )?;

    // Compare policy_data hash with message
    require!(
        hash(&args.policy_data).to_bytes() == msg.policy_data_hash,
        LazorKitError::InvalidInstructionData
    );
    // Compare policy_accounts hash with message
    let mut rh = Hasher::default();
    rh.hash(ctx.accounts.policy_program.key.as_ref());
    for a in policy_accounts.iter() {
        rh.hash(a.key.as_ref());
        rh.hash(&[a.is_signer as u8]);
        rh.hash(&[a.is_writable as u8]);
    }
    require!(
        rh.result().to_bytes() == msg.policy_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // Execute policy check
    let policy_signer = get_wallet_device_signer(
        &args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );
    require!(
        args.policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidCheckPolicyDiscriminator
    );
    execute_cpi(
        policy_accounts,
        &args.policy_data,
        &ctx.accounts.policy_program,
        policy_signer,
    )?;

    // 5. Write session using hashes from message
    let session: &mut Account<'_, TransactionSession> = &mut ctx.accounts.transaction_session;
    session.owner_wallet_address = ctx.accounts.smart_wallet.key();
    session.instruction_data_hash = msg.cpi_data_hash;
    session.accounts_metadata_hash = msg.cpi_accounts_hash;
    session.authorized_nonce = ctx.accounts.smart_wallet_data.last_nonce;
    session.expires_at = args.expires_at;
    session.rent_refund_address = ctx.accounts.payer.key();
    session.vault_index = args.vault_index;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: CreateDeferredExecutionArgs)]
pub struct CreateDeferredExecution<'info> {
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

    #[account(
        seeds = [PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = ID
    )]
    pub policy_program_registry: Box<Account<'info, PolicyProgramRegistry>>,

    /// Policy program for optional policy enforcement at session creation
    /// CHECK: validated via executable + registry
    #[account(executable)]
    pub policy_program: UncheckedAccount<'info>,

    /// New transaction session account (rent payer: payer)
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + TransactionSession::INIT_SPACE,
        seeds = [TransactionSession::PREFIX_SEED, smart_wallet.key().as_ref(), &smart_wallet_data.last_nonce.to_le_bytes()],
        bump,
        owner = ID,
    )]
    pub transaction_session: Account<'info, TransactionSession>,

    /// CHECK: instructions sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
