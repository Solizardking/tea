//! Solana interaction — builds and submits StoreProofV2 transactions,
//! and reads back TeeProofV2 PDAs.
//!
//! The StoreProofV2 instruction layout is defined in the on-chain program
//! (processor/store_proof_v2.rs). This file mirrors that layout exactly.
//!
//! Instruction data (after 1-byte discriminator = 2):
//!   secp_ix_index(2) + verification_level(1)
//!   + signed_payload_hash(32) + quote_hash(32) + request_hash(32)
//!   + response_hash(32) + model_hash(32) + attestation_report_hash(32) + compose_hash(32)
//!   + signing_address(20) + verifier_set_hash(32) + policy_version(4)
//!   + nonce(32) + expires_at(8) + day_tag(8)
//! Total = 332 bytes.

use std::str::FromStr;

use anyhow::{Context, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    sysvar,
    transaction::Transaction,
};
use tracing::warn;

use crate::{config::Config, AppState};

// Program seeds — must match constants.rs in the on-chain program
const TEE_PROOF_V2_SEED: &[u8] = b"tee_proof_v2";
const PROOF_COUNTER_DAY_SEED: &[u8] = b"proof_counter_day";

// Instruction discriminator for StoreProofV2
const STORE_PROOF_V2_DISCRIMINATOR: u8 = 2;
const VERIFICATION_LEVEL_TEE_SIG_ONCHAIN: u8 = 1;

// ---------------------------------------------------------------------------
// PDA derivation
// ---------------------------------------------------------------------------

pub fn find_tee_proof_v2_pda(program_id: &Pubkey, signed_payload_hash: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[TEE_PROOF_V2_SEED, signed_payload_hash.as_ref()],
        program_id,
    )
}

pub fn find_proof_counter_day_pda(program_id: &Pubkey, day_tag: &[u8; 8]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[PROOF_COUNTER_DAY_SEED, day_tag.as_ref()], program_id)
}

fn day_tag_today() -> [u8; 8] {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // convert epoch seconds → YYYYMMDD (simplified, no leap seconds)
    let days_since_epoch = secs / 86_400;
    let mut year = 1970u32;
    let mut days = days_since_epoch as u32;
    loop {
        let dy = if is_leap(year) { 366 } else { 365 };
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let months = [
        31,
        if is_leap(year) { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1u32;
    for &m in &months {
        if days < m {
            break;
        }
        days -= m;
        month += 1;
    }
    let day = days + 1;
    let mut tag = [b'0'; 8];
    let s = format!("{year:04}{month:02}{day:02}");
    tag.copy_from_slice(s.as_bytes());
    tag
}

fn is_leap(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ---------------------------------------------------------------------------
// Build StoreProofV2 instruction
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn build_store_proof_v2_ix(
    program_id: &Pubkey,
    payer: &Pubkey,
    signed_payload_hash: &[u8; 32],
    quote_hash: &[u8; 32],
    request_hash: &[u8; 32],
    response_hash: &[u8; 32],
    model_hash: &[u8; 32],
    attestation_report_hash: &[u8; 32],
    compose_hash: &[u8; 32],
    signing_address: &[u8; 20],
    verifier_set_hash: &[u8; 32],
    policy_version: u32,
    nonce: &[u8; 32],
    expires_at: i64,
    verification_level: u8,
    secp_ix_index: u16,
) -> Instruction {
    let day_tag = day_tag_today();
    let (proof_pda, _) = find_tee_proof_v2_pda(program_id, signed_payload_hash);
    let (counter_pda, _) = find_proof_counter_day_pda(program_id, &day_tag);

    // Build instruction data
    let mut data = Vec::with_capacity(332);
    data.push(STORE_PROOF_V2_DISCRIMINATOR);
    data.extend_from_slice(&secp_ix_index.to_le_bytes());
    data.push(verification_level);
    data.extend_from_slice(signed_payload_hash);
    data.extend_from_slice(quote_hash);
    data.extend_from_slice(request_hash);
    data.extend_from_slice(response_hash);
    data.extend_from_slice(model_hash);
    data.extend_from_slice(attestation_report_hash);
    data.extend_from_slice(compose_hash);
    data.extend_from_slice(signing_address);
    data.extend_from_slice(verifier_set_hash);
    data.extend_from_slice(&policy_version.to_le_bytes());
    data.extend_from_slice(nonce);
    data.extend_from_slice(&expires_at.to_le_bytes());
    data.extend_from_slice(&day_tag);

    let accounts = vec![
        AccountMeta::new(*payer, true),
        AccountMeta::new(proof_pda, false),
        AccountMeta::new(counter_pda, false),
        AccountMeta::new_readonly(solana_sdk::system_program::ID, false),
        AccountMeta::new_readonly(sysvar::instructions::ID, false),
    ];

    Instruction {
        program_id: *program_id,
        accounts,
        data,
    }
}

fn build_secp256k1_ix(
    signing_address: &[u8; 20],
    signed_payload_hash: &[u8; 32],
    signature: &[u8; 65],
) -> Result<Instruction> {
    let secp_program_id = Pubkey::from_str("KeccakSecp256k11111111111111111111111111111")
        .context("parsing secp256k1 program ID")?;

    let sig_count = 1u8;
    let header_size = 1usize + 11usize;
    let sig_offset = header_size;
    let addr_offset = sig_offset + 65;
    let msg_offset = addr_offset + 20;
    let msg_size = 32usize;

    let mut data = vec![0u8; msg_offset + msg_size];
    data[0] = sig_count;
    data[1..3].copy_from_slice(&(sig_offset as u16).to_le_bytes());
    data[3] = 0;
    data[4..6].copy_from_slice(&(addr_offset as u16).to_le_bytes());
    data[6] = 0;
    data[7..9].copy_from_slice(&(msg_offset as u16).to_le_bytes());
    data[9..11].copy_from_slice(&(msg_size as u16).to_le_bytes());
    data[11] = 0;

    data[sig_offset..sig_offset + 65].copy_from_slice(signature);
    data[addr_offset..addr_offset + 20].copy_from_slice(signing_address);
    data[msg_offset..msg_offset + 32].copy_from_slice(signed_payload_hash);

    Ok(Instruction {
        program_id: secp_program_id,
        accounts: vec![],
        data,
    })
}

// ---------------------------------------------------------------------------
// Anchor a proof to Solana
// ---------------------------------------------------------------------------

/// Returns (proof_address_base58, tx_signature_base58).
///
/// When no keypair is configured this does **not** submit a transaction.
/// The computed PDA is still returned and the skip is logged at warn so
/// operators notice they are running without on-chain settlement.
pub async fn anchor_proof_v2(
    state: &AppState,
    signed_payload_hash: &[u8; 32],
    request_hash: &[u8; 32],
    response_hash: &[u8; 32],
    model_hash: &[u8; 32],
    nonce: &[u8; 32],
    quote_hash: Option<&[u8; 32]>,
    attestation_report_hash: Option<&[u8; 32]>,
    signature: Option<&[u8; 65]>,
) -> Result<(String, String)> {
    let quote_hash = match quote_hash {
        Some(h) if *h != [0u8; 32] => *h,
        Some(_) => {
            warn!("quote_hash is 32 zero bytes — TeeProofV2 will store an empty quote digest");
            [0u8; 32]
        }
        None => {
            warn!("quote_hash missing — TeeProofV2 will store an empty quote digest");
            [0u8; 32]
        }
    };
    let attestation_report_hash = match attestation_report_hash {
        Some(h) if *h != [0u8; 32] => *h,
        Some(_) => {
            warn!(
                "attestation_report_hash is 32 zero bytes — on-chain field will not bind evidence"
            );
            [0u8; 32]
        }
        None => {
            warn!("attestation_report_hash missing — on-chain field will be zeros");
            [0u8; 32]
        }
    };

    let keypair = match state.config.has_solana_keypair() {
        true => state.config.load_solana_keypair("StoreProofV2")?,
        false => {
            let program_id = Pubkey::from_str(&state.config.solana_program_id)?;
            let (pda, _) = find_tee_proof_v2_pda(&program_id, signed_payload_hash);
            warn!(
                pda = %pda,
                signed_payload_hash = %hex::encode(signed_payload_hash),
                "SOLANA_KEYPAIR_PATH / SOLANA_KEYPAIR_JSON unset — TeeProofV2 not submitted (PDA computed only)"
            );
            return Ok((pda.to_string(), String::new()));
        }
    };

    submit_store_proof_v2(
        &state.config,
        &keypair,
        signed_payload_hash,
        &quote_hash,
        request_hash,
        response_hash,
        model_hash,
        &attestation_report_hash,
        &[0u8; 32], // compose_hash — GPU/image stay off-chain; digest only
        state.signing_key.eth_address(),
        &[0u8; 32], // verifier_set_hash
        0,          // policy_version
        nonce,
        signature,
    )
    .await
}

async fn submit_store_proof_v2(
    config: &Config,
    keypair: &Keypair,
    signed_payload_hash: &[u8; 32],
    quote_hash: &[u8; 32],
    request_hash: &[u8; 32],
    response_hash: &[u8; 32],
    model_hash: &[u8; 32],
    attestation_report_hash: &[u8; 32],
    compose_hash: &[u8; 32],
    signing_address: &[u8; 20],
    verifier_set_hash: &[u8; 32],
    policy_version: u32,
    nonce: &[u8; 32],
    signature: Option<&[u8; 65]>,
) -> Result<(String, String)> {
    let program_id = Pubkey::from_str(&config.solana_program_id).context("parsing program ID")?;

    let expires_at = if config.proof_ttl_secs == 0 {
        0i64
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
            + config.proof_ttl_secs as i64
    };

    let secp_ix = if config.verification_level >= VERIFICATION_LEVEL_TEE_SIG_ONCHAIN {
        Some(build_secp256k1_ix(
            signing_address,
            signed_payload_hash,
            signature.context("verification_level >= 1 requires a secp256k1 signature")?,
        )?)
    } else {
        None
    };

    let ix = build_store_proof_v2_ix(
        &program_id,
        &keypair.pubkey(),
        signed_payload_hash,
        quote_hash,
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
        config.verification_level,
        0, // secp_ix_index — no secp when verification_level = 0
    );

    let (proof_pda, _) = find_tee_proof_v2_pda(&program_id, signed_payload_hash);

    // Use a blocking task for the synchronous RPC client
    let rpc_url = config.solana_rpc_url.clone();
    let payer = keypair.insecure_clone();
    let sig = tokio::task::spawn_blocking(move || -> Result<Signature> {
        let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

        // Check if proof already exists
        if client.get_account(&proof_pda).is_ok() {
            warn!(pda = %proof_pda, "proof already anchored, skipping");
            return Ok(Signature::default());
        }

        let recent_blockhash = client.get_latest_blockhash().context("getting blockhash")?;

        let mut instructions = Vec::with_capacity(2);
        if let Some(secp_ix) = secp_ix {
            instructions.push(secp_ix);
        }
        instructions.push(ix);

        let tx = Transaction::new_signed_with_payer(
            &instructions,
            Some(&payer.pubkey()),
            &[&payer],
            recent_blockhash,
        );

        client
            .send_and_confirm_transaction(&tx)
            .context("sending StoreProofV2 tx")
    })
    .await
    .context("spawn_blocking")??;

    if sig == Signature::default() {
        warn!(pda = %proof_pda, "StoreProofV2 skipped because the PDA already exists");
    }

    Ok((proof_pda.to_string(), sig.to_string()))
}

// ---------------------------------------------------------------------------
// Lookup a proof
// ---------------------------------------------------------------------------

/// Returns a JSON Value of the proof account data, or null if not found.
pub async fn lookup_proof_v2(
    config: &Config,
    signed_payload_hash: &[u8; 32],
) -> Result<Option<serde_json::Value>> {
    let program_id = Pubkey::from_str(&config.solana_program_id)?;
    let (pda, _) = find_tee_proof_v2_pda(&program_id, signed_payload_hash);
    let rpc_url = config.solana_rpc_url.clone();

    let result = tokio::task::spawn_blocking(move || {
        let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());
        client.get_account(&pda).ok().map(|acc| {
            serde_json::json!({
                "address": pda.to_string(),
                "data_hex": hex::encode(&acc.data),
                "lamports": acc.lamports,
                "owner": acc.owner.to_string(),
            })
        })
    })
    .await
    .context("spawn_blocking")?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_proof_v2_ix_embeds_nonzero_attestation_hash() {
        let program_id = Pubkey::new_unique();
        let payer = Pubkey::new_unique();
        let mut attestation_report_hash = [0u8; 32];
        attestation_report_hash[0] = 0xAB;
        attestation_report_hash[31] = 0xCD;
        let mut quote_hash = [0u8; 32];
        quote_hash[0] = 0x11;

        let ix = build_store_proof_v2_ix(
            &program_id,
            &payer,
            &[0x22; 32],
            &quote_hash,
            &[0x33; 32],
            &[0x44; 32],
            &[0x55; 32],
            &attestation_report_hash,
            &[0u8; 32],
            &[0xAAu8; 20],
            &[0u8; 32],
            0,
            &[0x66; 32],
            0,
            0,
            0,
        );

        // disc(1) + secp_ix(2) + level(1) + signed(32) + quote(32) + req(32)
        // + res(32) + model(32) + attestation(32)
        let att_off = 1 + 2 + 1 + 32 + 32 + 32 + 32 + 32;
        assert_eq!(&ix.data[att_off..att_off + 32], &attestation_report_hash);
        let quote_off = 1 + 2 + 1 + 32;
        assert_eq!(&ix.data[quote_off..quote_off + 32], &quote_hash);
        assert_ne!(&ix.data[att_off..att_off + 32], &[0u8; 32]);
    }
}
