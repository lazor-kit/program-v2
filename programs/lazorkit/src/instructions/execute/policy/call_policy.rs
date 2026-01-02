use super::NewWalletAuthority;
use crate::constants::PASSKEY_PUBLIC_KEY_SIZE;
use crate::error::LazorKitError;
use crate::security::validation;
use crate::state::{WalletAuthority, WalletState};
use crate::utils::{
    compute_call_policy_message_hash, compute_instruction_hash, create_wallet_authority_hash,
    execute_cpi, get_wallet_authority, split_remaining_accounts, verify_authorization_hash,
};
use crate::ID;
use anchor_lang::prelude::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CallPolicyArgs {
    pub passkey_public_key: [u8; PASSKEY_PUBLIC_KEY_SIZE],
    pub signature: [u8; 64],
    pub client_data_json_raw: Vec<u8>,
    pub authenticator_data_raw: Vec<u8>,
    pub verify_instruction_index: u8,
    pub policy_data: Vec<u8>,
    pub new_wallet_authoritys: Vec<NewWalletAuthority>,
    pub wallet_authority_split_index: Option<u16>,
    pub timestamp: i64,
}

/// Call policy program to update policy data
pub fn call_policy<'c: 'info, 'info>(
    ctx: Context<'_, '_, 'c, 'info, CallPolicy<'info>>,
    args: CallPolicyArgs,
) -> Result<()> {
    validation::validate_instruction_timestamp(args.timestamp)?;

    let smart_wallet_key = ctx.accounts.smart_wallet.key();
    let wallet_authority_key = ctx.accounts.wallet_authority.key();
    let policy_program_key = ctx.accounts.policy_program.key();
    let credential_hash = ctx.accounts.wallet_authority.credential_hash;
    let last_nonce = ctx.accounts.wallet_state.last_nonce;

    // Verify policy program matches current wallet_state.policy_program
    require!(
        policy_program_key == ctx.accounts.wallet_state.policy_program,
        LazorKitError::InvalidInstruction
    );

    // Split remaining accounts: policy accounts and wallet_authority accounts
    let (policy_accounts, wallet_authority_accounts) =
        if let Some(wallet_authority_split) = args.wallet_authority_split_index {
            split_remaining_accounts(&ctx.remaining_accounts, wallet_authority_split)?
        } else {
            (ctx.remaining_accounts, &[] as &[AccountInfo])
        };

    // Compute instruction hash for message verification
    let policy_hash =
        compute_instruction_hash(&args.policy_data, policy_accounts, policy_program_key)?;

    // Compute expected message hash (only policy hash, no CPI hash)
    let expected_message_hash =
        compute_call_policy_message_hash(last_nonce, args.timestamp, policy_hash)?;

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

    // Get policy signer
    let wallet_authority = get_wallet_authority(smart_wallet_key, wallet_authority_key, credential_hash)?;

    // Validate policy instruction discriminator (can be any policy instruction, not just check_policy)
    // We don't enforce a specific discriminator here, allowing flexibility for different policy instructions

    // Call policy program to get updated policy data
    let new_policy_data = execute_cpi(
        policy_accounts,
        &args.policy_data,
        &ctx.accounts.policy_program,
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

    // Update wallet_state with new policy data (policy_program stays the same)
    // This will serialize the entire struct, overwriting any old data
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
#[instruction(args: CallPolicyArgs)]
pub struct CallPolicy<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(mut)]
    pub smart_wallet: SystemAccount<'info>,

    #[account(
        mut,
        seeds = [WalletState::PREFIX_SEED, smart_wallet.key().as_ref()],
        bump,
        owner = ID,
    )]
    pub wallet_state: Box<Account<'info, WalletState>>,

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
    pub policy_program: UncheckedAccount<'info>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: Instruction sysvar
    pub ix_sysvar: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}
