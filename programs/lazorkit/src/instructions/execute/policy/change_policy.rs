use super::NewWalletAuthority;
use crate::constants::PASSKEY_PUBLIC_KEY_SIZE;
use crate::error::LazorKitError;
use crate::security::validation;
use crate::state::{WalletAuthority, WalletState};
use crate::utils::{
    compute_change_policy_message_hash, compute_instruction_hash, create_wallet_authority_hash,
    execute_cpi, get_wallet_authority, sighash, split_remaining_accounts,
    verify_authorization_hash,
};
use crate::{constants::SMART_WALLET_SEED, ID};
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct ChangePolicyArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub split_index: u16,
    pub delete_policy_data: Vec<u8>,
    pub init_policy_data: Vec<u8>,
    pub new_wallet_authoritys: Vec<NewWalletAuthority>,
    pub wallet_authority_split_index: Option<u16>,
    pub timestamp: i64,
}

/// Change the policy program and policy data for a wallet
pub fn change_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, ChangePolicy<'info>>,
    args: ChangePolicyArgs,
) -> Result<()> {
    validation::validate_instruction_timestamp(args.timestamp)?;

    let smart_wallet_key = ctx.accounts.smart_wallet.key();
    let wallet_authority_key = ctx.accounts.wallet_authority.key();
    let old_policy_program_key = ctx.accounts.old_policy_program.key();
    let new_policy_program_key = ctx.accounts.new_policy_program.key();
    let credential_hash = ctx.accounts.wallet_authority.credential_hash;
    let last_nonce = ctx.accounts.wallet_state.last_nonce;

    // Verify old policy program matches current wallet_state.policy_program
    require!(
        old_policy_program_key == ctx.accounts.wallet_state.policy_program,
        LazorKitError::InvalidInstruction
    );

    // Verify new policy program is different and not self-reentrancy
    require!(
        new_policy_program_key != old_policy_program_key,
        LazorKitError::InvalidInstruction
    );
    require!(
        new_policy_program_key != ID,
        LazorKitError::ReentrancyDetected
    );

    // Split remaining accounts: policy accounts and wallet_authority accounts
    let (policy_accounts, wallet_authority_accounts) =
        if let Some(wallet_authority_split) = args.wallet_authority_split_index {
            split_remaining_accounts(&ctx.remaining_accounts, wallet_authority_split)?
        } else {
            (ctx.remaining_accounts, &[] as &[AccountInfo])
        };

    let (delete_policy_accounts, init_policy_accounts) =
        split_remaining_accounts(policy_accounts, args.split_index)?;

    // Compute instruction hashes for message verification
    let delete_policy_hash = compute_instruction_hash(
        &args.delete_policy_data,
        delete_policy_accounts,
        old_policy_program_key,
    )?;
    let init_policy_hash = compute_instruction_hash(
        &args.init_policy_data,
        init_policy_accounts,
        new_policy_program_key,
    )?;

    // Compute expected message hash (similar to execute but for policy change)
    let expected_message_hash = compute_change_policy_message_hash(
        last_nonce,
        args.timestamp,
        delete_policy_hash,
        init_policy_hash,
    )?;

    // Verify passkey authentication
    verify_authorization_hash(
        &ctx.accounts.ix_sysvar,
        args.passkey_public_key,
        args.signature,
        &args.client_data_json_raw,
        &args.authenticator_data_raw,
        args.verify_instruction_index,
        expected_message_hash,
    )?;

    // Get policy signer for both old and new policy programs
    let wallet_authority =
        get_wallet_authority(smart_wallet_key, wallet_authority_key, credential_hash)?;

    // Validate delete_policy instruction discriminator
    require!(
        args.delete_policy_data.get(0..8) == Some(&sighash("global", "delete_policy")),
        LazorKitError::InvalidInstructionDiscriminator
    );

    // Call old policy program to delete policy
    execute_cpi(
        delete_policy_accounts,
        &args.delete_policy_data,
        &ctx.accounts.old_policy_program,
        &wallet_authority,
    )?;

    // Validate init_policy instruction discriminator
    require!(
        args.init_policy_data.get(0..8) == Some(&sighash("global", "init_policy")),
        LazorKitError::InvalidInstructionDiscriminator
    );

    // Call new policy program to initialize policy
    let new_policy_data = execute_cpi(
        init_policy_accounts,
        &args.init_policy_data,
        &ctx.accounts.new_policy_program,
        &wallet_authority,
    )?;

    // Calculate required space for wallet_state
    let current_space = ctx.accounts.wallet_state.to_account_info().data_len();
    let required_space =
        WalletState::DISCRIMINATOR.len() + WalletState::INIT_SPACE + new_policy_data.len();

    // Resize account if needed (must be done before updating data to ensure proper serialization)
    if required_space != current_space {
        let rent = Rent::get()?;
        let current_rent = rent.minimum_balance(current_space);
        let required_rent = rent.minimum_balance(required_space);

        if required_space > current_space {
            // Need to increase size - realloc and transfer additional rent
            ctx.accounts
                .wallet_state
                .to_account_info()
                .realloc(required_space, false)?;

            let additional_rent = required_rent - current_rent;
            if additional_rent > 0 {
                anchor_lang::solana_program::program::invoke(
                    &anchor_lang::solana_program::system_instruction::transfer(
                        ctx.accounts.payer.key,
                        &ctx.accounts.wallet_state.key(),
                        additional_rent,
                    ),
                    &[
                        ctx.accounts.payer.to_account_info(),
                        ctx.accounts.wallet_state.to_account_info(),
                        ctx.accounts.system_program.to_account_info(),
                    ],
                )?;
            }
        } else {
            // Need to decrease size - realloc down (rent will be refunded to wallet_state)
            // Note: This is safe because we're about to update the data with new_policy_data
            // which is smaller, so the old data will be overwritten anyway
            ctx.accounts
                .wallet_state
                .to_account_info()
                .realloc(required_space, false)?;
        }
    }

    // Update wallet_state with new policy program and data
    // This will serialize the entire struct, overwriting any old data
    ctx.accounts.wallet_state.policy_program = new_policy_program_key;
    ctx.accounts.wallet_state.policy_data = new_policy_data;

    // Create new wallet authorities if provided
    if !args.new_wallet_authoritys.is_empty() {
        require!(
            wallet_authority_accounts.len() >= args.new_wallet_authoritys.len(),
            LazorKitError::InsufficientCpiAccounts
        );

        for (i, new_authority) in args.new_wallet_authoritys.iter().enumerate() {
            if i >= wallet_authority_accounts.len() {
                break;
            }

            let wallet_authority_account = &wallet_authority_accounts[i];
            let wallet_authority_hash =
                create_wallet_authority_hash(smart_wallet_key, new_authority.credential_hash);

            // Verify the account matches expected PDA
            let (expected_pda, _bump) = Pubkey::find_program_address(
                &[WalletAuthority::PREFIX_SEED, &wallet_authority_hash],
                &ID,
            );

            require!(
                wallet_authority_account.key() == expected_pda,
                LazorKitError::InvalidInstruction
            );

            // Initialize wallet authority if needed
            crate::utils::init_wallet_authority_if_needed(
                wallet_authority_account,
                smart_wallet_key,
                new_authority.passkey_public_key,
                new_authority.credential_hash,
                &ctx.accounts.payer.to_account_info(),
                &ctx.accounts.system_program.to_account_info(),
            )?;
        }
    }

    // Increment nonce
    ctx.accounts.wallet_state.last_nonce = validation::safe_increment_nonce(last_nonce);

    Ok(())
}

#[derive(Accounts)]
#[instruction(args: ChangePolicyArgs)]
pub struct ChangePolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        mut,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

    #[account(
        mut,
        seeds = [SMART_WALLET_SEED, &wallet_state.base_seed, &wallet_state.salt.to_le_bytes()],
        bump = wallet_state.bump,
    )]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        seeds = [WalletAuthority::PREFIX_SEED, &create_wallet_authority_hash(smart_wallet.key(), wallet_authority.credential_hash)],
        bump,
        owner = ID,
    )]
    pub wallet_authority: Box<Account<'info, WalletAuthority>>,

    #[account(
        executable,
        address = wallet_state.policy_program
    )]
    /// CHECK: Validated to be executable and match wallet_state.policy_program
    pub old_policy_program: UncheckedAccount<'info>,

    #[account(
        executable,
        constraint = new_policy_program.key() != ID @ LazorKitError::ReentrancyDetected
    )]
    /// CHECK: Validated to be executable and not self-reentrancy
    pub new_policy_program: UncheckedAccount<'info>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: Instruction sysvar
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
