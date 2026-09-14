use pinocchio::{
    account_info::AccountInfo, entrypoint, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::processor::*;

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let (discriminator, instruction_data) = instruction_data
        .split_first()
        .ok_or(ProgramError::InvalidInstructionData)?;

    match discriminator {
        // Initialize the program-wide proof counter (one-time setup, V1)
        0 => process_init_counter(program_id, accounts, instruction_data),
        // Store a TEE proof record, V1
        1 => process_store_proof(program_id, accounts, instruction_data),
        // Store a TEE proof record, V2 (flexible secp ix, msg hash binding, sharded counter)
        2 => process_store_proof_v2(program_id, accounts, instruction_data),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}
