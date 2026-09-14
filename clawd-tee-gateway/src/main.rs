mod aci;
mod attestation;
mod config;
mod error;
mod proxy;
mod receipts;
mod sas;
mod signing;
mod solana;
mod types;

use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderName, HeaderValue, Method,
    },
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, timeout::TimeoutLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{fmt, EnvFilter};

use crate::{
    aci::{
        AciIdentity, HEADER_ACI_IDENTITY, HEADER_ACI_KEYSET_DIGEST, HEADER_ACI_VERSION,
        HEADER_RECEIPT_ID,
    },
    config::Config,
    receipts::ReceiptStore,
    sas::{new_proof_tracker, ProofTracker, SasState},
    signing::TeeSigningKey,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub signing_key: Arc<TeeSigningKey>,
    pub aci: Arc<AciIdentity>,
    pub receipts: ReceiptStore,
    pub sas_state: Arc<tokio::sync::RwLock<SasState>>,
    pub proof_tracker: ProofTracker,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Err(err) = run().await {
        eprintln!("CLAWD TEE Gateway startup failed: {err:#}");
        return Err(err);
    }

    Ok(())
}

async fn run() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .init();

    let config = Arc::new(Config::from_env()?);
    let signing_key = Arc::new(TeeSigningKey::generate_or_load(&config)?);
    let aci = Arc::new(AciIdentity::from_signing_key(&config, &signing_key));
    let receipts = ReceiptStore::new(config.receipt_ttl_secs, config.receipt_max_entries);

    info!(
        signing_address = %signing_key.eth_address_hex(),
        program_id = %config.solana_program_id,
        rpc = %config.solana_rpc_url,
        workload_id = %aci.workload_id,
        keyset_digest = %aci.keyset_digest,
        receipt_ttl_secs = config.receipt_ttl_secs,
        solana_anchor_enabled = config.has_solana_keypair(),
        "CLAWD TEE Gateway starting"
    );

    if !config.has_solana_keypair() {
        tracing::warn!(
            "SOLANA_KEYPAIR_PATH / SOLANA_KEYPAIR_JSON unset — inference will attest and issue receipts but will not submit TeeProofV2"
        );
    }

    // Attempt SAS init at startup (non-fatal — gateway still serves without it)
    let sas_state = if config.sas_proof_threshold > 0 && config.has_solana_keypair() {
        match sas::init_credential_and_schema(&config).await {
            Ok(s) => {
                info!(
                    credential = %s.credential_pda,
                    schema = %s.schema_pda,
                    "SAS credential and schema ready"
                );
                s
            }
            Err(e) => {
                tracing::warn!("SAS init failed (will retry on first issuance): {e}");
                SasState::uninitialized()
            }
        }
    } else {
        info!("SAS disabled (threshold=0 or no keypair)");
        SasState::uninitialized()
    };

    let state = AppState {
        config: config.clone(),
        signing_key,
        aci,
        receipts,
        sas_state: Arc::new(tokio::sync::RwLock::new(sas_state)),
        proof_tracker: new_proof_tracker(),
    };

    let cors = build_cors_layer(&config)?;
    let mut app = build_router(state, &config);
    if let Some(cors) = cors {
        app = app.layer(cors);
    }

    let addr = format!("{}:{}", config.bind_addr, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(addr = %addr, "listening");

    axum::serve(listener, app).await?;
    Ok(())
}

pub(crate) fn build_router(state: AppState, config: &Config) -> Router {
    Router::new()
        .route("/health", get(proxy::health))
        // ACI surface (RedPill / private-ai-gateway)
        .route("/v1/aci/attestation", get(proxy::get_attestation))
        .route("/v1/aci/receipts/:receipt_id", get(proxy::get_receipt))
        // Compatibility aliases
        .route("/v1/attestation", get(proxy::get_attestation))
        .route("/v1/attestation/report", get(proxy::get_attestation))
        // OpenAI-compatible inference (proxied + attested + receipted)
        .route("/v1/chat/completions", post(proxy::chat_completions))
        .route("/v1/completions", post(proxy::completions))
        .route("/v1/embeddings", post(proxy::embeddings))
        // Proof lookup
        .route("/v1/proof/:payload_hash", get(proxy::lookup_proof))
        // SAS credential management
        .route("/v1/credential/init", post(proxy::credential_init))
        .route("/v1/credential/issue", post(proxy::credential_issue))
        .route("/v1/credential/:wallet", get(proxy::credential_status))
        .layer(DefaultBodyLimit::max(config.max_request_bytes))
        .layer(TraceLayer::new_for_http())
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(300)))
        .with_state(state)
}

fn build_cors_layer(config: &Config) -> anyhow::Result<Option<CorsLayer>> {
    let Some(origin) = config.cors_allowed_origin.as_deref() else {
        return Ok(None);
    };

    let origin = HeaderValue::from_str(origin)?;
    Ok(Some(
        CorsLayer::new()
            .allow_origin(origin)
            .allow_methods([Method::GET, Method::POST])
            .allow_headers([
                AUTHORIZATION,
                CONTENT_TYPE,
                HeaderName::from_static("x-clawd-wallet"),
                HeaderName::from_static("x-clawd-admin-token"),
            ])
            .expose_headers([
                HeaderName::from_static(HEADER_RECEIPT_ID),
                HeaderName::from_static(HEADER_ACI_IDENTITY),
                HeaderName::from_static(HEADER_ACI_KEYSET_DIGEST),
                HeaderName::from_static(HEADER_ACI_VERSION),
            ]),
    ))
}

#[cfg(test)]
mod smoke_tests;
