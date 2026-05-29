use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Hex decoding error: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("R2D2 pool error: {0}")]
    Pool(#[from] r2d2::Error),

    #[error("Invalid public key length")]
    InvalidPublicKeyLength,

    #[error("Invalid gene configuration: {0}")]
    InvalidGeneConfiguration(String),
}

impl IntoResponse for SessionError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            SessionError::InvalidPublicKeyLength => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            ),
        };
        let body = Json(json!({
            "error": error_message
        }));
        (status, body).into_response()
    }
}

#[derive(Error, Debug)]
pub enum VerificationError {
    #[error("Session not found")]
    SessionNotFound,

    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Hex decoding error: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("Signature verification error: {0}")]
    Signature(String),

    #[error("Session has expired")]
    Expired,

    #[error("Chain is broken")]
    ChainBroken,

    #[error("Timestamp drift exceeded threshold")]
    TimestampDrift,

    #[error("Trust criteria failed: {0}")]
    TrustFailed(String),

    #[error("Fingerprint validation failed: {0}")]
    FingerprintFailed(String),

    #[error("Mutation step mismatch: expected {expected}, got {got}")]
    MutationStepMismatch { expected: u64, got: u64 },

    #[error("Mutation commitment mismatch")]
    MutationCommitmentMismatch,

    #[error("Mutation program error: {0}")]
    MutationProgram(String),

    #[error("Gene state error: {0}")]
    GeneState(String),
}
