//! Accounts API Routes
//!
//! Fshare account management endpoints.

use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_accounts))
        .route("/", post(add_account))
        .route("/:email", delete(remove_account))
        .route("/:email/refresh", post(refresh_account))
}

pub fn canonical_router() -> Router<Arc<AppState>> {
    router().route("/validate", post(add_account))
}

#[derive(Serialize)]
struct AccountInfo {
    email: String,
    rank: String,
    valid_until: u64,
    quota_used: u64,
    quota_total: u64,
    is_active: bool,
}

#[derive(Serialize)]
struct AccountsResponse {
    accounts: Vec<AccountInfo>,
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    persisted: Option<bool>,
}

#[derive(Deserialize)]
struct AddAccountRequest {
    email: String,
    password: String,
}

fn stored_email(state: &AppState) -> String {
    state
        .db
        .get_setting("fshare_email")
        .ok()
        .flatten()
        .filter(|email| !email.is_empty())
        .unwrap_or_else(|| state.config.fshare.email.clone())
}

/// GET /api/accounts - Return the stored account without a live login.
async fn list_accounts(State(state): State<Arc<AppState>>) -> Json<AccountsResponse> {
    let email = stored_email(&state);
    if email.is_empty() {
        return Json(AccountsResponse { accounts: vec![] });
    }

    let rank = state
        .db
        .get_setting("fshare_rank")
        .ok()
        .flatten()
        .filter(|rank| !rank.is_empty())
        .unwrap_or_default();
    let valid_until = state
        .db
        .get_setting("fshare_valid_until")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    Json(AccountsResponse {
        accounts: vec![AccountInfo {
            email,
            rank,
            valid_until,
            quota_used: 0,
            quota_total: 0,
            is_active: true,
        }],
    })
}

/// POST /api/accounts - Replace the one supported Fshare account.
async fn add_account(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddAccountRequest>,
) -> Result<Json<ActionResponse>, StatusCode> {
    let email = payload.email.trim();
    if email.is_empty() || payload.password.is_empty() || !email.contains('@') {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some("Enter a valid email and password".to_string()),
            persisted: None,
        }));
    }

    tracing::info!("Replacing Fshare account");
    for (key, value) in [
        ("fshare_email", email),
        ("fshare_password", payload.password.as_str()),
        ("fshare_rank", ""),
        ("fshare_valid_until", "0"),
    ] {
        if let Err(error) = state.db.save_setting(key, value) {
            tracing::error!("Failed to save {}: {}", key, error);
            return Ok(Json(ActionResponse {
                success: false,
                message: Some(format!("Failed to save account: {}", error)),
                persisted: None,
            }));
        }
    }

    let Some(handler) = state.host_registry.get_handler("fshare") else {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some("Credentials saved but verification is unavailable.".to_string()),
            persisted: Some(true),
        }));
    };

    if let Err(error) = handler.logout().await {
        tracing::warn!(
            "Failed to clear Fshare session after replacement: {}",
            error
        );
    }

    match handler.check_account_status().await {
        Ok(status) => {
            let rank = if status.premium { "VIP" } else { "FREE" };
            let valid_until = status.valid_until.unwrap_or(0);
            let _ = state.db.save_setting("fshare_rank", rank);
            let _ = state
                .db
                .save_setting("fshare_valid_until", &valid_until.to_string());
            Ok(Json(ActionResponse {
                success: true,
                message: Some(format!("Account verified. Rank: {}", rank)),
                persisted: Some(true),
            }))
        }
        Err(error) => {
            tracing::warn!("Fshare verification failed: {}", error);
            Ok(Json(ActionResponse {
                success: false,
                message: Some(format!(
                    "Credentials saved but verification failed: {}",
                    error
                )),
                persisted: Some(true),
            }))
        }
    }
}

/// DELETE /api/accounts/:email - Remove the stored Fshare account and session.
async fn remove_account(
    State(state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Json<ActionResponse> {
    let current_email = stored_email(&state);
    if current_email.is_empty() || email != current_email {
        return Json(ActionResponse {
            success: false,
            message: Some("Account not found".to_string()),
            persisted: None,
        });
    }

    if let Some(handler) = state.host_registry.get_handler("fshare") {
        if let Err(error) = handler.logout().await {
            tracing::warn!("Failed to clear Fshare session: {}", error);
        }
    }

    for (key, value) in [
        ("fshare_email", ""),
        ("fshare_password", ""),
        ("fshare_rank", ""),
        ("fshare_valid_until", "0"),
    ] {
        if let Err(error) = state.db.save_setting(key, value) {
            tracing::error!("Failed to clear {}: {}", key, error);
            return Json(ActionResponse {
                success: false,
                message: Some(format!("Failed to clear account: {}", error)),
                persisted: None,
            });
        }
    }

    let mut config = state.config.clone();
    config.fshare.email.clear();
    config.fshare.password.clear();
    config.fshare.session_id = None;
    if let Err(error) = crate::config::save_config(&config) {
        tracing::error!("Failed to save account removal: {}", error);
        return Json(ActionResponse {
            success: false,
            message: Some(format!(
                "Account cleared but configuration save failed: {}",
                error
            )),
            persisted: Some(false),
        });
    }

    Json(ActionResponse {
        success: true,
        message: Some("Account removed and session cleared.".to_string()),
        persisted: Some(false),
    })
}

/// POST /api/accounts/:email/refresh - Perform an explicit live status check.
async fn refresh_account(
    State(state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Json<ActionResponse> {
    if email != stored_email(&state) {
        return Json(ActionResponse {
            success: false,
            message: Some("Account not found".to_string()),
            persisted: None,
        });
    }

    let Some(handler) = state.host_registry.get_handler("fshare") else {
        return Json(ActionResponse {
            success: false,
            message: Some("Fshare handler not found".to_string()),
            persisted: None,
        });
    };

    match handler.check_account_status().await {
        Ok(status) => {
            let rank = if status.premium { "VIP" } else { "FREE" };
            let valid_until = status.valid_until.unwrap_or(0);
            let _ = state.db.save_setting("fshare_rank", rank);
            let _ = state
                .db
                .save_setting("fshare_valid_until", &valid_until.to_string());
            Json(ActionResponse {
                success: status.can_download,
                message: status.reason,
                persisted: None,
            })
        }
        Err(error) => Json(ActionResponse {
            success: false,
            message: Some(format!("Refresh failed: {}", error)),
            persisted: None,
        }),
    }
}
