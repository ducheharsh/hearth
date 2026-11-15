//! API error type: every handler returns `Result<_, ApiError>`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0} not found")]
    NotFound(String),
    #[error("{0}")]
    BadRequest(String),
    #[error("unauthorized")]
    Unauthorized,
    /// Refused with a structured body (e.g. a capacity verdict).
    #[error("conflict")]
    Conflict(serde_json::Value),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = match self {
            Self::NotFound(what) => (
                StatusCode::NOT_FOUND,
                json!({ "error": format!("{what} not found") }),
            ),
            Self::BadRequest(msg) => (StatusCode::BAD_REQUEST, json!({ "error": msg })),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                json!({ "error": "sign in through the gateway" }),
            ),
            Self::Conflict(detail) => (StatusCode::CONFLICT, detail),
            Self::Internal(e) => {
                tracing::error!(error = ?e, "request failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({ "error": format!("{e:#}") }),
                )
            }
        };
        (status, Json(body)).into_response()
    }
}

/// Convert any library error into a 500.
pub fn internal<E: Into<anyhow::Error>>(e: E) -> ApiError {
    ApiError::Internal(e.into())
}
