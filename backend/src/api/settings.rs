//! Settings API Routes
//!
//! Application configuration management endpoints.

use axum::{
    routing::{get, put, post},
    Router,
    Json,
    extract::State,
    http::StatusCode,
};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_settings))
        .route("/", put(update_settings))
        .route("/downloads", get(get_downloads_settings))
        .route("/downloads", put(update_downloads_settings))
        .route("/sonarr", get(get_sonarr_settings))
        .route("/sonarr", put(update_sonarr_settings))
        .route("/sonarr/test", post(test_sonarr_connection))
        .route("/radarr", get(get_radarr_settings))
        .route("/radarr", put(update_radarr_settings))
        .route("/radarr/test", post(test_radarr_connection))
        .route("/indexer", get(get_indexer_settings))
        .route("/indexer", put(update_indexer_settings))
        .route("/indexer/generate-key", get(generate_api_key))
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Serialize)]
struct SettingsResponse {
    server: ServerSettings,
    downloads: DownloadsSettings,
    sonarr: Option<ArrSettings>,
    radarr: Option<ArrSettings>,
}

#[derive(Serialize)]
struct ServerSettings {
    host: String,
    port: u16,
}

#[derive(Serialize, Deserialize)]
struct DownloadsSettings {
    directory: String,
    max_concurrent: usize,
    segments_per_download: u32,
}

#[derive(Serialize, Deserialize)]
struct ArrSettings {
    enabled: bool,
    url: String,
    api_key: String,
    auto_import: bool,
}

#[derive(Serialize, Deserialize)]
struct IndexerSettings {
    enabled: bool,
    api_key: String,
    indexer_url: String,
}

#[derive(Serialize)]
struct GenerateKeyResponse {
    api_key: String,
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
struct UpdateSettingsRequest {
    #[allow(dead_code)]
    downloads: Option<DownloadsSettings>,
    #[allow(dead_code)]
    sonarr: Option<ArrSettings>,
    #[allow(dead_code)]
    radarr: Option<ArrSettings>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/settings - Get all settings
async fn get_settings(
    State(state): State<Arc<AppState>>,
) -> Json<SettingsResponse> {
    let config = &state.config;
    
    Json(SettingsResponse {
        server: ServerSettings {
            host: config.server.host.clone(),
            port: config.server.port,
        },
        downloads: DownloadsSettings {
            directory: config.downloads.directory.to_string_lossy().to_string(),
            max_concurrent: config.downloads.max_concurrent,
            segments_per_download: config.downloads.segments_per_download,
        },
        sonarr: config.sonarr.as_ref().map(|s| ArrSettings {
            enabled: s.enabled,
            url: s.url.clone(),
            api_key: mask_api_key(&s.api_key),
            auto_import: s.auto_import,
        }),
        radarr: config.radarr.as_ref().map(|r| ArrSettings {
            enabled: r.enabled,
            url: r.url.clone(),
            api_key: mask_api_key(&r.api_key),
            auto_import: r.auto_import,
        }),
    })
}

/// PUT /api/settings - Update settings
async fn update_settings(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<UpdateSettingsRequest>,
) -> Json<ActionResponse> {
    // TODO: Implement config file persistence
    Json(ActionResponse {
        success: false,
        message: Some("Settings update not yet implemented. Edit config.toml directly.".to_string()),
    })
}

/// GET /api/settings/downloads - Get download settings
async fn get_downloads_settings(
    State(state): State<Arc<AppState>>,
) -> Json<DownloadsSettings> {
    let config = state.download_orchestrator.get_config().await;
    
    Json(DownloadsSettings {
        directory: config.download_dir.to_string_lossy().to_string(),
        max_concurrent: config.max_concurrent,
        segments_per_download: config.segments_per_download as u32,
    })
}

/// PUT /api/settings/downloads - Update download settings
async fn update_downloads_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DownloadsSettings>,
) -> Json<ActionResponse> {
    // Get current config to preserve other fields
    let mut config = state.download_orchestrator.get_config().await;
    
    // Update fields
    config.max_concurrent = payload.max_concurrent;
    config.segments_per_download = payload.segments_per_download as usize;
    config.download_dir = std::path::PathBuf::from(payload.directory);
    
    // Push update to orchestrator
    state.download_orchestrator.update_config(config).await;
    
    Json(ActionResponse {
        success: true,
        message: Some("Settings updated successfully (runtime only, persistence not implemented)".to_string()),
    })
}

/// GET /api/settings/sonarr - Get Sonarr settings
async fn get_sonarr_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ArrSettings>, StatusCode> {
    state.config.sonarr.as_ref()
        .map(|s| Json(ArrSettings {
            enabled: s.enabled,
            url: s.url.clone(),
            api_key: mask_api_key(&s.api_key),
            auto_import: s.auto_import,
        }))
        .ok_or(StatusCode::NOT_FOUND)
}

/// PUT /api/settings/sonarr - Update Sonarr settings
async fn update_sonarr_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ArrSettings>,
) -> Json<ActionResponse> {
    // Update in-memory config
    let mut config = state.config.clone();
    config.sonarr = Some(crate::config::ArrConfig {
        enabled: payload.enabled,
        url: payload.url,
        api_key: payload.api_key,
        auto_import: payload.auto_import,
    });
    
    // Save to config.toml
    match crate::config::save_config(&config) {
        Ok(_) => {
            // Update AppState config
            // Note: This won't update the orchestrator's arr_client until restart
            // For full dynamic updates, we'd need to reload the orchestrator
            Json(ActionResponse {
                success: true,
                message: Some("Sonarr settings saved. Restart required for changes to take effect.".to_string()),
            })
        }
        Err(e) => {
            tracing::error!("Failed to save Sonarr settings: {}", e);
            Json(ActionResponse {
                success: false,
                message: Some(format!("Failed to save settings: {}", e)),
            })
        }
    }
}

/// GET /api/settings/radarr - Get Radarr settings
async fn get_radarr_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ArrSettings>, StatusCode> {
    state.config.radarr.as_ref()
        .map(|r| Json(ArrSettings {
            enabled: r.enabled,
            url: r.url.clone(),
            api_key: mask_api_key(&r.api_key),
            auto_import: r.auto_import,
        }))
        .ok_or(StatusCode::NOT_FOUND)
}

/// PUT /api/settings/radarr - Update Radarr settings
async fn update_radarr_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ArrSettings>,
) -> Json<ActionResponse> {
    // Update in-memory config
    let mut config = state.config.clone();
    config.radarr = Some(crate::config::ArrConfig {
        enabled: payload.enabled,
        url: payload.url,
        api_key: payload.api_key,
        auto_import: payload.auto_import,
    });
    
    // Save to config.toml
    match crate::config::save_config(&config) {
        Ok(_) => {
            // Update AppState config
            // Note: This won't update the orchestrator's arr_client until restart
            // For full dynamic updates, we'd need to reload the orchestrator
            Json(ActionResponse {
                success: true,
                message: Some("Radarr settings saved. Restart required for changes to take effect.".to_string()),
            })
        }
        Err(e) => {
            tracing::error!("Failed to save Radarr settings: {}", e);
            Json(ActionResponse {
                success: false,
                message: Some(format!("Failed to save settings: {}", e)),
            })
        }
    }
}

/// GET /api/settings/indexer - Get indexer settings
async fn get_indexer_settings(
    State(state): State<Arc<AppState>>,
) -> Result<Json<IndexerSettings>, StatusCode> {
    state.config.indexer.as_ref()
        .map(|i| {
            let host = &state.config.server.host;
            let port = state.config.server.port;
            let indexer_url = format!("http://{}:{}/api/indexer", host, port);
            
            Json(IndexerSettings {
                enabled: i.enabled,
                api_key: i.api_key.clone(),
                indexer_url,
            })
        })
        .ok_or(StatusCode::NOT_FOUND)
}

/// PUT /api/settings/indexer - Update indexer settings
async fn update_indexer_settings(
    State(_state): State<Arc<AppState>>,
    Json(_payload): Json<IndexerSettings>,
) -> Json<ActionResponse> {
    // TODO: Implement config file persistence
    Json(ActionResponse {
        success: false,
        message: Some("Indexer settings update not yet implemented. Edit config.toml directly.".to_string()),
    })
}

/// GET /api/settings/indexer/generate-key - Generate new API key
async fn generate_api_key() -> Json<GenerateKeyResponse> {
    use uuid::Uuid;
    
    // Generate a secure random API key
    let api_key = format!("flasharr_{}", Uuid::new_v4().to_string().replace("-", ""));
    
    Json(GenerateKeyResponse {
        api_key,
    })
}

// ============================================================================
// Helpers
// ============================================================================

fn mask_api_key(key: &str) -> String {
    if key.len() > 8 {
        format!("{}...{}", &key[..4], &key[key.len()-4..])
    } else {
        "****".to_string()
    }
}

/// POST /api/settings/sonarr/test - Test Sonarr connection
async fn test_sonarr_connection(
    Json(payload): Json<ArrSettings>,
) -> Result<Json<ActionResponse>, StatusCode> {
    match crate::arr::ArrClient::test_sonarr_connection(&payload.url, &payload.api_key).await {
        Ok(message) => Ok(Json(ActionResponse {
            success: true,
            message: Some(message),
        })),
        Err(e) => Ok(Json(ActionResponse {
            success: false,
            message: Some(format!("Connection failed: {}", e)),
        })),
    }
}

/// POST /api/settings/radarr/test - Test Radarr connection
async fn test_radarr_connection(
    Json(payload): Json<ArrSettings>,
) -> Result<Json<ActionResponse>, StatusCode> {
    match crate::arr::ArrClient::test_radarr_connection(&payload.url, &payload.api_key).await {
        Ok(message) => Ok(Json(ActionResponse {
            success: true,
            message: Some(message),
        })),
        Err(e) => Ok(Json(ActionResponse {
            success: false,
            message: Some(format!("Connection failed: {}", e)),
        })),
    }
}
