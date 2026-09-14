/// store_proof — stores a TEE attestation proof record on-chain.
///
/// Solana SVM equivalent of RedpillProofStore.verifyAndStore().
///
/// TDX quote verification is performed off-chain (Phala/Automata cloud APIs or
/// dstack-verifier Docker). The `is_valid` field is submitted by the caller.
/// When `verify_sig = 1`, the caller must also include a native secp256k1
/// instruction at index 0 of the transaction; the program validates it by
/// reading the Instructions sysvar directly.
///
/// Accounts:
///   0. [signer, writable] payer / submitter
///   1. [writable]         proof_account  (PDA: ["tee_proof", quote_hash])
///   2. [writable]         counter_account (PDA: ["proof_counter"])
///   3. []                 system_program
///   4. []                 instructions_sysvar  (only when verify_sig = 1)
///
/// Instruction data (after 1-byte discriminator):
///   [0..32]  quote_hash:      [u8; 32]
///   [32..52] signing_address: [u8; 20]
///   [52]     is_valid:        u8
///   [53]     verify_sig:      u8
extern crate alloc;

use pinocchio::{
    account_info::AccountInfo,
    instruction::Seed,
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{clock::Clock, instructions::Instructions, rent::Rent, Sysvar},
    ProgramResult,
};

use crate::{
    constants::{TEE_PROOF_SEED, TEE_PROOF_SIZE},
    error::RedpillVerifierError,
    processor::shared::{
        account_checks::{assert_signer, assert_uninitialized, assert_writable},
        pda_utils::{
            assert_pda_key, create_pda_account, find_proof_counter_pda, find_tee_proof_pda,
        },
    },
    state::{ProofCounter, TeeProof},
};

#[inline(always)]
pub fn process_store_proof(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // -----------------------------------------------------------------------
    // 1. Parse instruction data
    // -----------------------------------------------------------------------
    if data.len() < 54 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let quote_hash: [u8; 32] = data[0..32].try_into().unwrap();
    let signing_address: [u8; 20] = data[32..52].try_into().unwrap();
    let is_valid = data[52] != 0;
    let verify_sig = data[53] != 0;

    // -----------------------------------------------------------------------
    // 2. Account destructuring
    // -----------------------------------------------------------------------
    let min_accounts = if verify_sig { 5 } else { 4 };
    if accounts.len() < min_accounts {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let payer = &accounts[0];
    let proof_account = &accounts[1];
    let counter_account = &accounts[2];
    // accounts[3] = system_program (consumed by create_pda_account CPI)
    // accounts[4] = instructions_sysvar (optional)

    // -----------------------------------------------------------------------
    // 3. Security checks
    // -----------------------------------------------------------------------
    assert_signer(payer)?;
    assert_writable(proof_account)?;
    assert_writable(counter_account)?;
    assert_uninitialized(proof_account)?;

    let (proof_pda, proof_bump) = find_tee_proof_pda(program_id, &quote_hash);
    assert_pda_key(proof_account, &proof_pda)?;

    let (counter_pda, _) = find_proof_counter_pda(program_id);
    assert_pda_key(counter_account, &counter_pda)?;

    // -----------------------------------------------------------------------
    // 4. Optional secp256k1 signature check via instructions sysvar
    // -----------------------------------------------------------------------
    if verify_sig {
        let ix_sysvar = &accounts[4];
        verify_secp256k1_present(ix_sysvar, &signing_address)?;
    }

    // -----------------------------------------------------------------------
    // 5. Allocate TeeProof PDA
    // -----------------------------------------------------------------------
    let rent = Rent::get()?;
    let clock = Clock::get()?;

    let bump_arr = [proof_bump];
    let signer_seeds = [
        Seed::from(TEE_PROOF_SEED),
        Seed::from(quote_hash.as_ref()),
        Seed::from(&bump_arr),
    ];

    create_pda_account(
        payer,
        &rent,
        TEE_PROOF_SIZE,
        program_id,
        proof_account,
        signer_seeds,
    )?;

    // -----------------------------------------------------------------------
    // 6. Write TeeProof data
    // -----------------------------------------------------------------------
    let proof = TeeProof {
        quote_hash,
        signing_address,
        is_valid,
        timestamp: clock.unix_timestamp,
        slot: clock.slot,
        submitter: *payer.key(),
    };
    let proof_bytes = proof.to_bytes();
    {
        let mut proof_data = proof_account.try_borrow_mut_data()?;
        proof_data[..TEE_PROOF_SIZE].copy_from_slice(&proof_bytes);
    }

    // -----------------------------------------------------------------------
    // 7. Increment proof counter
    // -----------------------------------------------------------------------
    {
        let mut counter_data = counter_account.try_borrow_mut_data()?;
        let mut counter = ProofCounter::try_from_bytes(&counter_data)?;
        counter.count = counter
            .count
            .checked_add(1)
            .ok_or(RedpillVerifierError::ArithmeticOverflow)?;
        let updated = counter.to_bytes();
        counter_data[..updated.len()].copy_from_slice(&updated);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Instructions sysvar: verify secp256k1 instruction at index 0
// ---------------------------------------------------------------------------
//
// The Instructions sysvar stores the serialized instructions in the current
// transaction. Layout (raw account data):
//   [0..2]   num_ixs (u16 LE)
//   [2..2+num_ixs*2]  ix_offsets (u16 LE per entry)
//   [offsets[i]..]    serialized instruction i
//
// Each serialized instruction:
//   [0..2]   program_id_index (u16 LE) into the tx account keys
//   [2..4]   num_accounts (u16 LE)
//   [4..4+n*33]  accounts (flags u8 + pubkey [u8;32] each)
//   [4+n*33..4+n*33+2]  data_len (u16 LE)
//   [..]     instruction data
//
// secp256k1 instruction data:
//   [0]  sig_count (u8)
//   per sig, 11-byte header:
//     [0..2]  sig_offset (u16 LE) — into this instruction data
//     [2]     sig_ix
//     [3..5]  eth_addr_offset (u16 LE) — into this instruction data
//     [5]     eth_addr_ix
//     [6..8]  msg_offset (u16 LE)
//     [8..10] msg_len (u16 LE)
//     [10]    msg_ix
//   then: sig(65) + eth_addr(20) + message

fn verify_secp256k1_present(
    ix_sysvar: &AccountInfo,
    expected_eth_addr: &[u8; 20],
) -> Result<(), ProgramError> {
    let instructions = Instructions::try_from(ix_sysvar)?;
    if instructions.load_current_index() == 0 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let secp_ix = instructions.load_instruction_at(0)?;
    if secp_ix.get_program_id().as_ref() != solana_program::secp256k1_program::ID.as_ref() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let ix_data = secp_ix.get_instruction_data();
    if ix_data.len() < 12 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // sig_count must be at least 1
    if ix_data[0] == 0 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // V1's helper requires instruction 0. Ensure the secp precompile verified
    // address bytes from the same instruction data that we inspect below.
    if ix_data[3] != 0 || ix_data[6] != 0 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // eth_addr_offset is bytes 4..6 of the full instruction data.
    let eth_addr_off = u16::from_le_bytes([ix_data[4], ix_data[5]]) as usize;

    if ix_data.len() < eth_addr_off + 20 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let recovered = &ix_data[eth_addr_off..eth_addr_off + 20];
    if recovered != expected_eth_addr.as_ref() {
        return Err(RedpillVerifierError::InvalidSigningAddress.into());
    }

    Ok(())
}
