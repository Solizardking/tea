/// store_proof_v2 — V2 TEE attestation proof registry entry.
///
/// Improvements over V1:
///   1. Flexible secp256k1 ix index — caller provides `secp_ix_index` so
///      Compute Budget instructions can appear before the secp256k1 precompile.
///   2. Message hash binding — the secp256k1 instruction MUST have signed
///      `signed_payload_hash`, not just any message. V1 only checked the eth
///      address; V2 checks address AND message hash.
///   3. `verification_level: u8` + `status: u8` replace the V1 `is_valid: bool`.
///   4. Sharded day counter — bumps `proof_counter_day/YYYYMMDD` instead of
///      the global `proof_counter` PDA, eliminating write-lock serialisation.
///
/// Accounts:
///   0. [signer, writable] payer / submitter
///   1. [writable]         proof_account    (PDA: ["tee_proof_v2", signed_payload_hash])
///   2. [writable]         day_counter      (PDA: ["proof_counter_day", day_tag])
///   3. []                 system_program
///   4. []                 instructions_sysvar
///
/// Instruction data (after 1-byte discriminator, total = 3 + 32*7 + 20 + 32 + 4 + 32 + 8 + 8 + 2 = 333 bytes):
///   [0..2]     secp_ix_index:        u16 LE  — which ix in the tx has the secp256k1 precompile
///   [2]        verification_level:   u8
///   [3..35]    signed_payload_hash:  [u8; 32]
///   [35..67]   quote_hash:           [u8; 32]
///   [67..99]   request_hash:         [u8; 32]
///   [99..131]  response_hash:        [u8; 32]
///   [131..163] model_hash:           [u8; 32]
///   [163..195] attestation_report_hash: [u8; 32]
///   [195..227] compose_hash:         [u8; 32]
///   [227..247] signing_address:      [u8; 20]
///   [247..279] verifier_set_hash:    [u8; 32]
///   [279..283] policy_version:       u32 LE
///   [283..315] nonce:                [u8; 32]
///   [315..323] expires_at:           i64 LE
///   [323..331] day_tag:              [u8; 8]  ("YYYYMMDD" ASCII for PDA seed)
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
    constants::{
        PROOF_COUNTER_DAY_DISCRIMINATOR, PROOF_COUNTER_DAY_SIZE, TEE_PROOF_V2_SEED,
        TEE_PROOF_V2_SIZE, VERIFICATION_LEVEL_TEE_SIG_ONCHAIN, VERIFICATION_LEVEL_ZK_AUTHORIZED,
    },
    error::RedpillVerifierError,
    processor::shared::{
        account_checks::{assert_signer, assert_uninitialized, assert_writable},
        pda_utils::{
            assert_pda_key, create_pda_account, find_proof_counter_day_pda, find_tee_proof_v2_pda,
        },
    },
    state::TeeProofV2,
};

const IX_DATA_MIN_LEN: usize = 331; // secp_ix_index(2) + vl(1) + 7*32 + 20 + 32 + 4 + 32 + 8 + 8

#[inline(always)]
pub fn process_store_proof_v2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // -----------------------------------------------------------------------
    // 1. Parse instruction data
    // -----------------------------------------------------------------------
    if data.len() < IX_DATA_MIN_LEN {
        return Err(ProgramError::InvalidInstructionData);
    }

    let secp_ix_index = u16::from_le_bytes([data[0], data[1]]);
    let verification_level = data[2];

    let mut o = 3usize;
    macro_rules! read32 {
        () => {{
            let v: [u8; 32] = data[o..o + 32].try_into().unwrap();
            o += 32;
            v
        }};
    }

    let signed_payload_hash = read32!();
    let quote_hash = read32!();
    let request_hash = read32!();
    let response_hash = read32!();
    let model_hash = read32!();
    let attestation_report_hash = read32!();
    let compose_hash = read32!();

    let signing_address: [u8; 20] = data[o..o + 20].try_into().unwrap();
    o += 20;
    let verifier_set_hash = read32!();
    let policy_version = u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    o += 4;
    let nonce = read32!();
    let expires_at = i64::from_le_bytes(data[o..o + 8].try_into().unwrap());
    o += 8;
    let day_tag: [u8; 8] = data[o..o + 8].try_into().unwrap();

    if verification_level > VERIFICATION_LEVEL_ZK_AUTHORIZED {
        return Err(RedpillVerifierError::InvalidVerificationLevel.into());
    }

    validate_day_tag(&day_tag)?;

    // -----------------------------------------------------------------------
    // 2. Account destructuring
    // -----------------------------------------------------------------------
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let payer = &accounts[0];
    let proof_account = &accounts[1];
    let day_counter = &accounts[2];
    // accounts[3] = system_program
    let ix_sysvar = &accounts[4];

    // -----------------------------------------------------------------------
    // 3. Security checks
    // -----------------------------------------------------------------------
    assert_signer(payer)?;
    assert_writable(proof_account)?;
    assert_writable(day_counter)?;
    assert_uninitialized(proof_account)?;

    let (proof_pda, proof_bump) = find_tee_proof_v2_pda(program_id, &signed_payload_hash);
    assert_pda_key(proof_account, &proof_pda)?;

    let (counter_pda, _) = find_proof_counter_day_pda(program_id, &day_tag);
    assert_pda_key(day_counter, &counter_pda)?;

    // -----------------------------------------------------------------------
    // 4. Clock — expiry guard and timestamp
    // -----------------------------------------------------------------------
    let clock = Clock::get()?;

    if expires_at != 0 && clock.unix_timestamp > expires_at {
        return Err(RedpillVerifierError::ProofExpired.into());
    }

    // -----------------------------------------------------------------------
    // 5. secp256k1 verification via instructions sysvar
    //    When verification_level >= VERIFICATION_LEVEL_TEE_SIG_ONCHAIN,
    //    the caller MUST include a secp256k1 instruction at secp_ix_index.
    //    We verify:  (a) signing_address matches, (b) message == signed_payload_hash
    // -----------------------------------------------------------------------
    if verification_level >= VERIFICATION_LEVEL_TEE_SIG_ONCHAIN {
        verify_secp256k1_v2(
            ix_sysvar,
            secp_ix_index,
            &signing_address,
            &signed_payload_hash,
        )?;
    }

    // -----------------------------------------------------------------------
    // 6. Allocate TeeProofV2 PDA
    // -----------------------------------------------------------------------
    let rent = Rent::get()?;

    let bump_arr = [proof_bump];
    let signer_seeds = [
        Seed::from(TEE_PROOF_V2_SEED),
        Seed::from(signed_payload_hash.as_ref()),
        Seed::from(&bump_arr),
    ];

    create_pda_account(
        payer,
        &rent,
        TEE_PROOF_V2_SIZE,
        program_id,
        proof_account,
        signer_seeds,
    )?;

    // -----------------------------------------------------------------------
    // 7. Write TeeProofV2 data
    // -----------------------------------------------------------------------
    let proof = TeeProofV2 {
        version: 2,
        verification_level,
        status: 0, // PROOF_STATUS_ACTIVE
        quote_hash,
        signed_payload_hash,
        request_hash,
        response_hash,
        model_hash,
        attestation_report_hash,
        compose_hash,
        signing_address,
        verifier_set_hash,
        policy_version,
        nonce,
        expires_at,
        timestamp: clock.unix_timestamp,
        slot: clock.slot,
        submitter: *payer.key(),
    };
    let proof_bytes = proof.to_bytes();
    {
        let mut d = proof_account.try_borrow_mut_data()?;
        d[..TEE_PROOF_V2_SIZE].copy_from_slice(&proof_bytes);
    }

    // -----------------------------------------------------------------------
    // 8. Bump sharded day counter (create if first proof this day)
    // -----------------------------------------------------------------------
    bump_day_counter(
        payer,
        day_counter,
        &counter_pda,
        program_id,
        &day_tag,
        &rent,
    )?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Sharded day counter helpers
// ---------------------------------------------------------------------------

fn bump_day_counter(
    payer: &AccountInfo,
    counter_account: &AccountInfo,
    _counter_pda: &solana_program::pubkey::Pubkey,
    program_id: &Pubkey,
    day_tag: &[u8; 8],
    rent: &Rent,
) -> ProgramResult {
    if counter_account.lamports() == 0 {
        // First proof this day: create the counter PDA.
        let (_, bump) = find_proof_counter_day_pda(program_id, day_tag);
        let bump_arr = [bump];
        let signer_seeds = [
            Seed::from(crate::constants::PROOF_COUNTER_DAY_SEED),
            Seed::from(day_tag.as_ref()),
            Seed::from(&bump_arr),
        ];
        create_pda_account(
            payer,
            rent,
            PROOF_COUNTER_DAY_SIZE,
            program_id,
            counter_account,
            signer_seeds,
        )?;
        let mut d = counter_account.try_borrow_mut_data()?;
        d[0] = PROOF_COUNTER_DAY_DISCRIMINATOR;
        d[1..9].copy_from_slice(day_tag.as_ref());
        // count starts at 1
        d[9..17].copy_from_slice(&1u64.to_le_bytes());
    } else {
        let mut d = counter_account.try_borrow_mut_data()?;
        if d.len() < PROOF_COUNTER_DAY_SIZE {
            return Err(RedpillVerifierError::AccountDataTooSmall.into());
        }
        let count = u64::from_le_bytes(d[9..17].try_into().unwrap());
        let new_count = count
            .checked_add(1)
            .ok_or(RedpillVerifierError::ArithmeticOverflow)?;
        d[9..17].copy_from_slice(&new_count.to_le_bytes());
    }
    Ok(())
}

fn validate_day_tag(day_tag: &[u8; 8]) -> ProgramResult {
    if !day_tag.iter().all(u8::is_ascii_digit) {
        return Err(RedpillVerifierError::InvalidDayTag.into());
    }

    let month = (day_tag[4] - b'0') * 10 + (day_tag[5] - b'0');
    let day = (day_tag[6] - b'0') * 10 + (day_tag[7] - b'0');
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return Err(RedpillVerifierError::InvalidDayTag.into());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Instructions sysvar — V2 secp256k1 check with flexible index + msg binding
// ---------------------------------------------------------------------------
//
// secp256k1 instruction data layout (Solana native precompile):
//   [0]        sig_count (u8)
//   per sig (11-byte header starting at data[1]):
//     [0..2]   sig_offset    (u16 LE) — byte offset into ix data of the 65-byte sig
//     [2]      sig_ix        (u8)     — which instruction contains the sig (0xFF = same)
//     [3..5]   eth_addr_off  (u16 LE) — byte offset into ix data of the 20-byte eth addr
//     [5]      eth_addr_ix   (u8)
//     [6..8]   msg_offset    (u16 LE) — byte offset into ix data of the message
//     [8..10]  msg_len       (u16 LE) — byte length of the message
//     [10]     msg_ix        (u8)
//   followed by: sig(65) | eth_addr(20) | message(msg_len)

fn verify_secp256k1_v2(
    ix_sysvar: &AccountInfo,
    secp_ix_index: u16,
    expected_eth_addr: &[u8; 20],
    expected_msg_hash: &[u8; 32],
) -> Result<(), ProgramError> {
    let instructions = Instructions::try_from(ix_sysvar)?;
    let idx = secp_ix_index as usize;
    let current_index = instructions.load_current_index();

    if secp_ix_index >= current_index {
        return Err(RedpillVerifierError::SecpIxMustPrecedeCurrentIx.into());
    }

    let secp_ix = instructions
        .load_instruction_at(idx)
        .map_err(|_| RedpillVerifierError::SecpIxNotFound)?;

    if secp_ix.get_program_id().as_ref() != solana_program::secp256k1_program::ID.as_ref() {
        return Err(RedpillVerifierError::SecpIxWrongProgram.into());
    }

    let ix_data = secp_ix.get_instruction_data();

    if ix_data.len() < 12 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // Must have at least one signature
    if ix_data[0] == 0 {
        return Err(RedpillVerifierError::SecpIxNotFound.into());
    }

    // Parse first sig header (starts at ix_data[1])
    let hdr = &ix_data[1..12];
    let sig_off = u16::from_le_bytes([hdr[0], hdr[1]]) as usize;
    let sig_ix = hdr[2];
    let eth_addr_off = u16::from_le_bytes([hdr[3], hdr[4]]) as usize;
    let eth_addr_ix = hdr[5];
    let msg_offset = u16::from_le_bytes([hdr[6], hdr[7]]) as usize;
    let msg_len = u16::from_le_bytes([hdr[8], hdr[9]]) as usize;
    let msg_ix = hdr[10];

    let secp_ix_index_u8 =
        u8::try_from(secp_ix_index).map_err(|_| ProgramError::InvalidInstructionData)?;
    if sig_ix != secp_ix_index_u8 || eth_addr_ix != secp_ix_index_u8 || msg_ix != secp_ix_index_u8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    if ix_data.len() < sig_off + 65 {
        return Err(ProgramError::InvalidInstructionData);
    }

    // (a) Verify Ethereum address
    if ix_data.len() < eth_addr_off + 20 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let recovered_addr = &ix_data[eth_addr_off..eth_addr_off + 20];
    if recovered_addr != expected_eth_addr.as_ref() {
        return Err(RedpillVerifierError::SecpAddressMismatch.into());
    }

    // (b) Verify signed message is exactly signed_payload_hash (32 bytes)
    if msg_len != 32 {
        return Err(RedpillVerifierError::SecpMessageHashMismatch.into());
    }
    if ix_data.len() < msg_offset + 32 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let signed_msg = &ix_data[msg_offset..msg_offset + 32];
    if signed_msg != expected_msg_hash.as_ref() {
        return Err(RedpillVerifierError::SecpMessageHashMismatch.into());
    }

    Ok(())
}
