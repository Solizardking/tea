//! TEE signing key management.
//!
//! The signing key is a secp256k1 keypair generated inside the TEE on first
//! boot. Its Ethereum address is embedded in the TDX report_data (bytes 0–19)
//! so attestation verifiers can confirm the key belongs to this TEE instance.
//!
//! Key lifecycle:
//!   - Default: ephemeral (regenerated each boot). Suitable for development.
//!   - With SIGNING_KEY_PEM_PATH: persisted to an in-enclave file. The enclave
//!     storage is encrypted at rest by the TEE platform (e.g. via dm-crypt with
//!     a TPM-sealed key on TDX).

use anyhow::{Context, Result};
use k256::{
    ecdsa::{Signature, SigningKey, VerifyingKey},
    pkcs8::{DecodePrivateKey, EncodePrivateKey},
};
use sha2::{Digest, Sha256};
use sha3::Keccak256;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use tracing::info;

use crate::config::Config;

pub struct TeeSigningKey {
    inner: SigningKey,
    /// 20-byte Ethereum-style address: keccak256(uncompressed_pubkey[1..])[12..]
    eth_address: [u8; 20],
}

impl TeeSigningKey {
    pub fn generate_or_load(config: &Config) -> Result<Self> {
        let signing_key = if let Some(pem) = &config.signing_key_pem_value {
            SigningKey::from_pkcs8_pem(pem).with_context(|| "parsing SIGNING_KEY_PEM")?
        } else if let Some(pem_path) = &config.signing_key_pem {
            if std::path::Path::new(pem_path).exists() {
                let pem = std::fs::read_to_string(pem_path)
                    .with_context(|| format!("reading signing key from {pem_path}"))?;
                SigningKey::from_pkcs8_pem(&pem).with_context(|| "parsing signing key PEM")?
            } else {
                let key = SigningKey::random(&mut rand::thread_rng());
                let pem = key
                    .to_pkcs8_pem(Default::default())
                    .with_context(|| "serializing signing key")?;
                write_secret_file(pem_path, pem.as_bytes())
                    .with_context(|| format!("writing signing key to {pem_path}"))?;
                info!(path = %pem_path, "generated new TEE signing key");
                key
            }
        } else {
            let key = SigningKey::random(&mut rand::thread_rng());
            info!("generated ephemeral TEE signing key (not persisted)");
            key
        };

        let eth_address = derive_eth_address(signing_key.verifying_key());
        Ok(Self {
            inner: signing_key,
            eth_address,
        })
    }

    /// 20-byte Ethereum address derived from the public key.
    pub fn eth_address(&self) -> &[u8; 20] {
        &self.eth_address
    }

    pub fn eth_address_hex(&self) -> String {
        format!("0x{}", hex::encode(self.eth_address))
    }

    /// Uncompressed SEC1 public key hex (`04 || x || y`), no `0x` prefix.
    pub fn uncompressed_pubkey_hex(&self) -> String {
        hex::encode(
            self.inner
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes(),
        )
    }

    /// Sign arbitrary bytes as keccak256(msg) with a recoverable secp256k1 sig.
    /// Used for ACI receipt documents (`algo = ecdsa-secp256k1`).
    pub fn sign_bytes(&self, msg: &[u8]) -> Result<[u8; 65]> {
        let digest = Keccak256::new_with_prefix(msg);
        let (sig, recovery_id): (Signature, _) = self
            .inner
            .sign_digest_recoverable(digest)
            .with_context(|| "signing recoverable secp256k1 message")?;
        let bytes = sig.to_bytes();
        let mut out = [0u8; 65];
        out[..64].copy_from_slice(&bytes);
        out[64] = recovery_id.to_byte();
        Ok(out)
    }

    /// Sign the 32-byte payload hash for Solana's native secp256k1 verifier.
    ///
    /// The secp256k1 native program verifies signatures against
    /// keccak256(message_data). StoreProofV2 sets message_data to the exact
    /// signed_payload_hash bytes, so we sign keccak256(signed_payload_hash)
    /// and return r(32) + s(32) + recovery id(1).
    pub fn sign_hash(&self, hash: &[u8; 32]) -> Result<[u8; 65]> {
        let digest = Keccak256::new_with_prefix(hash);
        let (sig, recovery_id): (Signature, _) = self
            .inner
            .sign_digest_recoverable(digest)
            .with_context(|| "signing recoverable secp256k1 digest")?;
        let bytes = sig.to_bytes();
        let mut out = [0u8; 65];
        out[..64].copy_from_slice(&bytes);
        out[64] = recovery_id.to_byte();
        Ok(out)
    }

    pub fn verifying_key(&self) -> &VerifyingKey {
        self.inner.verifying_key()
    }
}

/// Derive Ethereum address: keccak256(uncompressed_pubkey[1..])[12..]
fn derive_eth_address(vk: &VerifyingKey) -> [u8; 20] {
    // uncompressed SEC1: 0x04 || x(32) || y(32) = 65 bytes
    let uncompressed = vk.to_encoded_point(false);
    let xy = &uncompressed.as_bytes()[1..]; // 64 bytes
    let hash = Keccak256::digest(xy);
    let mut addr = [0u8; 20];
    addr.copy_from_slice(&hash[12..]);
    addr
}

fn write_secret_file(path: &str, bytes: &[u8]) -> Result<()> {
    let path = std::path::Path::new(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating secret directory {}", parent.display()))?;
    }

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);

    let mut file = options
        .open(path)
        .with_context(|| format!("creating secret file {}", path.display()))?;
    std::io::Write::write_all(&mut file, bytes)
        .with_context(|| format!("writing secret file {}", path.display()))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Canonical signed payload hash
// ---------------------------------------------------------------------------

/// Compute signedPayloadHash = SHA256(request_hash || response_hash || model_hash || nonce)
///
/// This is the exact 32-byte value the TEE signs and the TeeProofV2 PDA is
/// derived from. It binds the signature to the specific inference triple.
pub fn compute_signed_payload_hash(
    request_hash: &[u8; 32],
    response_hash: &[u8; 32],
    model_hash: &[u8; 32],
    nonce: &[u8; 32],
) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"CLAWD_REDPILL_V2:");
    h.update(request_hash);
    h.update(response_hash);
    h.update(model_hash);
    h.update(nonce);
    h.finalize().into()
}

/// SHA256 of arbitrary bytes.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    Sha256::digest(data).into()
}

/// SHA256 of a string.
pub fn sha256_str(s: &str) -> [u8; 32] {
    sha256(s.as_bytes())
}

/// Generate a random 32-byte nonce.
pub fn random_nonce() -> [u8; 32] {
    use rand::RngCore;
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    nonce
}
