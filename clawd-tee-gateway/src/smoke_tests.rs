//! End-to-end smoke: ACI headers, nonce binding, receipt lookup, mocked anchor.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    extract::Json,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Router,
};
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::{
    aci::{
        parse_aci_nonce, report_data_digest, AciIdentity, HEADER_ACI_IDENTITY,
        HEADER_ACI_KEYSET_DIGEST, HEADER_ACI_VERSION, HEADER_RECEIPT_ID,
    },
    config::Config,
    receipts::ReceiptStore,
    sas::{new_proof_tracker, SasState},
    signing::TeeSigningKey,
    AppState,
};

async fn mock_chat(Json(body): Json<Value>) -> impl IntoResponse {
    let model = body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("mock-model");
    (
        StatusCode::OK,
        [(HEADER_RECEIPT_ID, "rcpt-upstream-observed")],
        Json(json!({
            "id": "chatcmpl-smoke-1",
            "object": "chat.completion",
            "model": model,
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "attested"},
                "finish_reason": "stop"
            }],
        })),
    )
}

async fn spawn_mock_upstream() -> (SocketAddr, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/v1/chat/completions", post(mock_chat))
        .route("/v1/completions", post(mock_chat))
        .route("/v1/embeddings", post(mock_chat));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().expect("addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("mock serve");
    });
    (addr, handle)
}

fn test_state(upstream: &str) -> AppState {
    let config = Arc::new(Config::test_upstream(upstream));
    let signing_key = Arc::new(TeeSigningKey::generate_or_load(&config).expect("signing key"));
    let aci = Arc::new(AciIdentity::from_signing_key(&config, &signing_key));
    AppState {
        receipts: ReceiptStore::new(config.receipt_ttl_secs, config.receipt_max_entries),
        signing_key,
        aci,
        config,
        sas_state: Arc::new(tokio::sync::RwLock::new(SasState::uninitialized())),
        proof_tracker: new_proof_tracker(),
    }
}

async fn spawn_gateway(state: AppState) -> (String, tokio::task::JoinHandle<()>) {
    let config = state.config.clone();
    let app = crate::build_router(state, &config);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind gw");
    let addr = listener.local_addr().expect("gw addr");
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.expect("gw serve");
    });
    (format!("http://{addr}"), handle)
}

#[tokio::test]
async fn attestation_nonce_binding_and_receipt_headers() {
    let (upstream_addr, _up) = spawn_mock_upstream().await;
    let state = test_state(&format!("http://{upstream_addr}"));
    let expected_digest = state.aci.keyset_digest.clone();
    let expected_identity = state.aci.workload_id.clone();
    let (base, _gw) = spawn_gateway(state).await;
    let client = reqwest::Client::new();

    let health = client
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("health");
    assert_eq!(health.status(), StatusCode::OK);
    let health_json: Value = health.json().await.expect("health json");
    assert_eq!(health_json["ok"], true);
    assert_eq!(health_json["workload_id"], expected_identity);
    assert_eq!(health_json["api_version"], "aci/1");
    assert_eq!(health_json["solana_anchor_enabled"], false);

    let nonce = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
    parse_aci_nonce(nonce).expect("test nonce");

    let att = client
        .get(format!("{base}/v1/aci/attestation?nonce={nonce}"))
        .send()
        .await
        .expect("attestation");
    assert_eq!(att.status(), StatusCode::OK);
    assert_eq!(
        att.headers()
            .get(HEADER_ACI_KEYSET_DIGEST)
            .and_then(|v| v.to_str().ok()),
        Some(expected_digest.as_str())
    );
    assert_eq!(
        att.headers()
            .get(HEADER_ACI_IDENTITY)
            .and_then(|v| v.to_str().ok()),
        Some(expected_identity.as_str())
    );
    assert_eq!(
        att.headers()
            .get(HEADER_ACI_VERSION)
            .and_then(|v| v.to_str().ok()),
        Some("aci/1")
    );

    let report: Value = att.json().await.expect("report json");
    assert_eq!(report["api_version"], "aci/1");
    assert_eq!(report["workload_keyset_digest"], expected_digest);
    assert_eq!(report["workload_id"], expected_identity);
    let expected_rd = hex::encode(report_data_digest(&expected_digest, Some(nonce)));
    let report_data = report["attestation"]["report_data"].as_str().unwrap();
    assert!(report_data.starts_with(&expected_rd));
    assert_eq!(report_data.len(), 128);
    assert!(report_data.ends_with(&"0".repeat(64)));
    assert_eq!(
        report["attestation"]["evidence"]["quote_report_data"],
        report["attestation"]["report_data"]
    );

    let alias = client
        .get(format!("{base}/v1/attestation?nonce={nonce}"))
        .send()
        .await
        .expect("alias");
    assert_eq!(alias.status(), StatusCode::OK);
    let alias_json: Value = alias.json().await.expect("alias json");
    assert_eq!(alias_json["attestation"]["report_data"], report_data);

    let bad = client
        .get(format!("{base}/v1/aci/attestation?nonce=ABCD"))
        .send()
        .await
        .expect("bad nonce");
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);

    let request_body = json!({
        "model": "smoke/model",
        "messages": [{"role": "user", "content": "ping"}]
    });
    let chat = client
        .post(format!("{base}/v1/chat/completions"))
        .json(&request_body)
        .send()
        .await
        .expect("chat");
    assert_eq!(chat.status(), StatusCode::OK);
    let headers: HeaderMap = chat.headers().clone();
    let receipt_id = headers
        .get(HEADER_RECEIPT_ID)
        .and_then(|v| v.to_str().ok())
        .expect("x-receipt-id")
        .to_string();
    assert!(receipt_id.starts_with("rcpt-"));
    assert_eq!(
        headers
            .get(HEADER_ACI_IDENTITY)
            .and_then(|v| v.to_str().ok()),
        Some(expected_identity.as_str())
    );
    assert_eq!(
        headers
            .get(HEADER_ACI_KEYSET_DIGEST)
            .and_then(|v| v.to_str().ok()),
        Some(expected_digest.as_str())
    );

    let body: Value = chat.json().await.expect("chat json");
    assert_eq!(body["id"], "chatcmpl-smoke-1");
    assert!(body["_clawd"]["signed_payload_hash"].as_str().is_some());
    let quote_hash = body["_clawd"]["tdx_quote_hash"].as_str().unwrap();
    assert_eq!(quote_hash.len(), 64);
    assert_ne!(quote_hash, &"0".repeat(64));
    let report_hash = body["_clawd"]["attestation_report_hash"].as_str().unwrap();
    assert_eq!(report_hash.len(), 64);
    assert_ne!(report_hash, &"0".repeat(64));
    assert_eq!(body["_clawd"]["receipt_id"], receipt_id);
    // No keypair: PDA is computed, tx sig is empty — skip is logged, not silent.
    assert!(body["_clawd"]["solana_proof_address"].as_str().is_some());
    let tx = body["_clawd"]["solana_tx_sig"].as_str().unwrap_or("");
    assert!(tx.is_empty());

    let receipt_resp = client
        .get(format!("{base}/v1/aci/receipts/{receipt_id}"))
        .send()
        .await
        .expect("receipt");
    assert_eq!(receipt_resp.status(), StatusCode::OK);
    let receipt: Value = receipt_resp.json().await.expect("receipt json");
    assert_eq!(receipt["receipt_id"], receipt_id);
    assert_eq!(receipt["workload_id"], expected_identity);
    assert_eq!(receipt["workload_keyset_digest"], expected_digest);
    assert_eq!(receipt["endpoint"], "/v1/chat/completions");
    let events = receipt["event_log"].as_array().expect("event_log");
    assert_eq!(events[0]["type"], "request.received");
    let upstream = events
        .iter()
        .find(|e| e["type"] == "upstream.verified")
        .expect("upstream.verified");
    assert_eq!(upstream["result"], "failed");
    assert_eq!(upstream["required"], false);
    assert_eq!(upstream["upstream_receipt_id"], "rcpt-upstream-observed");
    assert!(events.iter().any(|e| e["type"] == "response.returned"));
    assert!(events.iter().any(|e| e["type"] == "solana.tee_proof_v2"));
    assert_eq!(receipt["signature"].as_str().unwrap().len(), 130);

    let by_chat = client
        .get(format!("{base}/v1/aci/receipts/chatcmpl-smoke-1"))
        .send()
        .await
        .expect("by chat id");
    assert_eq!(by_chat.status(), StatusCode::OK);

    let missing = client
        .get(format!("{base}/v1/aci/receipts/rcpt-missing"))
        .send()
        .await
        .expect("missing");
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let embeddings = client
        .post(format!("{base}/v1/embeddings"))
        .json(&json!({"model": "smoke/embed", "input": "hi"}))
        .send()
        .await
        .expect("embeddings");
    assert_eq!(embeddings.status(), StatusCode::OK);
    assert!(embeddings.headers().get(HEADER_RECEIPT_ID).is_some());
}
