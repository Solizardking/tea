extern crate alloc;

use alloc::vec::Vec;
use pinocchio::{program_error::ProgramError, pubkey::Pubkey};

use crate::{
    constants::{
        ETH_ADDRESS_LEN, QUOTE_HASH_LEN, TEE_PROOF_DISCRIMINATOR, TEE_PROOF_SIZE,
        TEE_PROOF_V2_DISCRIMINATOR, TEE_PROOF_V2_SIZE,
    },
    error::RedpillVerifierError,
};

/// On-chain record of a verified TEE attestation proof.
///
/// PDA seeds: ["tee_proof", quote_hash[0..32]]
///
/// Maps 1:1 to the Ethereum RedpillProofStore Proof struct, replacing:
///   - `bytes32 quoteHash`    → `[u8; 32]` (SHA256 of TDX quote)
///   - `address signingAddress` → `[u8; 20]` (Ethereum-style 20-byte address)
///   - `uint256 timestamp`    → `i64` (Unix timestamp from Clock sysvar)
///   - `uint256 blockNumber`  → `u64` (Solana slot)
///   - `address submitter`    → `Pubkey` (Solana pubkey of tx signer)
#[derive(Clone, Debug, PartialEq)]
pub struct TeeProof {
    /// SHA256 of the raw TDX quote bytes
    pub quote_hash: [u8; QUOTE_HASH_LEN],
    /// Ethereum-style TEE signing address (first 20 bytes of keccak(pubkey))
    pub signing_address: [u8; ETH_ADDRESS_LEN],
    /// Whether the Automata/Phala DCAP verification passed (verified off-chain)
    pub is_valid: bool,
    /// Unix timestamp when this proof was stored (from Clock sysvar)
    pub timestamp: i64,
    /// Solana slot when this proof was stored
    pub slot: u64,
    /// Pubkey that submitted this proof
    pub submitter: Pubkey,
}

impl TeeProof {
    /// Serialize to bytes (with leading discriminator byte).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(TEE_PROOF_SIZE);
        buf.push(TEE_PROOF_DISCRIMINATOR);
        buf.extend_from_slice(&self.quote_hash);
        buf.extend_from_slice(&self.signing_address);
        buf.push(self.is_valid as u8);
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(&self.slot.to_le_bytes());
        buf.extend_from_slice(self.submitter.as_ref());
        buf
    }

    /// Deserialize from bytes (expects leading discriminator byte).
    pub fn try_from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < TEE_PROOF_SIZE {
            return Err(RedpillVerifierError::AccountDataTooSmall.into());
        }
        if data[0] != TEE_PROOF_DISCRIMINATOR {
            return Err(RedpillVerifierError::InvalidDiscriminator.into());
        }

        let mut offset = 1usize;

        let quote_hash: [u8; 32] = data[offset..offset + 32].try_into().unwrap();
        offset += 32;

        let signing_address: [u8; 20] = data[offset..offset + 20].try_into().unwrap();
        offset += 20;

        let is_valid = data[offset] != 0;
        offset += 1;

        let timestamp = i64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let slot = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
        offset += 8;

        let submitter: Pubkey = data[offset..offset + 32].try_into().unwrap();

        Ok(Self {
            quote_hash,
            signing_address,
            is_valid,
            timestamp,
            slot,
            submitter,
        })
    }
}

// ── V2 ────────────────────────────────────────────────────────────────────────

/// V2 on-chain proof record — versioned, forensic, machine-checkable.
///
/// PDA seeds: ["tee_proof_v2", signed_payload_hash[0..32]]
///
/// Security properties over V1:
///   - `is_valid: bool` → `verification_level: u8` + `status: u8`
///   - `signed_payload_hash` binds the secp256k1 signature to the exact payload,
///     preventing "valid sig over wrong message" attacks.
///   - `request_hash` / `response_hash` link the proof to the specific inference.
///   - `nonce` + `expires_at` prevent replay across time.
///   - `verifier_set_hash` records which verifier policy accepted the proof.
///   - `policy_version` allows policy rotation without breaking existing proofs.
#[derive(Clone, Debug, PartialEq)]
pub struct TeeProofV2 {
    pub version: u8,
    /// See VERIFICATION_LEVEL_* constants.
    pub verification_level: u8,
    /// See PROOF_STATUS_* constants.
    pub status: u8,

    /// SHA256 of the raw TDX quote bytes
    pub quote_hash: [u8; 32],
    /// SHA256 of the canonical signed payload (CLAWD_REDPILL_V2 || fields).
    /// This is what the secp256k1 instruction MUST have signed.
    pub signed_payload_hash: [u8; 32],
    /// SHA256 of the original inference request body
    pub request_hash: [u8; 32],
    /// SHA256 of the inference response
    pub response_hash: [u8; 32],
    /// SHA256 of the model identifier
    pub model_hash: [u8; 32],
    /// SHA256 of the DCAP attestation report
    pub attestation_report_hash: [u8; 32],
    /// SHA256 of the compose/mrconfig (links proof to exact runtime image)
    pub compose_hash: [u8; 32],

    /// Ethereum-style TEE signing address (first 20 bytes of keccak(pubkey))
    pub signing_address: [u8; 20],
    /// SHA256 of the active verifier set (set of verifier pubkeys that attested)
    pub verifier_set_hash: [u8; 32],
    /// Policy version number at time of proof creation
    pub policy_version: u32,

    /// Per-proof nonce (prevents cross-time replay)
    pub nonce: [u8; 32],
    /// Unix timestamp after which this proof should be considered stale
    pub expires_at: i64,
    /// Unix timestamp when stored (from Clock sysvar)
    pub timestamp: i64,
    /// Solana slot when stored
    pub slot: u64,
    /// Solana pubkey of the submitter
    pub submitter: Pubkey,
}

impl TeeProofV2 {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(TEE_PROOF_V2_SIZE);
        buf.push(TEE_PROOF_V2_DISCRIMINATOR);
        buf.push(self.version);
        buf.push(self.verification_level);
        buf.push(self.status);
        buf.extend_from_slice(&self.quote_hash);
        buf.extend_from_slice(&self.signed_payload_hash);
        buf.extend_from_slice(&self.request_hash);
        buf.extend_from_slice(&self.response_hash);
        buf.extend_from_slice(&self.model_hash);
        buf.extend_from_slice(&self.attestation_report_hash);
        buf.extend_from_slice(&self.compose_hash);
        buf.extend_from_slice(&self.signing_address);
        buf.extend_from_slice(&self.verifier_set_hash);
        buf.extend_from_slice(&self.policy_version.to_le_bytes());
        buf.extend_from_slice(&self.nonce);
        buf.extend_from_slice(&self.expires_at.to_le_bytes());
        buf.extend_from_slice(&self.timestamp.to_le_bytes());
        buf.extend_from_slice(&self.slot.to_le_bytes());
        buf.extend_from_slice(self.submitter.as_ref());
        buf
    }

    pub fn try_from_bytes(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < TEE_PROOF_V2_SIZE {
            return Err(RedpillVerifierError::AccountDataTooSmall.into());
        }
        if data[0] != TEE_PROOF_V2_DISCRIMINATOR {
            return Err(RedpillVerifierError::InvalidDiscriminator.into());
        }

        let mut o = 1usize;
        let version = data[o];
        o += 1;
        let verification_level = data[o];
        o += 1;
        let status = data[o];
        o += 1;

        macro_rules! read32 {
            () => {{
                let v: [u8; 32] = data[o..o + 32].try_into().unwrap();
                o += 32;
                v
            }};
        }

        let quote_hash = read32!();
        let signed_payload_hash = read32!();
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
        let timestamp = i64::from_le_bytes(data[o..o + 8].try_into().unwrap());
        o += 8;
        let slot = u64::from_le_bytes(data[o..o + 8].try_into().unwrap());
        o += 8;
        let submitter: Pubkey = data[o..o + 32].try_into().unwrap();

        Ok(Self {
            version,
            verification_level,
            status,
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
            timestamp,
            slot,
            submitter,
        })
    }
}

// ── VerifierRegistry ──────────────────────────────────────────────────────────

/// Registry entry for an authorized off-chain verifier.
/// PDA seeds: ["verifier_registry", verifier_pubkey]
#[derive(Clone, Debug, PartialEq)]
pub struct VerifierRegistry {
    /// Solana pubkey of the authorized verifier
    pub verifier_pubkey: Pubkey,
    /// Keccak256 of the provider name string
    pub provider_name_hash: [u8; 32],
    /// See VERIFIER_TYPE_* constants
    pub verifier_type: u8,
    /// 0 = active, 1 = revoked
    pub active: u8,
    /// Policy version this entry belongs to
    pub policy_version: u32,
    /// Unix timestamp: valid from
    pub valid_after: i64,
    /// Unix timestamp: valid until (0 = no expiry)
    pub valid_until: i64,
}
