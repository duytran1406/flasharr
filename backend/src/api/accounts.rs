//! Accounts API Routes
//!
//! Fshare account management endpoints.

use axum::{
    routing::{get, post, delete},
    Router,
    Json,
    extract::{State, Path},
    http::StatusCode,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_accounts))
        .route("/", post(add_account))
        .route("/:email", delete(remove_account))
        .route("/:email/primary", post(set_primary))
        .route("/:email/refresh", post(refresh_account))
        .route("/verify", post(verify_account))
}

// ============================================================================
// Response Types
// ============================================================================

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
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Deserialize)]
struct AddAccountRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct VerifyAccountRequest {
    email: String,
    #[allow(dead_code)]
    password: String,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/accounts - List all accounts
/// Returns stored credentials immediately — no live login. Use POST /:email/refresh
/// to force a live status check.
async fn list_accounts(
    State(state): State<Arc<AppState>>,
) -> Json<AccountsResponse> {
    // Read email from DB first, then fall back to config.toml
    let email = match state.db.get_setting("fshare_email") {
        Ok(Some(e)) if !e.is_empty() => e,
        _ => {
            let config_email = state.config.fshare.email.clone();
            if config_email.is_empty() {
                return Json(AccountsResponse { accounts: vec![] });
            }
            config_email
        }
    };

    // Read cached rank from DB (written when refresh or login succeeds).
    // Empty string = never verified — frontend treats this as non-VIP.
    let rank = state.db
        .get_setting("fshare_rank")
        .ok()
        .flatten()
        .filter(|r| !r.is_empty())
        .unwrap_or_default(); // empty → frontend isVip() returns false

    let valid_until = state.db
        .get_setting("fshare_valid_until")
        .ok()
        .flatten()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let account = AccountInfo {
        email,
        rank,
        valid_until,
        quota_used: 0,
        quota_total: 0,
        is_active: true,
    };

    Json(AccountsResponse { accounts: vec![account] })
}

/// POST /api/accounts - Add new account
async fn add_account(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddAccountRequest>,
) -> Result<Json<ActionResponse>, StatusCode> {
    tracing::info!("Add account request for: {}", payload.email);
    
    // Save credentials to database (where FshareHandler reads from)
    if let Err(e) = state.db.save_setting("fshare_email", &payload.email) {
        tracing::error!("Failed to save fshare_email to database: {}", e);
        return Ok(Json(ActionResponse {
            success: false,
            message: Some(format!("Failed to save email: {}", e)),
        }));
    }
    
    if let Err(e) = state.db.save_setting("fshare_password", &payload.password) {
        tracing::error!("Failed to save fshare_password to database: {}", e);
        return Ok(Json(ActionResponse {
            success: false,
            message: Some(format!("Failed to save password: {}", e)),
        }));
    }
    
    // Clear stale rank so the old FREE/VIP badge doesn't linger
    let _ = state.db.save_setting("fshare_rank", "");
    let _ = state.db.save_setting("fshare_valid_until", "0");

    // Clear the existing session so the handler re-logs in with new credentials
    if let Some(handler) = state.host_registry.get_handler_for_url("https://fshare.vn/file/test") {
        if let Err(e) = handler.logout().await {
            tracing::warn!("Failed to clear session after adding account: {}", e);
        } else {
            tracing::info!("Cleared old session, will re-login with new credentials");
        }

        // Immediately verify the new credentials and cache the VIP rank.
        // check_account_status() calls ensure_valid_session() which triggers a fresh login
        // with the new credentials we just saved, then fetches /api/user/get.
        match handler.check_account_status().await {
            Ok(status) => {
                let rank = if status.premium { "VIP" } else { "FREE" };
                let _ = state.db.save_setting("fshare_rank", rank);
                let valid_until = status.valid_until.unwrap_or(0);
                let _ = state.db.save_setting("fshare_valid_until", &valid_until.to_string());
                tracing::info!("[Accounts] add_account: verified rank='{}' valid_until={}", rank, valid_until);
                return Ok(Json(ActionResponse {
                    success: true,
                    message: Some(format!("Account activated. Rank: {}", rank)),
                }));
            }
            Err(e) => {
                // Login failed — bad credentials or network issue
                tracing::warn!("[Accounts] add_account: status check failed: {}", e);
                // Credentials are saved; rank stays empty (UNVERIFIED) until user retries
                return Ok(Json(ActionResponse {
                    success: false,
                    message: Some(format!("Credentials saved but login failed: {}", e)),
                }));
            }
        }
    }
    
    tracing::info!("Fshare account added successfully for {}", payload.email);
    Ok(Json(ActionResponse {
        success: true,
        message: Some("Account added. No Fshare handler available to verify.".to_string()),
    }))
}

/// DELETE /api/accounts/:email - Remove account
async fn remove_account(
    State(state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Json<ActionResponse> {
    tracing::info!("Remove account request for: {}", email);
    
    // Check if the account matches the current one
    if email != state.config.fshare.email && !state.config.fshare.email.is_empty() {
        return Json(ActionResponse {
            success: false,
            message: Some("Account email mismatch".to_string()),
        });
    }

    // Reset Fshare configuration
    let mut new_config = state.config.clone();
    new_config.fshare.email = "".to_string();
    new_config.fshare.password = "".to_string();
    new_config.fshare.session_id = None;

    // Clear in-memory session if handler exists
    if let Some(handler) = state.host_registry.get_handler_for_url("https://fshare.vn/file/test") {
        if let Err(e) = handler.logout().await {
            tracing::warn!("Failed to clear in-memory session during logout: {}", e);
        }
    }

    // Save to config.toml
    match crate::config::save_config(&new_config) {
        Ok(_) => {
            tracing::info!("Fshare account removed successfully for {}", email);
            Json(ActionResponse {
                success: true,
                message: Some("Account removed and session cleared successfully.".to_string()),
            })
        }
        Err(e) => {
            tracing::error!("Failed to save config after account removal: {}", e);
            Json(ActionResponse {
                success: false,
                message: Some(format!("Failed to save configuration: {}", e)),
            })
        }
    }
}

/// POST /api/accounts/:email/primary - Set as primary account
async fn set_primary(
    State(_state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Json<ActionResponse> {
    tracing::info!("Set primary account request for: {}", email);
    
    Json(ActionResponse {
        success: false,
        message: Some("Multi-account management not yet implemented".to_string()),
    })
}

/// POST /api/accounts/:email/refresh - Refresh account session (live login check)
async fn refresh_account(
    State(state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Json<ActionResponse> {
    // Resolve the current account email from DB first, then fall back to config.
    // This matters when credentials were saved to DB via the UI (add_account)
    // but the in-memory config still holds the old/blank email.
    let current_email = state.db
        .get_setting("fshare_email")
        .ok()
        .flatten()
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| state.config.fshare.email.clone());

    if email != current_email {
        return Json(ActionResponse {
            success: false,
            message: Some("Account not found".to_string()),
        });
    }
    
    // Perform a live status check (this IS the slow call, but it's intentionally triggered by user)
    if let Some(handler) = state.host_registry.get_handler_for_url("https://fshare.vn/file/test") {
        match handler.check_account_status().await {
            Ok(status) => {
                // Cache the rank in DB so list_accounts returns it instantly next time
                let rank = if status.premium { "VIP" } else { "FREE" };
                let _ = state.db.save_setting("fshare_rank", rank);
                
                let valid_until = status.valid_until.unwrap_or(0);
                let _ = state.db.save_setting("fshare_valid_until", &valid_until.to_string());
                
                tracing::info!("[Accounts] Refreshed rank for {}: {} (valid_until: {})", status.account_email, rank, valid_until);
                Json(ActionResponse {
                    success: status.can_download,
                    message: if status.can_download { None } else { status.reason },
                })
            },
            Err(e) => Json(ActionResponse {
                success: false,
                message: Some(format!("Refresh failed: {}", e)),
            }),
        }
    } else {
        Json(ActionResponse {
            success: false,
            message: Some("Fshare handler not found".to_string()),
        })
    }
}

/// POST /api/accounts/verify - Verify credentials without adding
async fn verify_account(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<VerifyAccountRequest>,
) -> Json<ActionResponse> {
    // TODO: Implement actual verification by attempting login
    tracing::info!("Verify account request for: {}", payload.email);
    
    Json(ActionResponse {
        success: false,
        message: Some("Verification not yet implemented".to_string()),
    })
}
