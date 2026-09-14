extern crate alloc;

use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};

use crate::{
    constants::{PROOF_COUNTER_SEED, PROOF_COUNTER_SIZE},
    processor::shared::{
        account_checks::{assert_signer, assert_uninitialized, assert_writable},
        pda_utils::{assert_pda_key, create_pda_account, find_proof_counter_pda},
    },
    state::ProofCounter,
};

/// Initialize the program-wide ProofCounter PDA.
///
/// Accounts:
///   0. [signer, writable] payer
///   1. [writable]         counter_account (PDA: ["proof_counter"])
///   2. []                 system_program
#[inline(always)]
pub fn process_init_counter(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _data: &[u8],
) -> ProgramResult {
    let [payer, counter_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    assert_signer(payer)?;
    assert_writable(counter_account)?;
    assert_uninitialized(counter_account)?;

    let (counter_pda, bump) = find_proof_counter_pda(program_id);
    assert_pda_key(counter_account, &counter_pda)?;

    let rent = Rent::get()?;
    let bump_arr = [bump];
    let signer_seeds = [Seed::from(PROOF_COUNTER_SEED), Seed::from(&bump_arr)];

    create_pda_account(
        payer,
        &rent,
        PROOF_COUNTER_SIZE,
        program_id,
        counter_account,
        signer_seeds,
    )?;

    let initial = ProofCounter { count: 0 };
    let bytes = initial.to_bytes();
    let mut data = counter_account.try_borrow_mut_data()?;
    data[..PROOF_COUNTER_SIZE].copy_from_slice(&bytes);

    Ok(())
}
