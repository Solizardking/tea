//! Solana Attestation Service (SAS) integration.
//!
//! # What this does
//!
//! Every time a wallet makes a verified TEE inference through the gateway,
//! we increment an in-memory counter for that wallet. Once the counter reaches
//! `SAS_PROOF_THRESHOLD`, we:
//!
//!   1. Issue a SAS Credential + Schema (one-time per gateway deployment) via
//!      `init_credential_and_schema()`.
//!   2. Issue a SAS Attestation to the wallet (a Token-2022 non-transferable NFT)
//!      containing the proof count, TEE address, and most recent payload hash.
//!
//! # On-chain layout
//!
//! Credential PDA:  ["credential", authority_pubkey, credential_name]
//! Schema PDA:      ["schema", credential_pda, schema_name, schema_version]
//! Attestation PDA: ["attestation", credential_pda, schema_pda, wallet_pubkey]
//!
//! Schema fields (all Borsh-serialized strings, layout = [12, 12, 12]):
//!   0: proof_count        — decimal string, e.g. "42"
//!   1: tee_signing_address — hex, e.g. "0xabc…"
//!   2: last_proof_hash    — hex of signedPayloadHash

use std::{
    str::FromStr,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use solana_attestation_service_client::{
    accounts::Attestation,
    instructions::{
        CloseAttestationBuilder, CreateAttestationBuilder, CreateCredentialBuilder,
        CreateSchemaBuilder, CreateTokenizedAttestationBuilder, TokenizeSchemaBuilder,
    },
    programs::SOLANA_ATTESTATION_SERVICE_ID,
};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::Transaction,
};
use tracing::{info, warn};

use crate::config::Config;

// SAS type byte for String fields
const SAS_TYPE_STRING: u8 = 12;

// ---------------------------------------------------------------------------
// Attestation data stored in the SAS schema
// ---------------------------------------------------------------------------

/// The payload written into every SAS attestation issued by this gateway.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Serialize, Deserialize)]
pub struct ClawdTeeAttestationData {
    /// Number of verified TEE proofs this wallet has contributed to.
    pub proof_count: String,
    /// Ethereum-style address of the TEE signing key.
    pub tee_signing_address: String,
    /// Most recent signedPayloadHash (hex).
    pub last_proof_hash: String,
}

// ---------------------------------------------------------------------------
// Per-wallet proof tracker (in-memory)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct WalletProofRecord {
    pub count: u64,
    pub last_proof_hash: String,
}

/// Thread-safe map: wallet pubkey (base58) → accumulated proof record.
pub type ProofTracker = Arc<DashMap<String, WalletProofRecord>>;

pub fn new_proof_tracker() -> ProofTracker {
    Arc::new(DashMap::new())
}

/// Increment the proof counter for a wallet. Returns the new count.
pub fn record_proof(tracker: &ProofTracker, wallet: &str, payload_hash: &str) -> u64 {
    let mut entry = tracker.entry(wallet.to_string()).or_default();
    entry.count += 1;
    entry.last_proof_hash = payload_hash.to_string();
    entry.count
}

/// Read the current count for a wallet (0 if unknown).
pub fn proof_count(tracker: &ProofTracker, wallet: &str) -> u64 {
    tracker.get(wallet).map(|r| r.count).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// SAS state (credential + schema PDAs, cached after init)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SasState {
    pub credential_pda: Pubkey,
    pub schema_pda: Pubkey,
    pub schema_mint_pda: Option<Pubkey>,
    pub initialized: bool,
}

impl SasState {
    pub fn uninitialized() -> Self {
        Self {
            credential_pda: Pubkey::default(),
            schema_pda: Pubkey::default(),
            schema_mint_pda: None,
            initialized: false,
        }
    }
}

// ---------------------------------------------------------------------------
// PDA derivation helpers (mirrors SAS program seeds)
// ---------------------------------------------------------------------------

pub fn derive_credential_pda(authority: &Pubkey, name: &str) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"credential", authority.as_ref(), name.as_bytes()],
        &SOLANA_ATTESTATION_SERVICE_ID,
    )
}

pub fn derive_schema_pda(credential_pda: &Pubkey, name: &str, version: u8) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"schema",
            credential_pda.as_ref(),
            name.as_bytes(),
            &[version],
        ],
        &SOLANA_ATTESTATION_SERVICE_ID,
    )
}

pub fn derive_attestation_pda(
    credential_pda: &Pubkey,
    schema_pda: &Pubkey,
    nonce: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            b"attestation",
            credential_pda.as_ref(),
            schema_pda.as_ref(),
            nonce.as_ref(),
        ],
        &SOLANA_ATTESTATION_SERVICE_ID,
    )
}

pub fn derive_schema_mint_pda(schema_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"schemaMint", schema_pda.as_ref()],
        &SOLANA_ATTESTATION_SERVICE_ID,
    )
}

pub fn derive_attestation_mint_pda(attestation_pda: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"attestationMint", attestation_pda.as_ref()],
        &SOLANA_ATTESTATION_SERVICE_ID,
    )
}

pub fn derive_sas_authority() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[b"sas"], &SOLANA_ATTESTATION_SERVICE_ID)
}

// ---------------------------------------------------------------------------
// Init: create Credential + Schema + tokenize schema (one-time setup)
// ---------------------------------------------------------------------------

/// Creates the on-chain SAS Credential and Schema if they don't already exist.
/// Returns the initialized `SasState`.
pub async fn init_credential_and_schema(config: &Config) -> Result<SasState> {
    let keypair = config.load_solana_keypair("SAS init")?;

    let authority = keypair.pubkey();
    let (credential_pda, _) = derive_credential_pda(&authority, &config.sas_credential_name);
    let (schema_pda, _) = derive_schema_pda(
        &credential_pda,
        &config.sas_schema_name,
        config.sas_schema_version,
    );
    let (schema_mint_pda, _) = derive_schema_mint_pda(&schema_pda);

    let rpc_url = config.solana_rpc_url.clone();
    let credential_name = config.sas_credential_name.clone();
    let schema_name = config.sas_schema_name.clone();
    let _schema_version = config.sas_schema_version;
    let _token_name = config.sas_token_name.clone();
    let _token_uri = config.sas_token_uri.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

        // ── Credential ─────────────────────────────────────────────────────
        if client.get_account(&credential_pda).is_err() {
            info!(credential_pda = %credential_pda, "creating SAS Credential");

            let ix = CreateCredentialBuilder::new()
                .payer(authority)
                .credential(credential_pda)
                .authority(authority)
                .system_program(solana_sdk::system_program::ID)
                .name(credential_name)
                .signers(vec![authority])
                .instruction();

            send_ix(&client, &keypair, ix, "create SAS credential")?;
        } else {
            info!(credential_pda = %credential_pda, "SAS Credential already exists");
        }

        // ── Schema ─────────────────────────────────────────────────────────
        if client.get_account(&schema_pda).is_err() {
            info!(schema_pda = %schema_pda, "creating SAS Schema");

            // Three String fields: proof_count | tee_signing_address | last_proof_hash
            let ix = CreateSchemaBuilder::new()
                .payer(authority)
                .authority(authority)
                .credential(credential_pda)
                .schema(schema_pda)
                .name(schema_name)
                .description("CLAWD TEE gateway verified inference count".to_string())
                .layout(vec![SAS_TYPE_STRING, SAS_TYPE_STRING, SAS_TYPE_STRING])
                .field_names(vec![
                    "proof_count".to_string(),
                    "tee_signing_address".to_string(),
                    "last_proof_hash".to_string(),
                ])
                .instruction();

            send_ix(&client, &keypair, ix, "create SAS schema")?;
        } else {
            info!(schema_pda = %schema_pda, "SAS Schema already exists");
        }

        // ── Tokenize schema (makes attestations appear as NFTs in wallets) ─
        if client.get_account(&schema_mint_pda).is_err() {
            info!(schema_mint_pda = %schema_mint_pda, "tokenizing SAS Schema");

            let (sas_authority, _) = derive_sas_authority();
            let schema_mint_size = compute_schema_mint_size();

            let ix = TokenizeSchemaBuilder::new()
                .payer(authority)
                .authority(authority)
                .sas_pda(sas_authority)
                .credential(credential_pda)
                .schema(schema_pda)
                .mint(schema_mint_pda)
                .max_size(schema_mint_size as u64)
                .instruction();

            send_ix(&client, &keypair, ix, "tokenize SAS schema")?;
        } else {
            info!(schema_mint_pda = %schema_mint_pda, "SAS Schema mint already exists");
        }

        Ok(())
    })
    .await
    .context("spawn_blocking")??;

    info!(
        credential = %credential_pda,
        schema = %schema_pda,
        schema_mint = %schema_mint_pda,
        "SAS initialized"
    );

    Ok(SasState {
        credential_pda,
        schema_pda,
        schema_mint_pda: Some(schema_mint_pda),
        initialized: true,
    })
}

// ---------------------------------------------------------------------------
// Issue: create a tokenized SAS attestation for a wallet
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueResult {
    pub wallet: String,
    pub attestation_pda: String,
    pub attestation_mint: Option<String>,
    pub tx_sig: String,
    pub proof_count: u64,
    pub already_existed: bool,
}

/// Issue (or re-issue) a SAS attestation to `wallet_pubkey`.
pub async fn issue_attestation(
    config: &Config,
    sas: &SasState,
    wallet_pubkey: &str,
    proof_count: u64,
    tee_signing_address: &str,
    last_proof_hash: &str,
) -> Result<IssueResult> {
    let keypair = config.load_solana_keypair("issue_attestation")?;

    let authority = keypair.pubkey();
    let wallet = Pubkey::from_str(wallet_pubkey).context("invalid wallet pubkey")?;
    let credential_pda = sas.credential_pda;
    let schema_pda = sas.schema_pda;
    let schema_mint_pda = sas.schema_mint_pda;

    let (attestation_pda, _) = derive_attestation_pda(&credential_pda, &schema_pda, &wallet);
    let (attestation_mint_pda, _) = derive_attestation_mint_pda(&attestation_pda);

    let data = ClawdTeeAttestationData {
        proof_count: proof_count.to_string(),
        tee_signing_address: tee_signing_address.to_string(),
        last_proof_hash: last_proof_hash.to_string(),
    };

    let expiry = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + config.sas_expiry_secs;

    let token_name = config.sas_token_name.clone();
    let token_symbol = config.sas_token_symbol.clone();
    let token_uri = config.sas_token_uri.clone();
    let rpc_url = config.solana_rpc_url.clone();
    let tokenize = schema_mint_pda.is_some();

    let result = tokio::task::spawn_blocking(move || -> Result<IssueResult> {
        let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

        // Close existing attestation so we can re-issue with updated counts
        if let Ok(existing) = client.get_account(&attestation_pda) {
            let _ = existing; // suppress warning
            info!(attestation_pda = %attestation_pda, "closing existing attestation to re-issue");

            let close_ix = CloseAttestationBuilder::new()
                .payer(authority)
                .attestation(attestation_pda)
                .authority(authority)
                .credential(credential_pda)
                .instruction();

            // Best-effort — don't fail if already closed
            let _ = send_ix(&client, &keypair, close_ix, "close old SAS attestation");
        }

        // Serialize attestation data with Borsh
        let mut serialized = Vec::new();
        BorshSerialize::serialize(&data, &mut serialized).context("borsh serialize")?;

        let tx_sig = if tokenize {
            let schema_mint = schema_mint_pda.unwrap();
            let (sas_authority, _) = derive_sas_authority();

            let mint_size = compute_attestation_mint_size(
                &token_name,
                &token_symbol,
                &token_uri,
                &attestation_pda,
                &schema_pda,
            );

            let recipient_ata = spl_associated_token_address(&wallet, &attestation_mint_pda);

            let ix = CreateTokenizedAttestationBuilder::new()
                .payer(authority)
                .authority(authority)
                .sas_pda(sas_authority)
                .credential(credential_pda)
                .schema(schema_pda)
                .attestation(attestation_pda)
                .schema_mint(schema_mint)
                .attestation_mint(attestation_mint_pda)
                .recipient(wallet)
                .nonce(wallet)
                .expiry(expiry)
                .data(serialized)
                .name(token_name)
                .symbol(token_symbol)
                .uri(token_uri)
                .mint_account_space(mint_size as u16)
                .recipient_token_account(recipient_ata)
                .instruction();

            send_ix(&client, &keypair, ix, "create tokenized SAS attestation")?
        } else {
            let ix = CreateAttestationBuilder::new()
                .payer(authority)
                .authority(authority)
                .credential(credential_pda)
                .schema(schema_pda)
                .attestation(attestation_pda)
                .data(serialized)
                .expiry(expiry)
                .nonce(wallet)
                .instruction();

            send_ix(&client, &keypair, ix, "create SAS attestation")?
        };

        Ok(IssueResult {
            wallet: wallet.to_string(),
            attestation_pda: attestation_pda.to_string(),
            attestation_mint: schema_mint_pda.map(|_| attestation_mint_pda.to_string()),
            tx_sig: tx_sig.to_string(),
            proof_count,
            already_existed: false,
        })
    })
    .await
    .context("spawn_blocking")??;

    info!(
        wallet = %wallet_pubkey,
        attestation_pda = %result.attestation_pda,
        tx = %result.tx_sig,
        proof_count,
        "SAS attestation issued"
    );

    Ok(result)
}

// ---------------------------------------------------------------------------
// Verify: check if a wallet has a valid (non-expired) SAS attestation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResult {
    pub wallet: String,
    pub verified: bool,
    pub attestation_pda: Option<String>,
    pub expiry: Option<i64>,
    pub data: Option<ClawdTeeAttestationData>,
    pub proof_count: u64,
}

pub async fn verify_attestation(
    config: &Config,
    sas: &SasState,
    wallet_pubkey: &str,
    tracker: &ProofTracker,
) -> Result<VerifyResult> {
    if !sas.initialized {
        return Ok(VerifyResult {
            wallet: wallet_pubkey.to_string(),
            verified: false,
            attestation_pda: None,
            expiry: None,
            data: None,
            proof_count: proof_count(tracker, wallet_pubkey),
        });
    }

    let wallet = Pubkey::from_str(wallet_pubkey).context("invalid wallet pubkey")?;
    let credential_pda = sas.credential_pda;
    let schema_pda = sas.schema_pda;
    let (attestation_pda, _) = derive_attestation_pda(&credential_pda, &schema_pda, &wallet);
    let rpc_url = config.solana_rpc_url.clone();
    let count = proof_count(tracker, wallet_pubkey);

    let result = tokio::task::spawn_blocking(move || -> Result<VerifyResult> {
        let client = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

        match client.get_account(&attestation_pda) {
            Err(_) => Ok(VerifyResult {
                wallet: wallet.to_string(),
                verified: false,
                attestation_pda: Some(attestation_pda.to_string()),
                expiry: None,
                data: None,
                proof_count: count,
            }),
            Ok(account) => match Attestation::from_bytes(&account.data) {
                Err(e) => {
                    warn!("could not decode attestation: {e}");
                    Ok(VerifyResult {
                        wallet: wallet.to_string(),
                        verified: false,
                        attestation_pda: Some(attestation_pda.to_string()),
                        expiry: None,
                        data: None,
                        proof_count: count,
                    })
                }
                Ok(att) => {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64;
                    let valid = now < att.expiry;

                    let data = ClawdTeeAttestationData::try_from_slice(&att.data).ok();

                    Ok(VerifyResult {
                        wallet: wallet.to_string(),
                        verified: valid,
                        attestation_pda: Some(attestation_pda.to_string()),
                        expiry: Some(att.expiry),
                        data,
                        proof_count: count,
                    })
                }
            },
        }
    })
    .await
    .context("spawn_blocking")??;

    Ok(result)
}

// ---------------------------------------------------------------------------
// Token-2022 mint size calculation
// ---------------------------------------------------------------------------

/// Approximate schema group-token mint account size (Group Pointer extension only).
/// In production import from spl-token-2022 ExtensionType calculation.
fn compute_schema_mint_size() -> usize {
    // Base mint (82) + GroupPointer extension header (4) + GroupPointer data (64) + padding
    82 + 4 + 64 + 4
}

/// Approximate attestation mint size including all extensions + metadata.
fn compute_attestation_mint_size(
    name: &str,
    symbol: &str,
    uri: &str,
    attestation_pda: &Pubkey,
    _schema_pda: &Pubkey,
) -> usize {
    // Base extensions (non-transferable, group member pointer, metadata pointer,
    // permanent delegate, mint close authority, token group member) ≈ 280 bytes
    let base = 82 + 280;

    // Metadata TLV: type(2) + length(2) + body
    // Body: update_authority(32) + mint(32) + name_len(4)+name + sym_len(4)+sym
    //       + uri_len(4)+uri + additional_metadata_len(4)
    //       + 2 entries × (key_len(4)+key + val_len(4)+val)
    let addr_len = attestation_pda.to_string().len();
    let meta = 4
        + 32
        + 32
        + 4
        + name.len()
        + 4
        + symbol.len()
        + 4
        + uri.len()
        + 4
        + (4 + "attestation".len() + 4 + addr_len)
        + (4 + "schema".len() + 4 + addr_len);

    base + 4 + meta + 64 // +64 padding
}

/// Derive the associated token address for a Token-2022 mint.
fn spl_associated_token_address(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    // ATA derivation: ["", wallet, token_program, mint] under ATA program
    let token_2022_id = Pubkey::from_str("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb").unwrap();
    let ata_program = Pubkey::from_str("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJe8bv").unwrap();
    Pubkey::find_program_address(
        &[owner.as_ref(), token_2022_id.as_ref(), mint.as_ref()],
        &ata_program,
    )
    .0
}

// ---------------------------------------------------------------------------
// Transaction helper
// ---------------------------------------------------------------------------

fn send_ix(client: &RpcClient, payer: &Keypair, ix: Instruction, label: &str) -> Result<Signature> {
    let sim_msg = Message::new(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit(1_400_000),
            ComputeBudgetInstruction::set_compute_unit_price(1_000),
            ix.clone(),
        ],
        Some(&payer.pubkey()),
    );
    let sim_tx = Transaction::new(&[payer], sim_msg, client.get_latest_blockhash()?);
    let units = client
        .simulate_transaction(&sim_tx)
        .ok()
        .and_then(|r| r.value.units_consumed)
        .unwrap_or(400_000);

    let msg = Message::new(
        &[
            ComputeBudgetInstruction::set_compute_unit_limit((units * 11 / 10) as u32),
            ComputeBudgetInstruction::set_compute_unit_price(1_000),
            ix,
        ],
        Some(&payer.pubkey()),
    );
    let tx = Transaction::new(&[payer], msg, client.get_latest_blockhash()?);
    client
        .send_and_confirm_transaction_with_spinner(&tx)
        .with_context(|| format!("{label} tx failed"))
}
