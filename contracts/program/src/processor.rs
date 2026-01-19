//! Instruction Processor
//!
//! Thin dispatcher that routes instructions to individual handlers.

use pinocchio::{
    account_info::AccountInfo, msg, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

use crate::actions;
use crate::instruction::LazorKitInstruction;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    msg!(
        "Processing Instruction. Raw: {:?}, Discriminator: {:?}",
        instruction_data,
        instruction_data.get(0)
    );
    let instruction = LazorKitInstruction::unpack(instruction_data).map_err(|e| {
        msg!("Failed to unpack instruction: {:?}", e);
        e
    })?;
    match instruction {
        LazorKitInstruction::CreateWallet {
            id,
            bump,
            wallet_bump,
            owner_authority_type,
            owner_authority_data,
        } => actions::process_create_wallet(
            program_id,
            accounts,
            id,
            bump,
            wallet_bump,
            owner_authority_type,
            owner_authority_data,
        ),

        LazorKitInstruction::AddAuthority {
            acting_role_id,
            authority_type,
            authority_data,
            authorization_data,
        } => actions::process_add_authority(
            program_id,
            accounts,
            acting_role_id,
            authority_type,
            authority_data,
            authorization_data,
        ),

        LazorKitInstruction::RemoveAuthority {
            acting_role_id,
            target_role_id,
            authorization_data,
        } => actions::process_remove_authority(
            program_id,
            accounts,
            acting_role_id,
            target_role_id,
            &authorization_data,
        ),

        LazorKitInstruction::UpdateAuthority {
            acting_role_id,
            target_role_id,
            new_authority_data,
            authorization_data,
        } => actions::process_update_authority(
            program_id,
            accounts,
            acting_role_id,
            target_role_id,
            new_authority_data,
            authorization_data,
        ),

        LazorKitInstruction::CreateSession {
            role_id,
            session_key,
            duration,
            authorization_data,
        } => actions::process_create_session(
            program_id,
            accounts,
            role_id,
            session_key,
            duration,
            &authorization_data,
        ),

        LazorKitInstruction::Execute {
            role_id,
            instruction_payload,
        } => actions::process_execute(program_id, accounts, role_id, &instruction_payload),

        LazorKitInstruction::TransferOwnership {
            new_owner_authority_type,
            new_owner_authority_data,
            auth_payload,
        } => actions::process_transfer_ownership(
            program_id,
            accounts,
            new_owner_authority_type,
            new_owner_authority_data,
            auth_payload,
        ),
    }
}
