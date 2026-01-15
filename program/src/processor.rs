//! Instruction Processor
//!
//! Thin dispatcher that routes instructions to individual handlers.

use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

use crate::actions;
use crate::instruction::LazorKitInstruction;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = LazorKitInstruction::unpack(instruction_data)?;
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
            plugins_config,
            authorization_data,
        } => actions::process_add_authority(
            program_id,
            accounts,
            acting_role_id,
            authority_type,
            authority_data,
            plugins_config,
            authorization_data,
        ),

        LazorKitInstruction::RemoveAuthority {
            acting_role_id,
            target_role_id,
        } => {
            actions::process_remove_authority(program_id, accounts, acting_role_id, target_role_id)
        },

        LazorKitInstruction::UpdateAuthority {
            acting_role_id,
            target_role_id,
            operation,
            payload,
        } => actions::process_update_authority(
            program_id,
            accounts,
            acting_role_id,
            target_role_id,
            operation,
            payload,
        ),

        LazorKitInstruction::CreateSession {
            role_id,
            session_key,
            duration,
        } => actions::process_create_session(program_id, accounts, role_id, session_key, duration),

        LazorKitInstruction::Execute {
            role_id,
            instruction_payload,
        } => actions::process_execute(program_id, accounts, role_id, &instruction_payload),

        LazorKitInstruction::TransferOwnership {
            new_owner_authority_type,
            new_owner_authority_data,
        } => actions::process_transfer_ownership(
            program_id,
            accounts,
            new_owner_authority_type,
            new_owner_authority_data,
        ),
    }
}
