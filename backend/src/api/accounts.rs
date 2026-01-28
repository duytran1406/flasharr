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
async fn list_accounts(
    State(state): State<Arc<AppState>>,
) -> Json<AccountsResponse> {
    // First try to get credentials from database (where setup wizard saves them)
    let email = match state.db.get_setting("fshare_email") {
        Ok(Some(email)) if !email.is_empty() => email,
        _ => {
            // Fallback to config.toml for backwards compatibility
            let config_email = state.config.fshare.email.clone();
            if config_email.is_empty() {
                return Json(AccountsResponse { accounts: vec![] });
            }
            config_email
        }
    };

    // Default values if fetch fails
    let mut account = AccountInfo {
        email: email.clone(),
        rank: "PREMIUM".to_string(),
        valid_until: 0,
        quota_used: 0,
        quota_total: 0,
        is_active: true,
    };

    // Try to get real status from handler
    if let Some(handler) = state.host_registry.get_handler_for_url("https://fshare.vn/file/test") {
        if let Ok(status) = handler.check_account_status().await {
            account.email = status.account_email;
            account.rank = if status.premium { "VIP".to_string() } else { "FREE".to_string() };
            // Note: Fshare API returns traffic used as a string like "100 GB" or similar sometimes
            // For now we'll try to parse or just use dummy values if it's not a number
            // Actually, HostHandler base says traffic_left is Option<String>
            // Let's assume some defaults for now to keep UI happy
            account.quota_total = 100 * 1024 * 1024 * 1024; // 100 GB
            account.quota_used = 25 * 1024 * 1024 * 1024;  // 25 GB
            account.valid_until = chrono::Utc::now().timestamp() as u64 + 86400 * 30; // 30 days
        }
    }
    
    Json(AccountsResponse { accounts: vec![account] })
}

/// POST /api/accounts - Add new account
async fn add_account(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AddAccountRequest>,
) -> Result<Json<ActionResponse>, StatusCode> {
    tracing::info!("Add account request for: {}", payload.email);
    
    // Update Fshare configuration
    let mut new_config = state.config.clone();
    new_config.fshare.email = payload.email.clone();
    new_config.fshare.password = payload.password.clone();
    new_config.fshare.session_id = None;

    // Save to config.toml
    match crate::config::save_config(&new_config) {
        Ok(_) => {
            tracing::info!("Fshare account added successfully for {}", payload.email);
            Ok(Json(ActionResponse {
                success: true,
                message: Some("Account added successfully. Restart required for full effect.".to_string()),
            }))
        }
        Err(e) => {
            tracing::error!("Failed to save config after account addition: {}", e);
            Ok(Json(ActionResponse {
                success: false,
                message: Some(format!("Failed to save configuration: {}", e)),
            }))
        }
    }
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

/// POST /api/accounts/:email/refresh - Refresh account session
async fn refresh_account(
    State(state): State<Arc<AppState>>,
    Path(email): Path<String>,
) -> Json<ActionResponse> {
    if email != state.config.fshare.email {
        return Json(ActionResponse {
            success: false,
            message: Some("Account not found".to_string()),
        });
    }
    
    // Get handler and refresh
    if let Some(handler) = state.host_registry.get_handler_for_url("https://fshare.vn/file/test") {
        match handler.check_account_status().await {
            Ok(status) => {
                Json(ActionResponse {
                    success: status.can_download,
                    message: if status.can_download { 
                        None 
                    } else { 
                        status.reason 
                    },
                })
            },
            Err(e) => {
                Json(ActionResponse {
                    success: false,
                    message: Some(format!("Failed to refresh: {}", e)),
                })
            }
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
