use anyhow::{bail, Context, Result};
use solana_sdk::signature::{read_keypair_file, Keypair};

#[derive(Debug, Clone)]
pub struct Config {
    pub port: u16,
    pub bind_addr: String,

    // Upstream AI provider
    pub upstream_url: String,
    pub upstream_api_key: Option<String>,
    /// Permit callers to supply their own Authorization header for upstream
    /// requests. Disabled by default so a public gateway has a single configured
    /// upstream identity and policy.
    pub allow_client_authorization: bool,
    /// Permit a non-HTTPS upstream URL. This should only be enabled for local
    /// development against loopback services.
    pub allow_insecure_upstream: bool,

    // Solana
    pub solana_rpc_url: String,
    pub solana_program_id: String,
    /// Path to a JSON-encoded 64-byte keypair for the Solana fee payer / submitter.
    /// If absent the gateway posts attestations but does NOT anchor to Solana.
    pub solana_keypair_path: Option<String>,
    /// JSON-encoded 64-byte keypair for platforms like Railway where secrets are
    /// environment variables rather than mounted files.
    pub solana_keypair_json: Option<String>,

    // NVIDIA attestation
    pub nvidia_api_key: Option<String>,
    /// NRAS endpoint; defaults to the public H100 endpoint.
    pub nras_url: String,

    // TEE signing key persistence (optional — ephemeral by default)
    /// PEM file path; if set the signing key is loaded/saved here across restarts.
    pub signing_key_pem: Option<String>,
    /// PKCS#8 PEM secret for platforms where the signing key is injected as an
    /// environment variable instead of a file.
    pub signing_key_pem_value: Option<String>,

    // Policy
    /// Maximum seconds before a stored proof expires. 0 = no expiry.
    pub proof_ttl_secs: u64,
    /// Verification level written into TeeProofV2 (0–5).
    pub verification_level: u8,

    // Solana Attestation Service (SAS)
    /// Number of verified TEE proofs a wallet must accumulate before
    /// a SAS attestation is issued to them. 0 = disabled.
    pub sas_proof_threshold: u64,
    /// Human-readable name for the SAS credential (max ~32 chars).
    pub sas_credential_name: String,
    /// Human-readable name for the SAS schema.
    pub sas_schema_name: String,
    /// Version byte for the SAS schema (bump when fields change).
    pub sas_schema_version: u8,
    /// Token name embedded in the SAS attestation NFT.
    pub sas_token_name: String,
    /// Token symbol embedded in the SAS attestation NFT.
    pub sas_token_symbol: String,
    /// Metadata URI for the SAS attestation NFT.
    pub sas_token_uri: String,
    /// Attestation validity in seconds (how long the SAS credential lasts).
    pub sas_expiry_secs: i64,

    // HTTP surface
    /// Required bearer token for admin-only credential endpoints.
    pub admin_token: Option<String>,
    /// Optional CORS origin. CORS is disabled unless this is set.
    pub cors_allowed_origin: Option<String>,
    /// Maximum JSON request body accepted by axum.
    pub max_request_bytes: usize,

    // ACI / RedPill compatibility
    /// How long signed receipts stay in memory for `GET /v1/aci/receipts/{id}`.
    pub receipt_ttl_secs: u64,
    /// Hard cap on in-memory receipts (expired entries evicted first).
    pub receipt_max_entries: usize,
    /// Keyset `subject` (naming metadata only — not a trust anchor).
    pub workload_subject: String,
    /// Seconds until the published ACI keyset `not_after`.
    pub keyset_ttl_secs: u64,
    /// Provenance labels published in the attestation report.
    pub source_repo_url: Option<String>,
    pub source_repo_commit: Option<String>,
    pub source_image_digest: Option<String>,
    /// Display name for `upstream.verified.provider` (defaults to upstream host).
    pub upstream_provider: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let config = Self {
            port: env_u16("PORT", 8080),
            bind_addr: env_str("BIND_ADDR", "0.0.0.0"),
            upstream_url: env_str("UPSTREAM_URL", "https://api.openai.com"),
            upstream_api_key: non_empty_env("UPSTREAM_API_KEY"),
            allow_client_authorization: env_bool("ALLOW_CLIENT_AUTHORIZATION", false),
            allow_insecure_upstream: env_bool("ALLOW_INSECURE_UPSTREAM", false),
            solana_rpc_url: env_str("SOLANA_RPC_URL", "https://api.devnet.solana.com"),
            solana_program_id: env_str(
                "REDPILL_PROGRAM_ID",
                "BnoUSPTE88ebzLb74RAnTogddDMzyBPtkmKxiRoRJG4L",
            ),
            solana_keypair_path: non_empty_env("SOLANA_KEYPAIR_PATH"),
            solana_keypair_json: non_empty_env("SOLANA_KEYPAIR_JSON"),
            nvidia_api_key: non_empty_env("NVIDIA_API_KEY"),
            nras_url: env_str(
                "NRAS_URL",
                "https://nras.attestation.nvidia.com/v4/attest/gpu",
            ),
            signing_key_pem: non_empty_env("SIGNING_KEY_PEM_PATH"),
            signing_key_pem_value: non_empty_env("SIGNING_KEY_PEM"),
            proof_ttl_secs: env_u64("PROOF_TTL_SECS", 86_400),
            verification_level: env_u8("VERIFICATION_LEVEL", 0),
            sas_proof_threshold: env_u64("SAS_PROOF_THRESHOLD", 0),
            sas_credential_name: env_str("SAS_CREDENTIAL_NAME", "CLAWD-TEE-GATEWAY"),
            sas_schema_name: env_str("SAS_SCHEMA_NAME", "CLAWD-TEE-PROOF-V1"),
            sas_schema_version: env_u8("SAS_SCHEMA_VERSION", 1),
            sas_token_name: env_str("SAS_TOKEN_NAME", "CLAWD TEE Verified"),
            sas_token_symbol: env_str("SAS_TOKEN_SYMBOL", "CLAWD-TEE"),
            sas_token_uri: env_str(
                "SAS_TOKEN_URI",
                "https://zk.x402.wtf/credential-metadata.json",
            ),
            sas_expiry_secs: env_u64("SAS_EXPIRY_SECS", 86_400 * 30) as i64,
            admin_token: non_empty_env("ADMIN_TOKEN"),
            cors_allowed_origin: non_empty_env("CORS_ALLOWED_ORIGIN"),
            max_request_bytes: env_usize("MAX_REQUEST_BYTES", 1024 * 1024),
            receipt_ttl_secs: env_u64("RECEIPT_TTL_SECS", 3600),
            receipt_max_entries: env_usize("RECEIPT_MAX_ENTRIES", 10_000),
            workload_subject: env_str("WORKLOAD_SUBJECT", "clawd-tee-gateway"),
            keyset_ttl_secs: env_u64("KEYSET_TTL_SECS", 86_400 * 365),
            source_repo_url: non_empty_env("SOURCE_REPO_URL")
                .or_else(|| Some("https://github.com/Solizardking/clawd-tee".into())),
            source_repo_commit: non_empty_env("SOURCE_REPO_COMMIT"),
            source_image_digest: non_empty_env("SOURCE_IMAGE_DIGEST"),
            upstream_provider: non_empty_env("UPSTREAM_PROVIDER"),
        };

        config.validate()?;
        Ok(config)
    }

    pub fn has_solana_keypair(&self) -> bool {
        self.solana_keypair_json.is_some() || self.solana_keypair_path.is_some()
    }

    pub fn load_solana_keypair(&self, purpose: &str) -> Result<Keypair> {
        if let Some(json) = &self.solana_keypair_json {
            return keypair_from_json(json)
                .with_context(|| format!("parsing SOLANA_KEYPAIR_JSON for {purpose}"));
        }

        if let Some(path) = &self.solana_keypair_path {
            return read_keypair_file(path)
                .map_err(|e| anyhow::anyhow!("reading keypair file for {purpose}: {e}"));
        }

        bail!("{purpose} requires SOLANA_KEYPAIR_JSON or SOLANA_KEYPAIR_PATH")
    }

    fn validate(&self) -> Result<()> {
        if self.verification_level > 5 {
            bail!("VERIFICATION_LEVEL must be between 0 and 5");
        }

        if !self.allow_insecure_upstream && !self.upstream_url.starts_with("https://") {
            bail!("UPSTREAM_URL must use https:// unless ALLOW_INSECURE_UPSTREAM=true");
        }

        if self.upstream_api_key.is_none() && !self.allow_client_authorization {
            bail!("UPSTREAM_API_KEY is required unless ALLOW_CLIENT_AUTHORIZATION=true");
        }

        if self.max_request_bytes == 0 {
            bail!("MAX_REQUEST_BYTES must be greater than zero");
        }

        if self.receipt_ttl_secs == 0 {
            bail!("RECEIPT_TTL_SECS must be greater than zero");
        }

        if self.receipt_max_entries == 0 {
            bail!("RECEIPT_MAX_ENTRIES must be greater than zero");
        }

        Ok(())
    }

    pub fn upstream_provider_name(&self) -> String {
        if let Some(name) = &self.upstream_provider {
            return name.clone();
        }
        extract_host(&self.upstream_url).unwrap_or_else(|| "upstream".into())
    }
}

fn extract_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next().unwrap_or(rest);
    let host = host.split('@').next_back().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

#[cfg(test)]
impl Config {
    pub fn test_upstream(upstream_url: &str) -> Self {
        Self {
            port: 0,
            bind_addr: "127.0.0.1".into(),
            upstream_url: upstream_url.into(),
            upstream_api_key: Some("test-upstream-key".into()),
            allow_client_authorization: false,
            allow_insecure_upstream: true,
            solana_rpc_url: "https://api.devnet.solana.com".into(),
            solana_program_id: "BnoUSPTE88ebzLb74RAnTogddDMzyBPtkmKxiRoRJG4L".into(),
            solana_keypair_path: None,
            solana_keypair_json: None,
            nvidia_api_key: None,
            nras_url: "https://nras.attestation.nvidia.com/v4/attest/gpu".into(),
            signing_key_pem: None,
            signing_key_pem_value: None,
            proof_ttl_secs: 86_400,
            verification_level: 0,
            sas_proof_threshold: 0,
            sas_credential_name: "CLAWD-TEE-GATEWAY".into(),
            sas_schema_name: "CLAWD-TEE-PROOF-V1".into(),
            sas_schema_version: 1,
            sas_token_name: "CLAWD TEE Verified".into(),
            sas_token_symbol: "CLAWD-TEE".into(),
            sas_token_uri: "https://zk.x402.wtf/credential-metadata.json".into(),
            sas_expiry_secs: 86_400 * 30,
            admin_token: Some("test-admin".into()),
            cors_allowed_origin: None,
            max_request_bytes: 1024 * 1024,
            receipt_ttl_secs: 3600,
            receipt_max_entries: 128,
            workload_subject: "clawd-tee-gateway-test".into(),
            keyset_ttl_secs: 86_400,
            source_repo_url: Some("https://github.com/Solizardking/clawd-tee".into()),
            source_repo_commit: Some("test".into()),
            source_image_digest: None,
            upstream_provider: None,
        }
    }
}

fn keypair_from_json(raw: &str) -> Result<Keypair> {
    let bytes: Vec<u8> = serde_json::from_str(raw.trim())
        .context("keypair secret must be a JSON array of 64 bytes")?;

    if bytes.len() != 64 {
        bail!("keypair secret must contain 64 bytes, got {}", bytes.len());
    }

    Keypair::try_from(bytes.as_slice())
        .map_err(|e| anyhow::anyhow!("invalid Solana keypair bytes: {e}"))
}

fn env_str(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u16(key: &str, default: u16) -> u16 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_u8(key: &str, default: u8) -> u8 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_usize(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_bool(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| {
            matches!(
                v.as_str(),
                "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
            )
        })
        .unwrap_or(default)
}

fn non_empty_env(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|v| {
        let trimmed = v.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

#[cfg(test)]
mod from_env_tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvRestore(Vec<(String, Option<String>)>);

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                match value {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    fn override_env(pairs: &[(&str, Option<&str>)]) -> EnvRestore {
        let saved = pairs
            .iter()
            .map(|(key, _)| ((*key).to_string(), std::env::var(key).ok()))
            .collect();
        for (key, value) in pairs {
            match value {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
        EnvRestore(saved)
    }

    #[test]
    fn from_env_reads_port_and_bind_addr() {
        let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = override_env(&[
            ("PORT", Some("19080")),
            ("BIND_ADDR", Some("127.0.0.1")),
            ("UPSTREAM_URL", Some("https://openrouter.ai/api")),
            ("UPSTREAM_API_KEY", Some("test-from-env-key")),
            ("ALLOW_CLIENT_AUTHORIZATION", Some("false")),
            ("ALLOW_INSECURE_UPSTREAM", Some("false")),
        ]);

        let config = Config::from_env().expect("Config::from_env");
        assert_eq!(config.port, 19080);
        assert_eq!(config.bind_addr, "127.0.0.1");
        assert_eq!(config.upstream_url, "https://openrouter.ai/api");
    }
}
