//! In-memory ACI receipt store + signed receipt documents (aci/1).
//!
//! Receipts are hashes and verification metadata only — never plaintext
//! prompts. They live in process memory for `RECEIPT_TTL_SECS` (default 1h)
//! so a client can fetch `GET /v1/aci/receipts/{id}` after seeing
//! `x-receipt-id`. This is not a durable transparency log.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use serde_json::{json, Map, Value};

use crate::{
    aci::{jcs, sha256_prefixed, unix_now, AciIdentity, ACI_VERSION},
    signing::TeeSigningKey,
};

const RECEIPT_ID_PREFIX: &str = "rcpt-";

#[derive(Clone)]
struct StoredReceipt {
    document: Value,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct ReceiptStore {
    by_id: Arc<DashMap<String, StoredReceipt>>,
    ttl: Duration,
    max_entries: usize,
}

impl ReceiptStore {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        Self {
            by_id: Arc::new(DashMap::new()),
            ttl: Duration::from_secs(ttl_secs.max(1)),
            max_entries,
        }
    }

    pub fn insert(&self, receipt_id: &str, chat_id: Option<&str>, document: Value) {
        self.evict_expired();
        if self.by_id.len() >= self.max_entries {
            // Drop the first expired-or-arbitrary entry to stay bounded.
            if let Some(oldest) = self.by_id.iter().next().map(|e| e.key().clone()) {
                self.by_id.remove(&oldest);
            }
        }
        let stored = StoredReceipt {
            document,
            expires_at: Instant::now() + self.ttl,
        };
        self.by_id.insert(receipt_id.to_string(), stored.clone());
        if let Some(chat_id) = chat_id.filter(|s| !s.is_empty()) {
            self.by_id.insert(chat_id.to_string(), stored);
        }
    }

    pub fn get(&self, id: &str) -> Option<Value> {
        let Some(entry) = self.by_id.get(id) else {
            return None;
        };
        if Instant::now() >= entry.expires_at {
            drop(entry);
            self.by_id.remove(id);
            return None;
        }
        Some(entry.document.clone())
    }

    fn evict_expired(&self) {
        let now = Instant::now();
        let stale: Vec<String> = self
            .by_id
            .iter()
            .filter(|e| now >= e.expires_at)
            .map(|e| e.key().clone())
            .collect();
        for key in stale {
            self.by_id.remove(&key);
        }
    }
}

pub fn new_receipt_id() -> String {
    let mut raw = [0u8; 12];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut raw);
    format!("{RECEIPT_ID_PREFIX}{}", hex::encode(raw))
}

#[derive(Debug, Clone)]
pub struct UpstreamVerification {
    pub result: &'static str,
    pub required: bool,
    pub model_id: String,
    pub provider: String,
    pub session_id: Option<String>,
    pub reason: Option<String>,
    pub upstream_receipt_id: Option<String>,
    pub upstream_identity: Option<String>,
}

impl UpstreamVerification {
    /// Honest aggregator default: we forwarded to a configured HTTP upstream
    /// and did not independently verify that upstream's TEE.
    pub fn unverified(model_id: &str, provider: &str) -> Self {
        Self {
            result: "failed",
            required: false,
            model_id: model_id.to_string(),
            provider: provider.to_string(),
            session_id: None,
            reason: Some("upstream_not_independently_verified".into()),
            upstream_receipt_id: None,
            upstream_identity: None,
        }
    }

    pub fn observed_aci_headers(
        model_id: &str,
        provider: &str,
        receipt_id: Option<String>,
        identity: Option<String>,
    ) -> Self {
        let mut v = Self::unverified(model_id, provider);
        v.upstream_receipt_id = receipt_id;
        v.upstream_identity = identity;
        if v.upstream_receipt_id.is_some() || v.upstream_identity.is_some() {
            v.reason = Some("upstream_aci_headers_observed_not_verified".into());
        }
        v
    }

    fn to_event(&self) -> Value {
        let mut ev = Map::new();
        ev.insert("type".into(), json!("upstream.verified"));
        ev.insert("result".into(), json!(self.result));
        ev.insert("required".into(), json!(self.required));
        ev.insert("model_id".into(), json!(self.model_id));
        ev.insert("provider".into(), json!(self.provider));
        if let Some(session_id) = &self.session_id {
            ev.insert("session_id".into(), json!(session_id));
        }
        if let Some(reason) = &self.reason {
            ev.insert("reason".into(), json!(reason));
        }
        if let Some(id) = &self.upstream_receipt_id {
            ev.insert("upstream_receipt_id".into(), json!(id));
        }
        if let Some(id) = &self.upstream_identity {
            ev.insert("upstream_identity".into(), json!(id));
        }
        Value::Object(ev)
    }
}

pub struct ReceiptInput<'a> {
    pub receipt_id: &'a str,
    pub chat_id: Option<&'a str>,
    pub model: Option<&'a str>,
    pub endpoint: &'a str,
    pub request_body: &'a [u8],
    pub forwarded_body: &'a [u8],
    pub response_body: &'a [u8],
    pub upstream: UpstreamVerification,
    pub solana_proof_address: Option<&'a str>,
    pub solana_tx_sig: Option<&'a str>,
    pub signed_payload_hash: Option<&'a str>,
}

/// Build and sign an ACI receipt. Extra Solana fields are implementation
/// events — ACI verifiers ignore unknown event types.
pub fn issue_receipt(
    identity: &AciIdentity,
    signing_key: &TeeSigningKey,
    input: ReceiptInput<'_>,
) -> Value {
    let request_hash = sha256_prefixed(input.request_body);
    let forwarded_hash = sha256_prefixed(input.forwarded_body);
    let response_hash = sha256_prefixed(input.response_body);

    let mut event_log = vec![
        json!({"type": "request.received", "body_hash": request_hash}),
        json!({"type": "request.forwarded", "body_hash": forwarded_hash}),
        input.upstream.to_event(),
    ];
    if input.solana_proof_address.is_some() || input.solana_tx_sig.is_some() {
        event_log.push(json!({
            "type": "solana.tee_proof_v2",
            "proof_address": input.solana_proof_address,
            "tx_sig": input.solana_tx_sig,
            "signed_payload_hash": input.signed_payload_hash,
        }));
    }
    event_log.push(json!({
        "type": "response.returned",
        "body_hash": response_hash,
    }));

    let mut unsigned = Map::new();
    unsigned.insert("api_version".into(), json!(ACI_VERSION));
    unsigned.insert("receipt_id".into(), json!(input.receipt_id));
    unsigned.insert(
        "chat_id".into(),
        match input.chat_id {
            Some(id) => json!(id),
            None => Value::Null,
        },
    );
    unsigned.insert("workload_id".into(), json!(identity.workload_id));
    unsigned.insert(
        "workload_keyset_digest".into(),
        json!(identity.keyset_digest),
    );
    unsigned.insert(
        "model".into(),
        match input.model {
            Some(m) => json!(m),
            None => Value::Null,
        },
    );
    unsigned.insert("endpoint".into(), json!(input.endpoint));
    unsigned.insert("method".into(), json!("POST"));
    unsigned.insert("served_at".into(), json!(unix_now()));
    unsigned.insert("event_log".into(), json!(event_log));
    unsigned.insert("key_id".into(), json!(identity.receipt_key_id));

    let canonical = jcs(&Value::Object(unsigned.clone()));
    let signature = identity.sign_receipt_jcs(signing_key, &canonical);
    unsigned.insert("signature".into(), json!(signature));
    Value::Object(unsigned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aci::sha256_prefixed;

    #[test]
    fn receipt_vector_jcs_and_body_hashes() {
        let request = br#"{"messages":[{"content":"hi","role":"user"}],"model":"demo-model"}"#;
        let response = br#"{"choices":[],"id":"chatcmpl-123"}"#;
        assert_eq!(
            sha256_prefixed(request),
            "sha256:94d809bf47380d8a2eab0eb6e126d4dda9364b0b4725cdf7ead52dd70b2aa87b"
        );
        assert_eq!(
            sha256_prefixed(response),
            "sha256:dedfffe5b14d031b8e2c01996d021a15293cb7c63b56be7e4be9e89b6f0a5f61"
        );

        let unsigned: Value = serde_json::from_str(
            r#"{
              "api_version": "aci/1",
              "receipt_id": "rcpt-0001",
              "chat_id": "chatcmpl-123",
              "model": "demo-model",
              "workload_keyset_digest": "sha256:53a5cd44b30dcc51999754c719f2628a041f174ecbf9662a6f8e898a10cd9371",
              "endpoint": "/v1/chat/completions",
              "method": "POST",
              "served_at": 1750000000,
              "event_log": [
                {"type":"request.received","body_hash":"sha256:94d809bf47380d8a2eab0eb6e126d4dda9364b0b4725cdf7ead52dd70b2aa87b"},
                {"type":"request.forwarded","body_hash":"sha256:94d809bf47380d8a2eab0eb6e126d4dda9364b0b4725cdf7ead52dd70b2aa87b"},
                {"type":"upstream.verified","model_id":"demo-model","required":true,"result":"verified","session_id":"95ad1cb4dd25445808c2e9d116caf420b05703730b506395e8fc1ca6faeae28f"},
                {"type":"response.returned","body_hash":"sha256:dedfffe5b14d031b8e2c01996d021a15293cb7c63b56be7e4be9e89b6f0a5f61"}
              ],
              "key_id": "receipt-1"
            }"#,
        )
        .unwrap();
        let expected = r#"{"api_version":"aci/1","chat_id":"chatcmpl-123","endpoint":"/v1/chat/completions","event_log":[{"body_hash":"sha256:94d809bf47380d8a2eab0eb6e126d4dda9364b0b4725cdf7ead52dd70b2aa87b","type":"request.received"},{"body_hash":"sha256:94d809bf47380d8a2eab0eb6e126d4dda9364b0b4725cdf7ead52dd70b2aa87b","type":"request.forwarded"},{"model_id":"demo-model","required":true,"result":"verified","session_id":"95ad1cb4dd25445808c2e9d116caf420b05703730b506395e8fc1ca6faeae28f","type":"upstream.verified"},{"body_hash":"sha256:dedfffe5b14d031b8e2c01996d021a15293cb7c63b56be7e4be9e89b6f0a5f61","type":"response.returned"}],"key_id":"receipt-1","method":"POST","model":"demo-model","receipt_id":"rcpt-0001","served_at":1750000000,"workload_keyset_digest":"sha256:53a5cd44b30dcc51999754c719f2628a041f174ecbf9662a6f8e898a10cd9371"}"#;
        assert_eq!(jcs(&unsigned), expected);
        assert_eq!(
            crate::aci::sha256_hex(jcs(&unsigned).as_bytes()),
            "1bd328e6880a5a12b3915af95ea32111310e04ab9e21ac3d71ce268e33b965c9"
        );
    }

    #[test]
    fn store_roundtrip_and_ttl() {
        let store = ReceiptStore::new(60, 16);
        store.insert("rcpt-abc", Some("chatcmpl-1"), json!({"ok": true}));
        assert!(store.get("rcpt-abc").is_some());
        assert!(store.get("chatcmpl-1").is_some());
        assert!(store.get("missing").is_none());
    }
}
