use crate::AppState;
use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct VerifyResponse {
    valid: bool,
}

/// Router for auth-related endpoints (behind auth middleware).
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/verify", get(verify_key))
}

/// GET /api/auth/verify — returns 200 { valid: true } if the API key is correct.
/// Since this route is behind auth_middleware, reaching here means the key passed.
async fn verify_key() -> Json<VerifyResponse> {
    Json(VerifyResponse { valid: true })
}

/// Middleware to validate API key
pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    // 1. Check for X-Api-Key header
    let mut provided_key = req
        .headers()
        .get("X-Api-Key")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    // 2. Fallback to apikey query parameter
    if provided_key.is_none() {
        provided_key = req
            .uri()
            .query()
            .and_then(|q| {
                q.split('&')
                    .find(|p| p.starts_with("apikey="))
                    .and_then(|p| p.split('=').nth(1))
            })
            .map(|s| s.to_string());
    }

    match provided_key {
        Some(key) if validate_api_key(&state, &key) => Ok(next.run(req).await),
        _ => {
            tracing::warn!("Unauthorized access attempt to {}", req.uri().path());
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

/// Shared validation logic
pub fn validate_api_key(state: &AppState, provided: &str) -> bool {
    // Get API key from database (where UI saves it)
    let config_key = state
        .db
        .get_setting("indexer_api_key")
        .ok()
        .flatten()
        .unwrap_or_else(|| "flasharr-default-key".to_string());

    !provided.is_empty() && provided == config_key
}
