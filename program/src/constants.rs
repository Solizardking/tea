// ── V1 seeds / discriminators (kept for backwards compatibility) ─────────────

/// PDA seed for TeeProof V1 accounts: ["tee_proof", quote_hash]
pub const TEE_PROOF_SEED: &[u8] = b"tee_proof";

/// PDA seed for the program-wide V1 proof counter (global — do not use in hot path)
pub const PROOF_COUNTER_SEED: &[u8] = b"proof_counter";

/// Discriminator for TeeProof V1 accounts
pub const TEE_PROOF_DISCRIMINATOR: u8 = 0x54; // 'T'

/// Discriminator for ProofCounter accounts
pub const PROOF_COUNTER_DISCRIMINATOR: u8 = 0x43; // 'C'

pub const ETH_ADDRESS_LEN: usize = 20;
pub const QUOTE_HASH_LEN: usize = 32;

/// V1 TeeProof account size (legacy):
/// discriminator(1) + quote_hash(32) + signing_address(20) + is_valid(1)
/// + timestamp(8) + slot(8) + submitter(32) = 102
pub const TEE_PROOF_SIZE: usize = 1 + 32 + 20 + 1 + 8 + 8 + 32;

/// ProofCounter account fixed size: discriminator(1) + count(8) = 9
pub const PROOF_COUNTER_SIZE: usize = 1 + 8;

// ── V2 seeds / discriminators ────────────────────────────────────────────────

/// PDA seed for TeeProofV2 accounts: ["tee_proof_v2", signed_payload_hash]
pub const TEE_PROOF_V2_SEED: &[u8] = b"tee_proof_v2";

/// PDA seed for sharded day counter: ["proof_counter_day", b"YYYYMMDD"]
pub const PROOF_COUNTER_DAY_SEED: &[u8] = b"proof_counter_day";

/// PDA seed for verifier registry: ["verifier_registry", verifier_pubkey]
pub const VERIFIER_REGISTRY_SEED: &[u8] = b"verifier_registry";

/// PDA seed for RedPill nullifiers connecting to clawd-zk: ["rp_nullifier", nullifier_hash]
pub const REDPILL_NULLIFIER_SEED: &[u8] = b"rp_nullifier";

/// Discriminator for TeeProofV2 accounts
pub const TEE_PROOF_V2_DISCRIMINATOR: u8 = 0x56; // 'V'

/// Discriminator for sharded day counter
pub const PROOF_COUNTER_DAY_DISCRIMINATOR: u8 = 0x44; // 'D'

/// Discriminator for VerifierRegistry accounts
pub const VERIFIER_REGISTRY_DISCRIMINATOR: u8 = 0x52; // 'R'

/// TeeProofV2 account fixed size:
/// discriminator(1) + version(1) + verification_level(1) + status(1)
/// + quote_hash(32) + signed_payload_hash(32) + request_hash(32) + response_hash(32)
/// + model_hash(32) + attestation_report_hash(32) + compose_hash(32)
/// + signing_address(20) + verifier_set_hash(32) + policy_version(4)
/// + nonce(32) + expires_at(8) + timestamp(8) + slot(8) + submitter(32)
/// = 1+1+1+1 + 32*7 + 20 + 32 + 4 + 32 + 8 + 8 + 8 + 32 = 4 + 224 + 20 + 32 + 4 + 32 + 24 + 32 = 372
pub const TEE_PROOF_V2_SIZE: usize = 4           // discriminator + version + verification_level + status
    + 32 * 7    // quote_hash + signed_payload_hash + request_hash + response_hash
                // + model_hash + attestation_report_hash + compose_hash
    + 20        // signing_address
    + 32        // verifier_set_hash
    + 4         // policy_version
    + 32        // nonce
    + 8         // expires_at
    + 8         // timestamp
    + 8         // slot
    + 32; // submitter

/// Sharded day counter size: discriminator(1) + day_tag(8) + count(8) = 17
pub const PROOF_COUNTER_DAY_SIZE: usize = 1 + 8 + 8;

/// VerifierRegistry account size:
/// discriminator(1) + verifier_pubkey(32) + provider_name_hash(32)
/// + verifier_type(1) + active(1) + policy_version(4) + valid_after(8) + valid_until(8) = 87
pub const VERIFIER_REGISTRY_SIZE: usize = 1 + 32 + 32 + 1 + 1 + 4 + 8 + 8;

// ── Verification levels ──────────────────────────────────────────────────────

/// Proof was stored on-chain but no signature was verified.
pub const VERIFICATION_LEVEL_STORED_ONLY: u8 = 0;
/// secp256k1 signature over the canonical payload was verified on-chain.
pub const VERIFICATION_LEVEL_TEE_SIG_ONCHAIN: u8 = 1;
/// DCAP/NRAS attestation verified off-chain by Phala/NVIDIA/Automata.
pub const VERIFICATION_LEVEL_DCAP_OFFCHAIN: u8 = 2;
/// Multiple independent verifiers attested.
pub const VERIFICATION_LEVEL_MULTI_VERIFIER: u8 = 3;
/// DCAP verified on-chain via Automata DCAP attestation contract.
pub const VERIFICATION_LEVEL_DCAP_ONCHAIN: u8 = 4;
/// Groth16 ZK authorization proof verified.
pub const VERIFICATION_LEVEL_ZK_AUTHORIZED: u8 = 5;

// ── Proof status ─────────────────────────────────────────────────────────────

pub const PROOF_STATUS_ACTIVE: u8 = 0;
pub const PROOF_STATUS_CONSUMED: u8 = 1;
pub const PROOF_STATUS_REVOKED: u8 = 2;
pub const PROOF_STATUS_EXPIRED: u8 = 3;

// ── Verifier types ───────────────────────────────────────────────────────────

pub const VERIFIER_TYPE_PHALA: u8 = 0;
pub const VERIFIER_TYPE_NVIDIA: u8 = 1;
pub const VERIFIER_TYPE_AUTOMATA: u8 = 2;
pub const VERIFIER_TYPE_CLAWD: u8 = 3;
pub const VERIFIER_TYPE_OTHER: u8 = 255;
