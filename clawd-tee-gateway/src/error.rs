use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GatewayError {
    #[error("upstream error: {0}")]
    Upstream(String),

    #[error("attestation failed: {0}")]
    Attestation(String),

    #[error("signing failed: {0}")]
    Signing(String),

    #[error("solana error: {0}")]
    Solana(String),

    #[error("invalid request: {0}")]
    BadRequest(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            GatewayError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            GatewayError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            GatewayError::Forbidden(_) => (StatusCode::FORBIDDEN, "forbidden"),
            GatewayError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            GatewayError::Upstream(_) => (StatusCode::BAD_GATEWAY, "upstream_error"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };

        let body = json!({
            "error": {
                "type": code,
                "message": self.to_string(),
            }
        });

        (status, Json(body)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, GatewayError>;
