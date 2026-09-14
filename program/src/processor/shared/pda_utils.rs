extern crate alloc;

use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::rent::Rent,
    ProgramResult,
};
use pinocchio_system::instructions::{Allocate, Assign, CreateAccount, Transfer};
use solana_program::pubkey::Pubkey as SolanaPubkey;

use crate::{
    constants::{
        PROOF_COUNTER_DAY_SEED, PROOF_COUNTER_SEED, TEE_PROOF_SEED, TEE_PROOF_V2_SEED,
        VERIFIER_REGISTRY_SEED,
    },
    error::RedpillVerifierError,
};

/// Derive and return the PDA for a TeeProof account.
pub fn find_tee_proof_pda(program_id: &Pubkey, quote_hash: &[u8; 32]) -> (SolanaPubkey, u8) {
    SolanaPubkey::find_program_address(
        &[TEE_PROOF_SEED, quote_hash.as_ref()],
        &SolanaPubkey::from(*program_id),
    )
}

/// Derive and return the PDA for the ProofCounter account.
pub fn find_proof_counter_pda(program_id: &Pubkey) -> (SolanaPubkey, u8) {
    SolanaPubkey::find_program_address(&[PROOF_COUNTER_SEED], &SolanaPubkey::from(*program_id))
}

/// Derive the PDA for a TeeProofV2 account using `signed_payload_hash` as seed.
pub fn find_tee_proof_v2_pda(
    program_id: &Pubkey,
    signed_payload_hash: &[u8; 32],
) -> (SolanaPubkey, u8) {
    SolanaPubkey::find_program_address(
        &[TEE_PROOF_V2_SEED, signed_payload_hash.as_ref()],
        &SolanaPubkey::from(*program_id),
    )
}

/// Derive the PDA for a sharded day counter: ["proof_counter_day", day_tag_bytes].
/// `day_tag` is the YYYYMMDD string encoded as UTF-8 bytes (8 bytes).
pub fn find_proof_counter_day_pda(program_id: &Pubkey, day_tag: &[u8; 8]) -> (SolanaPubkey, u8) {
    SolanaPubkey::find_program_address(
        &[PROOF_COUNTER_DAY_SEED, day_tag.as_ref()],
        &SolanaPubkey::from(*program_id),
    )
}

/// Derive the PDA for a VerifierRegistry entry using the verifier's pubkey.
pub fn find_verifier_registry_pda(
    program_id: &Pubkey,
    verifier_pubkey: &Pubkey,
) -> (SolanaPubkey, u8) {
    SolanaPubkey::find_program_address(
        &[VERIFIER_REGISTRY_SEED, verifier_pubkey.as_ref()],
        &SolanaPubkey::from(*program_id),
    )
}

/// Assert that `account` key matches the given Solana PDA.
pub fn assert_pda_key(account: &AccountInfo, expected: &SolanaPubkey) -> Result<(), ProgramError> {
    if account.key().as_ref() != expected.as_ref() {
        return Err(RedpillVerifierError::InvalidPda.into());
    }
    Ok(())
}

/// Create (or resize) a PDA account. Handles the case where lamports already exist.
pub fn create_pda_account<const N: usize>(
    payer: &AccountInfo,
    rent: &Rent,
    space: usize,
    owner: &Pubkey,
    new_pda_account: &AccountInfo,
    new_pda_signer_seeds: [Seed; N],
) -> ProgramResult {
    let signers = [Signer::from(&new_pda_signer_seeds)];
    let required_lamports = rent.minimum_balance(space).max(1);

    if new_pda_account.lamports() > 0 {
        let top_up = required_lamports.saturating_sub(new_pda_account.lamports());
        if top_up > 0 {
            Transfer {
                from: payer,
                to: new_pda_account,
                lamports: top_up,
            }
            .invoke()?;
        }
        Allocate {
            account: new_pda_account,
            space: space as u64,
        }
        .invoke_signed(&signers)?;
        Assign {
            account: new_pda_account,
            owner,
        }
        .invoke_signed(&signers)
    } else {
        CreateAccount {
            from: payer,
            to: new_pda_account,
            lamports: required_lamports,
            space: space as u64,
            owner,
        }
        .invoke_signed(&signers)
    }
}
