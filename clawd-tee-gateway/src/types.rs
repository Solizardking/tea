use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// OpenAI-compatible wire types (subset used for proxying)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// CLAWD-specific attestation payload attached to every response
// ---------------------------------------------------------------------------

/// Attached as `x-clawd-proof` JSON header and returned in `_clawd` field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClawdProof {
    /// SHA256(request_body) hex
    pub request_hash: String,
    /// SHA256(response_body) hex
    pub response_hash: String,
    /// SHA256(model_id) hex
    pub model_hash: String,
    /// SHA256(request_hash || response_hash || model_hash || nonce) hex
    /// — This is what the TEE signing key signs; also the TeeProofV2 PDA seed.
    pub signed_payload_hash: String,
    /// Ethereum-style address of the TEE signing key (20 bytes hex)
    pub signing_address: String,
    /// 65-byte secp256k1 signature over signed_payload_hash (hex)
    pub signature: String,
    /// Verification level stored in TeeProofV2
    pub verification_level: u8,
    /// Solana TeeProofV2 PDA address (base58) — present when proof was anchored
    pub solana_proof_address: Option<String>,
    /// Solana transaction signature — present when proof was anchored
    pub solana_tx_sig: Option<String>,
    /// TDX quote (hex, truncated for header transport) — full quote in /v1/attestation
    pub tdx_quote_hash: Option<String>,
    /// NRAS JWT verdict — true if GPU TEE verified
    pub gpu_verified: Option<bool>,
    /// Per-inference nonce (hex)
    pub nonce: String,
    /// ACI receipt id (`rcpt-…`) when a receipt was issued
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_id: Option<String>,
    /// SHA256 of the per-inference attestation report (hex)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attestation_report_hash: Option<String>,
}

// ---------------------------------------------------------------------------
// Attestation report returned by GET /v1/attestation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationReport {
    /// Ethereum address of the TEE signing key
    pub signing_address: String,
    /// SHA256 of the raw TDX quote bytes (hex)
    pub quote_hash: Option<String>,
    /// Full TDX quote (hex) — only present when running inside a TDX VM
    pub tdx_quote: Option<String>,
    /// NVIDIA GPU attestation JWT from NRAS
    pub nras_token: Option<String>,
    /// Parsed NRAS claims (subset)
    pub gpu_claims: Option<NrasClaims>,
    /// Docker compose manifest hash (from TDX mr_config)
    pub compose_hash: Option<String>,
    /// The verifier set hash stored in TeeProofV2
    pub verifier_set_hash: String,
    /// Policy version
    pub policy_version: u32,
    /// Unix timestamp this report was generated
    pub generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NrasClaims {
    pub overall_result: bool,
    pub gpu_uuid: Option<String>,
    pub driver_version: Option<String>,
    pub cc_mode: Option<String>,
}

// ---------------------------------------------------------------------------
// Proof anchor request (optional manual POST)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorRequest {
    pub signed_payload_hash: String,
    pub request_hash: String,
    pub response_hash: String,
    pub model_hash: String,
    pub signing_address: String,
    pub signature: String,
    pub nonce: String,
    pub quote_hash: Option<String>,
}
