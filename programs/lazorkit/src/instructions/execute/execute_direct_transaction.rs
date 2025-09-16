use anchor_lang::prelude::*;

use crate::instructions::{Args as _, ExecuteDirectTransactionArgs};
use crate::security::validation;
use crate::state::{ExecuteMessage, LazorKitVault};
use crate::utils::{
    check_whitelist, execute_cpi, get_wallet_device_signer, sighash, split_remaining_accounts,
    verify_authorization, PdaSigner,
};
use crate::{constants::SMART_WALLET_SEED, error::LazorKitError};
use anchor_lang::solana_program::hash::{hash, Hasher};

pub fn execute_direct_transaction<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ExecuteDirectTransaction<'info>>,
    args: ExecuteDirectTransactionArgs,
) -> Result<()> {
    // 0. Validate args and global state
    args.validate()?;
    require!(!ctx.accounts.config.is_paused, LazorKitError::ProgramPaused);
    validation::validate_remaining_accounts(&ctx.remaining_accounts)?;

    // 0.1 Verify authorization and parse typed message
    let msg: ExecuteMessage = verify_authorization(
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

    // 1. Validate and check policy program
    let policy_program_info = &ctx.accounts.policy_program;

    // Ensure policy program is executable
    validation::validate_program_executable(policy_program_info)?;

    // Verify policy program is registered
    check_whitelist(
        &ctx.accounts.policy_program_registry,
        &policy_program_info.key(),
    )?;

    // Ensure policy program matches wallet configuration
    require!(
        policy_program_info.key() == ctx.accounts.smart_wallet_data.policy_program_id,
        LazorKitError::InvalidProgramAddress
    );

    // 2. Prepare PDA signer for policy CPI
    let policy_signer = get_wallet_device_signer(
        &args.passkey_public_key,
        ctx.accounts.smart_wallet.key(),
        ctx.accounts.wallet_device.bump,
    );

    // 3. Split remaining accounts
    let (policy_accounts, cpi_accounts) =
        split_remaining_accounts(&ctx.remaining_accounts, args.split_index)?;

    // Validate account counts
    require!(
        !policy_accounts.is_empty(),
        LazorKitError::InsufficientPolicyAccounts
    );

    // 4. Verify policy discriminator on provided policy_data
    let policy_data = &args.policy_data;
    require!(
        policy_data.get(0..8) == Some(&sighash("global", "check_policy")),
        LazorKitError::InvalidCheckPolicyDiscriminator
    );

    // 4.1 Validate policy_data size and compare hash from message
    validation::validate_policy_data(policy_data)?;
    require!(
        hash(policy_data).to_bytes() == msg.policy_data_hash,
        LazorKitError::InvalidInstructionData
    );

    // 4.2 Compare policy accounts hash against message
    let mut rh = Hasher::default();
    rh.hash(policy_program_info.key.as_ref());
    for acc in policy_accounts.iter() {
        rh.hash(acc.key.as_ref());
        rh.hash(&[acc.is_signer as u8]);
        rh.hash(&[acc.is_writable as u8]);
    }
    require!(
        rh.result().to_bytes() == msg.policy_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // 5. Execute policy CPI to check if the transaction is allowed

    execute_cpi(
        policy_accounts,
        policy_data,
        policy_program_info,
        policy_signer,
    )?;

    // 6. Validate CPI payload and compare hashes
    validation::validate_cpi_data(&args.cpi_data)?;
    require!(
        hash(&args.cpi_data).to_bytes() == msg.cpi_data_hash,
        LazorKitError::InvalidInstructionData
    );
    let mut ch = Hasher::default();
    ch.hash(ctx.accounts.cpi_program.key.as_ref());
    for acc in cpi_accounts.iter() {
        ch.hash(acc.key.as_ref());
        ch.hash(&[acc.is_signer as u8]);
        ch.hash(&[acc.is_writable as u8]);
    }
    require!(
        ch.result().to_bytes() == msg.cpi_accounts_hash,
        LazorKitError::InvalidAccountData
    );

    // === General CPI ===
    validation::validate_program_executable(&ctx.accounts.cpi_program)?;
    require!(
        ctx.accounts.cpi_program.key() != crate::ID,
        LazorKitError::ReentrancyDetected
    );
    require!(
        !cpi_accounts.is_empty(),
        LazorKitError::InsufficientCpiAccounts
    );

    // Create wallet signer
    let wallet_signer = PdaSigner {
        seeds: vec![
            SMART_WALLET_SEED.to_vec(),
            ctx.accounts
                .smart_wallet_data
                .wallet_id
                .to_le_bytes()
                .to_vec(),
        ],
        bump: ctx.accounts.smart_wallet_data.bump,
    };
    execute_cpi(
        cpi_accounts,
        &args.cpi_data,
        &ctx.accounts.cpi_program,
        wallet_signer.clone(),
    )?;

    // 8. Increment nonce
    ctx.accounts.smart_wallet_data.last_nonce = ctx
        .accounts
        .smart_wallet_data
        .last_nonce
        .checked_add(1)
        .ok_or(LazorKitError::NonceOverflow)?;

    // Validate that the provided vault matches the vault index from args
    crate::state::LazorKitVault::validate_vault_for_index(
        &ctx.accounts.lazorkit_vault.key(),
        args.vault_index,
        &crate::ID,
    )?;

    // Distribute fees to payer, referral, and lazorkit vault
    crate::utils::distribute_fees(
        &ctx.accounts.config,
        &ctx.accounts.smart_wallet.to_account_info(),
        &ctx.accounts.payer.to_account_info(),
        &ctx.accounts.referral.to_account_info(),
        &ctx.accounts.lazorkit_vault.to_account_info(),
        &ctx.accounts.system_program,
        wallet_signer,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: ExecuteDirectTransactionArgs)]
pub struct ExecuteDirectTransaction<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, smart_wallet_data.wallet_id.to_le_bytes().as_ref()],
        bump = smart_wallet_data.bump,
    )]
    /// CHECK: PDA verified by seeds
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [crate::state::SmartWalletData::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = crate::ID,
    )]
    pub smart_wallet_data: Box<Account<'info, crate::state::SmartWalletData>>,

    /// CHECK: referral account (matches smart_wallet_data.referral)
    #[account(mut, address = smart_wallet_data.referral_address)]
    pub referral: UncheckedAccount<'info>,

    /// LazorKit vault (empty PDA that holds SOL) - random vault selected by client
    #[account(
        mut,
        seeds = [LazorKitVault::PREFIX_SEED, &args.vault_index.to_le_bytes()],
        bump,
    )]
    /// CHECK: Empty PDA vault that only holds SOL, validated to be correct random vault
    pub lazorkit_vault: SystemAccount<'info>,

    #[account(owner = crate::ID)]
    pub wallet_device: Box<Account<'info, crate::state::WalletDevice>>,
    #[account(
        seeds = [crate::state::PolicyProgramRegistry::PREFIX_SEED],
        bump,
        owner = crate::ID
    )]
    pub policy_program_registry: Box<Account<'info, crate::state::PolicyProgramRegistry>>,
    /// CHECK: must be executable (policy program)
    #[account(executable)]
    pub policy_program: UncheckedAccount<'info>,
    /// CHECK: must be executable (target program)
    #[account(executable)]
    pub cpi_program: UncheckedAccount<'info>,
    #[account(
        seeds = [crate::state::ProgramConfig::PREFIX_SEED],
        bump,
        owner = crate::ID
    )]
    pub config: Box<Account<'info, crate::state::ProgramConfig>>,
    /// CHECK: instruction sysvar
    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
