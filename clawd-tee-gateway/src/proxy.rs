//! Route handlers — proxies AI inference, attests, receipts, and anchors to Solana.

use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE},
        HeaderMap, HeaderName, HeaderValue, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use subtle::ConstantTimeEq;
use tracing::{error, info, warn};

use crate::{
    aci::{
        build_attestation_report, inference_attestation_report_hash, parse_aci_nonce,
        report_data_slot, ACI_VERSION, HEADER_ACI_IDENTITY, HEADER_ACI_KEYSET_DIGEST,
        HEADER_ACI_VERSION, HEADER_RECEIPT_ID,
    },
    attestation::{attest_gpu, collect_gpu_evidence, TdxBackend},
    error::{GatewayError, Result},
    receipts::{issue_receipt, new_receipt_id, ReceiptInput, UpstreamVerification},
    sas,
    signing::{compute_signed_payload_hash, random_nonce, sha256, sha256_str},
    solana::{anchor_proof_v2, lookup_proof_v2},
    types::{ChatRequest, ClawdProof},
    AppState,
};

// ---------------------------------------------------------------------------
// GET /health
// ---------------------------------------------------------------------------

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    Json(json!({
        "ok": true,
        "signing_address": state.signing_key.eth_address_hex(),
        "program_id": state.config.solana_program_id,
        "sas_initialized": state.sas_state.read().await.initialized,
        "sas_threshold": state.config.sas_proof_threshold,
        "api_version": ACI_VERSION,
        "workload_id": state.aci.workload_id,
        "workload_keyset_digest": state.aci.keyset_digest,
        "receipt_ttl_secs": state.config.receipt_ttl_secs,
        "solana_anchor_enabled": state.config.has_solana_keypair(),
        "serving": "aggregator",
    }))
}

// ---------------------------------------------------------------------------
// GET /v1/aci/attestation  (+ /v1/attestation, /v1/attestation/report)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AttestationQuery {
    pub nonce: Option<String>,
}

pub async fn get_attestation(
    State(state): State<AppState>,
    Query(query): Query<AttestationQuery>,
) -> Result<impl IntoResponse> {
    let nonce = match query.nonce.as_deref() {
        Some(raw) => Some(parse_aci_nonce(raw).map_err(GatewayError::BadRequest)?),
        None => None,
    };

    let tdx = TdxBackend::detect();
    let tdx_backend = tdx.label();
    let slot = report_data_slot(&state.aci.keyset_digest, nonce);
    let quote_bytes = tdx
        .get_quote(&slot)
        .map_err(|e| GatewayError::Attestation(format!("TDX quote: {e}")))?;
    let quote_hash = sha256(&quote_bytes);

    let gpu_nonce = match nonce {
        Some(hex_nonce) => {
            let bytes = hex::decode(hex_nonce).unwrap_or_default();
            let mut n = [0u8; 32];
            if bytes.len() == 32 {
                n.copy_from_slice(&bytes);
            }
            n
        }
        None => random_nonce(),
    };
    let gpu_evidence = collect_gpu_evidence(&gpu_nonce);
    let gpu_claims = attest_gpu(
        &state.config.nras_url,
        state.config.nvidia_api_key.as_deref(),
        gpu_evidence,
    )
    .await
    .map_err(|e| GatewayError::Attestation(format!("NRAS: {e}")))?;

    let report = build_attestation_report(
        &state.aci,
        &state.config,
        &state.signing_key.eth_address_hex(),
        nonce,
        &hex::encode(&quote_bytes),
        &hex::encode(quote_hash),
        gpu_claims
            .as_ref()
            .and_then(|c| serde_json::to_value(c).ok()),
        tdx_backend,
    );

    let mut response = Json(report).into_response();
    attach_aci_headers(
        &mut response,
        None,
        &state.aci.workload_id,
        &state.aci.keyset_digest,
    );
    Ok(response)
}

// ---------------------------------------------------------------------------
// GET /v1/aci/receipts/{receipt_id}
// ---------------------------------------------------------------------------

pub async fn get_receipt(
    State(state): State<AppState>,
    Path(receipt_id): Path<String>,
) -> Result<impl IntoResponse> {
    let Some(doc) = state.receipts.get(&receipt_id) else {
        return Err(GatewayError::NotFound(format!(
            "receipt {receipt_id} not found or expired"
        )));
    };
    let mut response = Json(doc).into_response();
    attach_aci_headers(
        &mut response,
        Some(&receipt_id),
        &state.aci.workload_id,
        &state.aci.keyset_digest,
    );
    Ok(response)
}

// ---------------------------------------------------------------------------
// POST /v1/chat/completions
// ---------------------------------------------------------------------------

pub async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let parsed: ChatRequest = serde_json::from_slice(&body)
        .map_err(|e| GatewayError::BadRequest(format!("invalid chat request: {e}")))?;
    handle_inference(state, headers, "/v1/chat/completions", body, parsed.model).await
}

// ---------------------------------------------------------------------------
// POST /v1/completions
// ---------------------------------------------------------------------------

pub async fn completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let parsed: Value = serde_json::from_slice(&body)
        .map_err(|e| GatewayError::BadRequest(format!("invalid completions request: {e}")))?;
    let model = parsed
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    handle_inference(state, headers, "/v1/completions", body, model).await
}

// ---------------------------------------------------------------------------
// POST /v1/embeddings
// ---------------------------------------------------------------------------

pub async fn embeddings(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response> {
    let parsed: Value = serde_json::from_slice(&body)
        .map_err(|e| GatewayError::BadRequest(format!("invalid embeddings request: {e}")))?;
    let model = parsed
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    handle_inference(state, headers, "/v1/embeddings", body, model).await
}

// ---------------------------------------------------------------------------
// GET /v1/proof/:payload_hash
// ---------------------------------------------------------------------------

pub async fn lookup_proof(
    State(state): State<AppState>,
    Path(payload_hash): Path<String>,
) -> Result<impl IntoResponse> {
    if payload_hash.len() != 64 || !payload_hash.as_bytes().iter().all(u8::is_ascii_hexdigit) {
        return Err(GatewayError::BadRequest(
            "payload_hash must be 64 hex chars".into(),
        ));
    }
    let hash_bytes = hex::decode(&payload_hash)
        .map_err(|_| GatewayError::BadRequest("payload_hash must be 64 hex chars".into()))?;
    if hash_bytes.len() != 32 {
        return Err(GatewayError::BadRequest(
            "payload_hash must be 32 bytes".into(),
        ));
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&hash_bytes);

    let proof = lookup_proof_v2(&state.config, &hash)
        .await
        .map_err(|e| GatewayError::Solana(e.to_string()))?;

    Ok(Json(json!({ "ok": proof.is_some(), "proof": proof })))
}

// ---------------------------------------------------------------------------
// POST /v1/credential/init  (admin — re-run SAS init)
// ---------------------------------------------------------------------------

pub async fn credential_init(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse> {
    require_admin(&headers, state.config.admin_token.as_deref())?;

    let new_state = sas::init_credential_and_schema(&state.config)
        .await
        .map_err(|e| GatewayError::Solana(e.to_string()))?;

    let resp = json!({
        "ok": true,
        "credential_pda": new_state.credential_pda.to_string(),
        "schema_pda": new_state.schema_pda.to_string(),
        "schema_mint_pda": new_state.schema_mint_pda.map(|p| p.to_string()),
    });

    *state.sas_state.write().await = new_state;
    Ok(Json(resp))
}

// ---------------------------------------------------------------------------
// POST /v1/credential/issue  { "wallet": "<base58>" }
// ---------------------------------------------------------------------------

pub async fn credential_issue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<impl IntoResponse> {
    require_admin(&headers, state.config.admin_token.as_deref())?;

    let wallet = body
        .get("wallet")
        .and_then(Value::as_str)
        .ok_or_else(|| GatewayError::BadRequest("missing `wallet` field".into()))?;
    let wallet = parse_wallet(wallet)?;

    let sas_guard = state.sas_state.read().await;
    if !sas_guard.initialized {
        return Err(GatewayError::BadRequest(
            "SAS not initialized — POST /v1/credential/init first".into(),
        ));
    }

    let count = sas::proof_count(&state.proof_tracker, &wallet);
    let tee_addr = state.signing_key.eth_address_hex();
    let last_hash = state
        .proof_tracker
        .get(&wallet)
        .map(|r| r.last_proof_hash.clone())
        .unwrap_or_default();

    let result = sas::issue_attestation(
        &state.config,
        &sas_guard,
        &wallet,
        count,
        &tee_addr,
        &last_hash,
    )
    .await
    .map_err(|e| GatewayError::Solana(e.to_string()))?;

    Ok(Json(serde_json::to_value(result).unwrap_or(Value::Null)))
}

// ---------------------------------------------------------------------------
// GET /v1/credential/:wallet
// ---------------------------------------------------------------------------

pub async fn credential_status(
    State(state): State<AppState>,
    Path(wallet): Path<String>,
) -> Result<impl IntoResponse> {
    let wallet = parse_wallet(&wallet)?;
    let sas_guard = state.sas_state.read().await;
    let result = sas::verify_attestation(&state.config, &sas_guard, &wallet, &state.proof_tracker)
        .await
        .map_err(|e| GatewayError::Solana(e.to_string()))?;

    Ok(Json(serde_json::to_value(result).unwrap_or(Value::Null)))
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

fn wallet_from_headers(headers: &HeaderMap) -> Result<Option<String>> {
    headers
        .get("x-clawd-wallet")
        .and_then(|v| v.to_str().ok())
        .map(parse_wallet)
        .transpose()
}

fn parse_wallet(wallet: &str) -> Result<String> {
    Pubkey::from_str(wallet)
        .map(|p| p.to_string())
        .map_err(|_| GatewayError::BadRequest("wallet must be a valid Solana pubkey".into()))
}

fn require_admin(headers: &HeaderMap, expected_token: Option<&str>) -> Result<()> {
    let Some(expected) = expected_token else {
        return Err(GatewayError::Forbidden(
            "admin endpoints are disabled until ADMIN_TOKEN is configured".into(),
        ));
    };

    let provided = headers
        .get("x-clawd-admin-token")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            headers
                .get(AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
        });

    let Some(provided) = provided else {
        return Err(GatewayError::Unauthorized("missing admin token".into()));
    };

    if provided.as_bytes().ct_eq(expected.as_bytes()).into() {
        Ok(())
    } else {
        Err(GatewayError::Unauthorized("invalid admin token".into()))
    }
}

struct UpstreamResponse {
    value: Value,
    receipt_id: Option<String>,
    identity: Option<String>,
}

async fn forward_request(
    state: &AppState,
    headers: &HeaderMap,
    path: &str,
    body: &[u8],
) -> Result<UpstreamResponse> {
    let client = reqwest::Client::new();
    let url = format!(
        "{}{}",
        state.config.upstream_url.trim_end_matches('/'),
        path
    );

    let caller_auth = if state.config.allow_client_authorization {
        headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    } else {
        None
    };
    let auth = match caller_auth {
        Some(auth) => auth,
        None => {
            let api_key = state.config.upstream_api_key.as_deref().ok_or_else(|| {
                GatewayError::BadRequest(
                    "UPSTREAM_API_KEY is not configured and client authorization is disabled"
                        .into(),
                )
            })?;
            format!("Bearer {api_key}")
        }
    };

    let resp = client
        .post(&url)
        .header("Authorization", auth)
        .header("Content-Type", "application/json")
        .body(body.to_vec())
        .send()
        .await
        .map_err(|e| GatewayError::Upstream(e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(GatewayError::Upstream(format!("HTTP {status}: {text}")));
    }

    let upstream_receipt = header_str(resp.headers(), HEADER_RECEIPT_ID);
    let upstream_identity = header_str(resp.headers(), HEADER_ACI_IDENTITY)
        .or_else(|| header_str(resp.headers(), HEADER_ACI_KEYSET_DIGEST));

    let value = resp
        .json::<Value>()
        .await
        .map_err(|e| GatewayError::Upstream(e.to_string()))?;

    Ok(UpstreamResponse {
        value,
        receipt_id: upstream_receipt,
        identity: upstream_identity,
    })
}

fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

struct InferenceEvidence {
    quote_hash: [u8; 32],
    report_hash: [u8; 32],
    gpu_verified: Option<bool>,
    tdx_backend: &'static str,
}

fn collect_inference_evidence(state: &AppState, nonce: &[u8; 32]) -> InferenceEvidence {
    let tdx = TdxBackend::detect();
    let tdx_backend = tdx.label();
    let nonce_hex = hex::encode(nonce);
    let slot = report_data_slot(&state.aci.keyset_digest, Some(&nonce_hex));

    match tdx.get_quote(&slot) {
        Ok(quote) if !quote.is_empty() => {
            let quote_hash = sha256(&quote);
            let report_hash = inference_attestation_report_hash(
                &quote,
                &slot,
                &state.signing_key.eth_address_hex(),
                None,
                tdx_backend,
            );
            if quote_hash == [0u8; 32] || report_hash == [0u8; 32] {
                error!("attestation hashes unexpectedly zero after a non-empty quote");
            }
            InferenceEvidence {
                quote_hash,
                report_hash,
                gpu_verified: None,
                tdx_backend,
            }
        }
        Ok(_) => {
            error!("TDX quote collection returned empty bytes — on-chain hashes will be zeros");
            InferenceEvidence {
                quote_hash: [0u8; 32],
                report_hash: [0u8; 32],
                gpu_verified: None,
                tdx_backend,
            }
        }
        Err(e) => {
            error!("TDX quote collection failed on inference path: {e:#}");
            InferenceEvidence {
                quote_hash: [0u8; 32],
                report_hash: [0u8; 32],
                gpu_verified: None,
                tdx_backend,
            }
        }
    }
}

async fn maybe_gpu_attest(state: &AppState, nonce: &[u8; 32]) -> Option<bool> {
    let evidence = collect_gpu_evidence(nonce);
    match attest_gpu(
        &state.config.nras_url,
        state.config.nvidia_api_key.as_deref(),
        evidence,
    )
    .await
    {
        Ok(Some(claims)) => Some(claims.overall_result),
        Ok(None) => None,
        Err(e) => {
            warn!("NRAS GPU attestation failed on inference path: {e:#}");
            None
        }
    }
}

async fn handle_inference(
    state: AppState,
    headers: HeaderMap,
    endpoint: &str,
    body: Bytes,
    model: String,
) -> Result<Response> {
    let wallet = wallet_from_headers(&headers)?;
    let upstream = forward_request(&state, &headers, endpoint, &body).await?;

    let mut proof =
        build_and_anchor_proof(&state, &body, &upstream.value, &model, wallet.as_deref()).await;

    let receipt_id = new_receipt_id();
    proof.receipt_id = Some(receipt_id.clone());

    let mut out = upstream.value;
    let chat_id = out.get("id").and_then(Value::as_str).map(str::to_string);
    out["_clawd"] = serde_json::to_value(&proof).unwrap_or(Value::Null);

    let response_bytes = serde_json::to_vec(&out).map_err(|e| GatewayError::Internal(e.into()))?;

    let provider = state.config.upstream_provider_name();
    let upstream_event = UpstreamVerification::observed_aci_headers(
        &model,
        &provider,
        upstream.receipt_id,
        upstream.identity,
    );

    let receipt = issue_receipt(
        &state.aci,
        &state.signing_key,
        ReceiptInput {
            receipt_id: &receipt_id,
            chat_id: chat_id.as_deref(),
            model: Some(&model),
            endpoint,
            request_body: &body,
            forwarded_body: &body,
            response_body: &response_bytes,
            upstream: upstream_event,
            solana_proof_address: proof.solana_proof_address.as_deref(),
            solana_tx_sig: proof.solana_tx_sig.as_deref(),
            signed_payload_hash: Some(&proof.signed_payload_hash),
        },
    );
    state
        .receipts
        .insert(&receipt_id, chat_id.as_deref(), receipt);

    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(response_bytes))
        .map_err(|e| GatewayError::Internal(anyhow::anyhow!(e)))?;
    attach_aci_headers(
        &mut response,
        Some(&receipt_id),
        &state.aci.workload_id,
        &state.aci.keyset_digest,
    );
    Ok(response)
}

fn attach_aci_headers(
    response: &mut Response,
    receipt_id: Option<&str>,
    identity: &str,
    keyset_digest: &str,
) {
    let headers = response.headers_mut();
    if let Some(id) = receipt_id {
        if let Ok(v) = HeaderValue::from_str(id) {
            headers.insert(HeaderName::from_static(HEADER_RECEIPT_ID), v);
        }
    }
    if let Ok(v) = HeaderValue::from_str(identity) {
        headers.insert(HeaderName::from_static(HEADER_ACI_IDENTITY), v);
    }
    if let Ok(v) = HeaderValue::from_str(keyset_digest) {
        headers.insert(HeaderName::from_static(HEADER_ACI_KEYSET_DIGEST), v);
    }
    if let Ok(v) = HeaderValue::from_str(ACI_VERSION) {
        headers.insert(HeaderName::from_static(HEADER_ACI_VERSION), v);
    }
}

async fn build_and_anchor_proof(
    state: &AppState,
    request_body: &[u8],
    response_value: &Value,
    model: &str,
    wallet: Option<&str>,
) -> ClawdProof {
    let nonce = random_nonce();
    let request_hash = sha256(request_body);
    let response_json = serde_json::to_string(response_value).unwrap_or_else(|_| "{}".into());
    let response_hash = sha256(response_json.as_bytes());
    let model_hash = sha256_str(model);

    let signed_payload_hash =
        compute_signed_payload_hash(&request_hash, &response_hash, &model_hash, &nonce);

    let signature_bytes = match state.signing_key.sign_hash(&signed_payload_hash) {
        Ok(s) => Some(s),
        Err(e) => {
            error!("signing failed: {e}");
            None
        }
    };
    let signature = signature_bytes
        .as_ref()
        .map(hex::encode)
        .unwrap_or_default();

    let mut evidence = collect_inference_evidence(state, &nonce);
    evidence.gpu_verified = maybe_gpu_attest(state, &nonce).await;

    let quote_hash = if evidence.quote_hash == [0u8; 32] {
        None
    } else {
        Some(evidence.quote_hash)
    };
    let report_hash = if evidence.report_hash == [0u8; 32] {
        None
    } else {
        Some(evidence.report_hash)
    };

    let (solana_proof_address, solana_tx_sig) = anchor_proof_v2(
        state,
        &signed_payload_hash,
        &request_hash,
        &response_hash,
        &model_hash,
        &nonce,
        quote_hash.as_ref(),
        report_hash.as_ref(),
        signature_bytes.as_ref(),
    )
    .await
    .map(|r| (Some(r.0), Some(r.1)))
    .unwrap_or_else(|e| {
        error!("Solana anchor failed: {e:?}");
        (None, None)
    });

    let anchor_succeeded = solana_tx_sig
        .as_deref()
        .map(|sig| !sig.is_empty())
        .unwrap_or(false);

    if anchor_succeeded {
        if let Some(w) = wallet {
            let payload_hash_hex = hex::encode(signed_payload_hash);
            let new_count = sas::record_proof(&state.proof_tracker, w, &payload_hash_hex);

            let threshold = state.config.sas_proof_threshold;
            if threshold > 0 && new_count >= threshold && new_count % threshold == 0 {
                let sas_guard = state.sas_state.read().await;
                if sas_guard.initialized {
                    let tee_addr = state.signing_key.eth_address_hex();
                    let state_clone = state.clone();
                    let wallet_str = w.to_string();
                    let hash_str = payload_hash_hex.clone();

                    tokio::spawn(async move {
                        let guard = state_clone.sas_state.read().await;
                        let sas_snap = guard.clone();
                        drop(guard);
                        match sas::issue_attestation(
                            &state_clone.config,
                            &sas_snap,
                            &wallet_str,
                            new_count,
                            &tee_addr,
                            &hash_str,
                        )
                        .await
                        {
                            Ok(r) => {
                                info!(wallet = %wallet_str, tx = %r.tx_sig, "SAS attestation auto-issued")
                            }
                            Err(e) => warn!(wallet = %wallet_str, "SAS auto-issue failed: {e}"),
                        }
                    });
                }
            }
        }
    }

    let signing_address = state.signing_key.eth_address_hex();

    info!(
        signed_payload_hash = %hex::encode(signed_payload_hash),
        signing_address = %signing_address,
        quote_hash = %hex::encode(evidence.quote_hash),
        attestation_report_hash = %hex::encode(evidence.report_hash),
        tdx_backend = evidence.tdx_backend,
        solana_tx = ?solana_tx_sig,
        wallet = ?wallet,
        "proof anchored"
    );

    ClawdProof {
        request_hash: hex::encode(request_hash),
        response_hash: hex::encode(response_hash),
        model_hash: hex::encode(model_hash),
        signed_payload_hash: hex::encode(signed_payload_hash),
        signing_address,
        signature,
        verification_level: state.config.verification_level,
        solana_proof_address,
        solana_tx_sig,
        tdx_quote_hash: Some(hex::encode(evidence.quote_hash)),
        gpu_verified: evidence.gpu_verified,
        nonce: hex::encode(nonce),
        receipt_id: None,
        attestation_report_hash: Some(hex::encode(evidence.report_hash)),
    }
}
