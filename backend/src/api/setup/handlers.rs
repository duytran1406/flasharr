use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use once_cell::sync::Lazy;
use crate::AppState;

/// Shared HTTP client for all test connections (reuses connection pool)
static HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
});

// ============================================================================
// Response Types
// ============================================================================

#[derive(Serialize)]
pub struct SetupStatus {
    pub complete: bool,
}

#[derive(Serialize)]
pub struct TestResult {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Serialize)]
pub struct IndexerKeyResponse {
    pub api_key: String,
}

#[derive(Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Deserialize)]
pub struct FshareCredentials {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ArrConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct JellyfinConfig {
    pub url: String,
    pub api_key: String,
}

#[derive(Deserialize)]
pub struct CompleteSetupPayload {
    pub fshare: FshareCredentials,
    pub downloads: DownloadsConfig,
    pub sonarr: Option<ArrConfig>,
    pub radarr: Option<ArrConfig>,
    pub jellyfin: Option<JellyfinConfig>,
}

#[derive(Deserialize)]
pub struct DownloadsConfig {
    pub directory: String,
    pub max_concurrent: u32,
}

// ============================================================================
// API Handlers
// ============================================================================

/// GET /api/setup/status - Check if onboarding is complete
pub async fn get_setup_status(State(state): State<Arc<AppState>>) -> Json<SetupStatus> {
    let complete = state.db.is_onboarding_complete().unwrap_or(false);
    Json(SetupStatus { complete })
}

/// POST /api/setup/fshare - Validate and save Fshare credentials
pub async fn setup_fshare(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FshareCredentials>,
) -> Result<Json<TestResult>, StatusCode> {
    // Validate credentials by attempting to login directly
    let client = &*HTTP_CLIENT;
    
    match client
        .post("https://download.fsharegroup.site/api/user/login")
        .json(&serde_json::json!({
            "user_email": payload.email,
            "password": payload.password,
        }))
        .send()
        .await
    {
        Ok(response) => {
            if let Ok(data) = response.json::<serde_json::Value>().await {
                if data["code"] == 200 {
                    // Save credentials to database
                    if let Err(e) = state.db.save_fshare_credentials(&payload.email, &payload.password) {
                        tracing::error!("Failed to save Fshare credentials: {}", e);
                        return Ok(Json(TestResult {
                            success: false,
                            message: "Failed to save credentials".to_string(),
                            version: None,
                        }));
                    }

                    Ok(Json(TestResult {
                        success: true,
                        message: "Connected successfully".to_string(),
                        version: None,
                    }))
                } else {
                    let msg = data["msg"].as_str().unwrap_or("Invalid credentials");
                    Ok(Json(TestResult {
                        success: false,
                        message: msg.to_string(),
                        version: None,
                    }))
                }
            } else {
                Ok(Json(TestResult {
                    success: false,
                    message: "Failed to parse response".to_string(),
                    version: None,
                }))
            }
        }
        Err(e) => {
            tracing::error!("Fshare login failed: {}", e);
            Ok(Json(TestResult {
                success: false,
                message: format!("Connection failed: {}", e),
                version: None,
            }))
        }
    }
}

/// POST /api/setup/sonarr/test - Test Sonarr connection
pub async fn test_sonarr(Json(payload): Json<ArrConfig>) -> Json<TestResult> {
    // Test connection to Sonarr
    let client = &*HTTP_CLIENT;
    let url = format!("{}/api/v3/system/status", payload.url.trim_end_matches('/'));
    
    match client
        .get(&url)
        .header("X-Api-Key", &payload.api_key)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    let version = data["version"].as_str().map(|s| s.to_string());
                    Json(TestResult {
                        success: true,
                        message: "Connected to Sonarr".to_string(),
                        version,
                    })
                } else {
                    Json(TestResult {
                        success: true,
                        message: "Connected to Sonarr".to_string(),
                        version: None,
                    })
                }
            } else {
                Json(TestResult {
                    success: false,
                    message: format!("HTTP {}: Check API key", response.status()),
                    version: None,
                })
            }
        }
        Err(e) => Json(TestResult {
            success: false,
            message: format!("Connection failed: {}", e),
            version: None,
        }),
    }
}

/// POST /api/setup/radarr/test - Test Radarr connection
pub async fn test_radarr(Json(payload): Json<ArrConfig>) -> Json<TestResult> {
    // Test connection to Radarr (same API as Sonarr)
    let client = &*HTTP_CLIENT;
    let url = format!("{}/api/v3/system/status", payload.url.trim_end_matches('/'));
    
    match client
        .get(&url)
        .header("X-Api-Key", &payload.api_key)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    let version = data["version"].as_str().map(|s| s.to_string());
                    Json(TestResult {
                        success: true,
                        message: "Connected to Radarr".to_string(),
                        version,
                    })
                } else {
                    Json(TestResult {
                        success: true,
                        message: "Connected to Radarr".to_string(),
                        version: None,
                    })
                }
            } else {
                Json(TestResult {
                    success: false,
                    message: format!("HTTP {}: Check API key", response.status()),
                    version: None,
                })
            }
        }
        Err(e) => Json(TestResult {
            success: false,
            message: format!("Connection failed: {}", e),
            version: None,
        }),
    }
}

/// POST /api/setup/jellyfin/test - Test Jellyfin connection
pub async fn test_jellyfin(Json(payload): Json<JellyfinConfig>) -> Json<TestResult> {
    let client = &*HTTP_CLIENT;
    let url = format!("{}/System/Info", payload.url.trim_end_matches('/'));
    
    match client
        .get(&url)
        .header("X-Emby-Token", &payload.api_key)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    let version = data["Version"].as_str().map(|s| s.to_string());
                    Json(TestResult {
                        success: true,
                        message: "Connected to Jellyfin".to_string(),
                        version,
                    })
                } else {
                    Json(TestResult {
                        success: true,
                        message: "Connected to Jellyfin".to_string(),
                        version: None,
                    })
                }
            } else {
                Json(TestResult {
                    success: false,
                    message: format!("HTTP {}: Check API key", response.status()),
                    version: None,
                })
            }
        }
        Err(e) => Json(TestResult {
            success: false,
            message: format!("Connection failed: {}", e),
            version: None,
        }),
    }
}

/// GET /api/setup/indexer/key - Get or generate indexer API key
pub async fn get_indexer_key(State(state): State<Arc<AppState>>) -> Json<IndexerKeyResponse> {
    // Get existing key or generate new one
    let api_key = state.db.get_indexer_api_key()
        .unwrap_or_else(|_| {
            // Generate new key using UUID
            let key = uuid::Uuid::new_v4().to_string().replace("-", "");
            
            // Save to database
            let _ = state.db.save_indexer_api_key(&key);
            key
        });
    
    Json(IndexerKeyResponse { api_key })
}

/// POST /api/setup/complete - Save all settings and mark setup as complete
pub async fn complete_setup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CompleteSetupPayload>,
) -> Json<SuccessResponse> {
    // Save Fshare credentials
    if let Err(e) = state.db.save_fshare_credentials(&payload.fshare.email, &payload.fshare.password) {
        tracing::error!("Failed to save Fshare credentials: {}", e);
        return Json(SuccessResponse {
            success: false,
            message: "Failed to save Fshare credentials".to_string(),
        });
    }

    // Clear FShare session so it re-logs in with new credentials
    if let Some(handler) = state.host_registry.get_handler_for_url("https://fshare.vn/file/test") {
        if let Err(e) = handler.logout().await {
            tracing::warn!("Failed to clear FShare session: {}", e);
        } else {
            tracing::info!("Cleared FShare session, will re-login with new credentials");
        }
    }

    // Save download settings
    if let Err(e) = state.db.save_download_settings(&payload.downloads.directory, payload.downloads.max_concurrent) {
        tracing::error!("Failed to save download settings: {}", e);
        return Json(SuccessResponse {
            success: false,
            message: "Failed to save download settings".to_string(),
        });
    }

    // Collect arr configs for reload
    let mut sonarr_config = None;
    let mut radarr_config = None;

    // Save Sonarr config if provided
    if let Some(sonarr) = payload.sonarr {
        if let Err(e) = state.db.save_arr_config("sonarr", &sonarr.url, &sonarr.api_key) {
            tracing::error!("Failed to save Sonarr config: {}", e);
        } else {
            sonarr_config = Some(crate::config::ArrConfig {
                enabled: true,
                url: sonarr.url,
                api_key: sonarr.api_key,
                auto_import: true,
            });
        }
    }

    // Save Radarr config if provided
    if let Some(radarr) = payload.radarr {
        if let Err(e) = state.db.save_arr_config("radarr", &radarr.url, &radarr.api_key) {
            tracing::error!("Failed to save Radarr config: {}", e);
        } else {
            radarr_config = Some(crate::config::ArrConfig {
                enabled: true,
                url: radarr.url,
                api_key: radarr.api_key,
                auto_import: true,
            });
        }
    }

    // Reload arr_client with new configurations
    if sonarr_config.is_some() || radarr_config.is_some() {
        state.download_orchestrator.reload_arr_client(sonarr_config, radarr_config).await;
        tracing::info!("Reloaded *arr client with wizard settings");
    }

    // Save Jellyfin config if provided
    if let Some(jellyfin) = payload.jellyfin {
        if let Err(e) = state.db.save_jellyfin_config(&jellyfin.url, &jellyfin.api_key) {
            tracing::error!("Failed to save Jellyfin config: {}", e);
        }
    }

    // Mark onboarding as complete
    if let Err(e) = state.db.mark_onboarding_complete() {
        tracing::error!("Failed to mark onboarding complete: {}", e);
        return Json(SuccessResponse {
            success: false,
            message: "Failed to complete setup".to_string(),
        });
    }

    tracing::info!("Setup wizard completed successfully");
    Json(SuccessResponse {
        success: true,
        message: "Setup completed successfully".to_string(),
    })
}
