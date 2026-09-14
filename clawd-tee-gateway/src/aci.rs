//! RedPill / private-ai-gateway ACI (aci/1) workload identity.
//!
//! Implements the attestation statement and JCS digest constructions from
//! https://github.com/Dstack-TEE/private-ai-gateway/blob/main/spec/aci.md
//! so clients can verify `GET /v1/aci/attestation?nonce=…` the same way they
//! verify a RedPill provider.

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::{config::Config, signing::TeeSigningKey};

pub const ACI_VERSION: &str = "aci/1";
pub const RECEIPT_KEY_ID: &str = "receipt-v1";
pub const HEADER_RECEIPT_ID: &str = "x-receipt-id";
pub const HEADER_ACI_IDENTITY: &str = "x-aci-identity";
pub const HEADER_ACI_KEYSET_DIGEST: &str = "x-aci-keyset-digest";
pub const HEADER_ACI_VERSION: &str = "x-aci-version";

/// RFC 8785 JSON Canonicalization Scheme for ACI artifacts.
///
/// Under ACI's constraints (ASCII field names, integer numbers) this is
/// compact JSON with lexicographically sorted object keys.
pub fn jcs(value: &Value) -> String {
    let mut out = String::new();
    write_jcs(value, &mut out);
    out
}

fn write_jcs(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_jcs_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_jcs(item, out);
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_jcs_string(key, out);
                out.push(':');
                write_jcs(&map[*key], out);
            }
            out.push('}');
        }
    }
}

fn write_jcs_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(bytes))
}

/// Validate an ACI attestation nonce: exactly 64 lowercase hex characters.
pub fn parse_aci_nonce(raw: &str) -> Result<&str, String> {
    if raw.len() != 64 || !raw.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err("nonce must be exactly 64 lowercase hex characters (32 random bytes)".into());
    }
    Ok(raw)
}

/// ACI §3.2 attestation statement (exact bytes; no whitespace).
pub fn attestation_statement(keyset_digest: &str, nonce: Option<&str>) -> String {
    match nonce {
        Some(n) => format!(
            "{{\"keyset_digest\":\"{keyset_digest}\",\"nonce\":\"{n}\",\"purpose\":\"aci.report_data.v1\"}}"
        ),
        None => format!(
            "{{\"keyset_digest\":\"{keyset_digest}\",\"nonce\":null,\"purpose\":\"aci.report_data.v1\"}}"
        ),
    }
}

/// 32-byte SHA-256 of the attestation statement (ACI `report_data`).
pub fn report_data_digest(keyset_digest: &str, nonce: Option<&str>) -> [u8; 32] {
    Sha256::digest(attestation_statement(keyset_digest, nonce).as_bytes()).into()
}

/// 64-byte TDX report-data slot: digest in bytes 0–31, zeros in 32–63.
pub fn report_data_slot(keyset_digest: &str, nonce: Option<&str>) -> [u8; 64] {
    let mut slot = [0u8; 64];
    let digest = report_data_digest(keyset_digest, nonce);
    slot[..32].copy_from_slice(&digest);
    slot
}

/// Stable workload identity published as `workload_id` / `x-aci-identity`.
pub fn workload_id_from_receipt_key(public_key_hex: &str) -> String {
    sha256_prefixed(format!("clawd-tee-workload:{public_key_hex}").as_bytes())
}

#[derive(Clone)]
pub struct AciIdentity {
    pub workload_id: String,
    pub keyset: Value,
    pub keyset_digest: String,
    pub receipt_key_id: String,
    pub receipt_public_key_hex: String,
}

impl AciIdentity {
    pub fn from_signing_key(config: &Config, signing_key: &TeeSigningKey) -> Self {
        let receipt_public_key_hex = signing_key.uncompressed_pubkey_hex();
        let not_after = unix_now().saturating_add(config.keyset_ttl_secs as i64);
        let keyset = json!({
            "subject": config.workload_subject,
            "not_after": not_after,
            "receipt_signing_keys": [{
                "key_id": RECEIPT_KEY_ID,
                "algo": "ecdsa-secp256k1",
                "public_key": receipt_public_key_hex,
            }],
            "e2ee_public_keys": [],
            "tls_public_keys": [],
        });
        let keyset_digest = sha256_prefixed(jcs(&keyset).as_bytes());
        let workload_id = workload_id_from_receipt_key(&receipt_public_key_hex);

        tracing::info!(
            workload_id = %workload_id,
            keyset_digest = %keyset_digest,
            signing_address = %signing_key.eth_address_hex(),
            "ACI workload identity ready"
        );

        Self {
            workload_id,
            keyset,
            keyset_digest,
            receipt_key_id: RECEIPT_KEY_ID.to_string(),
            receipt_public_key_hex,
        }
    }

    pub fn sign_receipt_jcs(&self, signing_key: &TeeSigningKey, canonical: &str) -> String {
        match signing_key.sign_bytes(canonical.as_bytes()) {
            Ok(sig) => hex::encode(sig),
            Err(e) => {
                tracing::error!("receipt signing failed: {e:#}");
                String::new()
            }
        }
    }

    pub fn source_provenance(&self, config: &Config) -> Value {
        json!({
            "repo_url": config.source_repo_url,
            "repo_commit": config.source_repo_commit,
            "image_digest": config.source_image_digest,
            "image_provenance": Value::Null,
        })
    }
}

pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Build the ACI attestation report (plus CLAWD compatibility fields).
pub fn build_attestation_report(
    identity: &AciIdentity,
    config: &Config,
    signing_address: &str,
    nonce: Option<&str>,
    quote_hex: &str,
    quote_hash_hex: &str,
    gpu_claims: Option<Value>,
    tdx_backend: &str,
) -> Value {
    let slot = report_data_slot(&identity.keyset_digest, nonce);
    let now = unix_now();
    let mut report = Map::new();
    report.insert("api_version".into(), json!(ACI_VERSION));
    report.insert("workload_id".into(), json!(identity.workload_id));
    report.insert(
        "workload_keyset_digest".into(),
        json!(identity.keyset_digest),
    );
    report.insert(
        "attestation".into(),
        json!({
            "tee_type": "tdx",
            "vendor": "clawd-tee",
            "report_data": hex::encode(slot),
            "freshness": {
                "fetched_at": now,
                "stale_after": now + 3600,
            },
            "source_provenance": identity.source_provenance(config),
            "workload_keyset": identity.keyset,
            "evidence": {
                "quote": quote_hex,
                "quote_report_data": hex::encode(slot),
                "event_log": Value::Null,
                "vm_config": Value::Null,
                "key_custody": {
                    "provider": "local-ephemeral",
                    "keys": [RECEIPT_KEY_ID],
                },
                "tdx_backend": tdx_backend,
            },
        }),
    );
    report.insert(
        "service_capabilities".into(),
        json!({
            "supported_e2ee_versions": [],
            "serving": "aggregator",
        }),
    );

    // CLAWD / Dark Pool compatibility fields (ignored by ACI verifiers).
    report.insert("signing_address".into(), json!(signing_address));
    report.insert("quote_hash".into(), json!(quote_hash_hex));
    report.insert("tdx_quote".into(), json!(quote_hex));
    report.insert("nras_token".into(), Value::Null);
    report.insert("gpu_claims".into(), gpu_claims.unwrap_or(Value::Null));
    report.insert("compose_hash".into(), Value::Null);
    report.insert("verifier_set_hash".into(), json!(hex::encode([0u8; 32])));
    report.insert("policy_version".into(), json!(0));
    report.insert("generated_at".into(), json!(now));
    Value::Object(report)
}

/// Digest stored on-chain as TeeProofV2.attestation_report_hash.
///
/// Hashes the quote + ACI report_data slot + GPU verdict so the PDA commits
/// to real evidence instead of 32 zero bytes.
pub fn inference_attestation_report_hash(
    quote: &[u8],
    report_data: &[u8; 64],
    signing_address: &str,
    gpu_verified: Option<bool>,
    tdx_backend: &str,
) -> [u8; 32] {
    let report = json!({
        "quote_sha256": sha256_hex(quote),
        "report_data": hex::encode(report_data),
        "signing_address": signing_address,
        "gpu_verified": gpu_verified,
        "tdx_backend": tdx_backend,
        "purpose": "clawd.tee_proof_v2.attestation_report.v1",
    });
    Sha256::digest(jcs(&report).as_bytes()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jcs_and_keyset_digest_match_aci_vectors() {
        let keyset: Value = serde_json::from_str(
            r#"{
              "subject": "dstack-app://example-app",
              "not_after": 1800000000,
              "receipt_signing_keys": [{
                "key_id": "receipt-1",
                "algo": "ed25519",
                "public_key": "8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394"
              }],
              "e2ee_public_keys": [{
                "key_id": "e2ee-1",
                "algo": "x25519-aes-256-gcm-hkdf-sha256",
                "public_key": "5dfedd3b6bd47f6fa28ee15d969d5bb0ea53774d488bdaf9df1c6e0124b3ef22"
              }],
              "tls_public_keys": [{
                "domain": "api.example.com",
                "spki_sha256": "c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0"
              }]
            }"#,
        )
        .unwrap();

        let expected_jcs = r#"{"e2ee_public_keys":[{"algo":"x25519-aes-256-gcm-hkdf-sha256","key_id":"e2ee-1","public_key":"5dfedd3b6bd47f6fa28ee15d969d5bb0ea53774d488bdaf9df1c6e0124b3ef22"}],"not_after":1800000000,"receipt_signing_keys":[{"algo":"ed25519","key_id":"receipt-1","public_key":"8139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394"}],"subject":"dstack-app://example-app","tls_public_keys":[{"domain":"api.example.com","spki_sha256":"c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0c0"}]}"#;
        assert_eq!(jcs(&keyset), expected_jcs);
        assert_eq!(
            sha256_prefixed(jcs(&keyset).as_bytes()),
            "sha256:53a5cd44b30dcc51999754c719f2628a041f174ecbf9662a6f8e898a10cd9371"
        );
    }

    #[test]
    fn report_data_binds_nonce_per_aci_vectors() {
        let digest = "sha256:53a5cd44b30dcc51999754c719f2628a041f174ecbf9662a6f8e898a10cd9371";
        let nonce = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        assert_eq!(
            hex::encode(report_data_digest(digest, Some(nonce))),
            "df2174d28130852b413646a3786927b93e94c11d770268b65def8bdba45cb49e"
        );
        assert_eq!(
            hex::encode(report_data_digest(digest, None)),
            "0633919ca3f00e97bafaa3304278eb22420cc3ff0d19f87dfca2d3f7508150bc"
        );
        assert_eq!(
            hex::encode(report_data_slot(digest, Some(nonce))),
            "df2174d28130852b413646a3786927b93e94c11d770268b65def8bdba45cb49e0000000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    fn nonce_must_be_64_lowercase_hex() {
        assert!(parse_aci_nonce(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        )
        .is_ok());
        assert!(parse_aci_nonce("abcd").is_err());
        assert!(parse_aci_nonce(
            "000102030405060708090A0B0C0D0E0F101112131415161718191A1B1C1D1E1F"
        )
        .is_err());
        assert!(parse_aci_nonce(
            "zzzz02030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
        )
        .is_err());
    }

    #[test]
    fn inference_report_hash_is_nonzero_for_real_quote() {
        let quote = vec![0xCAu8; 256];
        let slot = [0x11u8; 64];
        let hash = inference_attestation_report_hash(&quote, &slot, "0xabc", Some(false), "stub");
        assert_ne!(hash, [0u8; 32]);
        let again = inference_attestation_report_hash(&quote, &slot, "0xabc", Some(false), "stub");
        assert_eq!(hash, again);
    }
}
