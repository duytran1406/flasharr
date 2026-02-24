//! Flasharr Domain Errors
//!
//! Typed error definitions for all application domains.
//! Replaces generic `anyhow` errors with specific, actionable error types.

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::fmt;

/// Main application error type
#[derive(Debug)]
pub enum FlasharrError {
    // Download errors
    DownloadNotFound(uuid::Uuid),
    DownloadAlreadyExists(String),
    DownloadInvalidState { id: uuid::Uuid, expected: String, actual: String },
    
    // Batch errors
    BatchNotFound(String),
    BatchEmpty(String),
    
    // Database errors
    Database(String),
    DatabaseConnection(String),
    
    // Host/Provider errors
    HostNotFound(String),
    HostAuthFailed(String),
    HostRateLimited { host: String, retry_after: Option<u64> },
    
    // Validation errors
    InvalidUuid(String),
    InvalidRequest(String),
    
    // External service errors
    TmdbError(String),
    FshareError(String),
    ArrServiceError { service: String, message: String },
    
    // Generic
    Internal(String),
}

impl fmt::Display for FlasharrError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DownloadNotFound(id) => write!(f, "Download not found: {}", id),
            Self::DownloadAlreadyExists(code) => write!(f, "Download already exists: {}", code),
            Self::DownloadInvalidState { id, expected, actual } => {
                write!(f, "Download {} in invalid state: expected {}, got {}", id, expected, actual)
            }
            Self::BatchNotFound(id) => write!(f, "Batch not found: {}", id),
            Self::BatchEmpty(id) => write!(f, "Batch is empty: {}", id),
            Self::Database(msg) => write!(f, "Database error: {}", msg),
            Self::DatabaseConnection(msg) => write!(f, "Database connection error: {}", msg),
            Self::HostNotFound(host) => write!(f, "Host not found: {}", host),
            Self::HostAuthFailed(host) => write!(f, "Host authentication failed: {}", host),
            Self::HostRateLimited { host, retry_after } => {
                if let Some(secs) = retry_after {
                    write!(f, "Host {} rate limited, retry after {}s", host, secs)
                } else {
                    write!(f, "Host {} rate limited", host)
                }
            }
            Self::InvalidUuid(s) => write!(f, "Invalid UUID: {}", s),
            Self::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            Self::TmdbError(msg) => write!(f, "TMDB error: {}", msg),
            Self::FshareError(msg) => write!(f, "Fshare error: {}", msg),
            Self::ArrServiceError { service, message } => {
                write!(f, "{} error: {}", service, message)
            }
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for FlasharrError {}

/// HTTP error response body
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

impl IntoResponse for FlasharrError {
    fn into_response(self) -> Response {
        let (status, code, message, details) = match &self {
            // 404 Not Found
            FlasharrError::DownloadNotFound(_) => (StatusCode::NOT_FOUND, "DOWNLOAD_NOT_FOUND", self.to_string(), None),
            FlasharrError::BatchNotFound(_) => (StatusCode::NOT_FOUND, "BATCH_NOT_FOUND", self.to_string(), None),
            FlasharrError::HostNotFound(_) => (StatusCode::NOT_FOUND, "HOST_NOT_FOUND", self.to_string(), None),
            
            // 400 Bad Request
            FlasharrError::InvalidUuid(_) => (StatusCode::BAD_REQUEST, "INVALID_UUID", self.to_string(), None),
            FlasharrError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "INVALID_REQUEST", self.to_string(), None),
            FlasharrError::BatchEmpty(_) => (StatusCode::BAD_REQUEST, "BATCH_EMPTY", self.to_string(), None),
            
            // 409 Conflict
            FlasharrError::DownloadAlreadyExists(_) => (StatusCode::CONFLICT, "DOWNLOAD_EXISTS", self.to_string(), None),
            FlasharrError::DownloadInvalidState { .. } => (StatusCode::CONFLICT, "INVALID_STATE", self.to_string(), None),
            
            // 401 Unauthorized
            FlasharrError::HostAuthFailed(_) => (StatusCode::UNAUTHORIZED, "AUTH_FAILED", self.to_string(), None),
            
            // 429 Too Many Requests
            FlasharrError::HostRateLimited { retry_after, .. } => {
                let msg = self.to_string();
                let details = retry_after.map(|s| format!("retry_after: {}", s));
                (StatusCode::TOO_MANY_REQUESTS, "RATE_LIMITED", msg, details)
            }
            
            // 503 Service Unavailable
            FlasharrError::TmdbError(_) => (StatusCode::SERVICE_UNAVAILABLE, "TMDB_ERROR", self.to_string(), None),
            FlasharrError::FshareError(_) => (StatusCode::SERVICE_UNAVAILABLE, "FSHARE_ERROR", self.to_string(), None),
            FlasharrError::ArrServiceError { .. } => (StatusCode::SERVICE_UNAVAILABLE, "ARR_ERROR", self.to_string(), None),
            FlasharrError::DatabaseConnection(_) => (StatusCode::SERVICE_UNAVAILABLE, "DB_UNAVAILABLE", self.to_string(), None),
            
            // 500 Internal Server Error
            FlasharrError::Database(_) => (StatusCode::INTERNAL_SERVER_ERROR, "DB_ERROR", self.to_string(), None),
            FlasharrError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", self.to_string(), None),
        };
        
        let body = ErrorResponse {
            error: message,
            code: code.to_string(),
            details,
        };
        
        (status, Json(body)).into_response()
    }
}

// Convenience conversions
impl From<rusqlite::Error> for FlasharrError {
    fn from(err: rusqlite::Error) -> Self {
        FlasharrError::Database(err.to_string())
    }
}

impl From<uuid::Error> for FlasharrError {
    fn from(err: uuid::Error) -> Self {
        FlasharrError::InvalidUuid(err.to_string())
    }
}

/// Result type alias for Flasharr operations
pub type FlasharrResult<T> = Result<T, FlasharrError>;
