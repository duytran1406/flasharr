//! Settings API Routes
//!
//! Application configuration management endpoints.

use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_settings))
        .route("/", put(update_settings))
        .route("/downloads", get(get_downloads_settings))
        .route("/downloads", put(update_downloads_settings))
        .route("/storage", get(get_storage_status))
        .route("/sonarr", get(get_sonarr_settings))
        .route("/sonarr", put(update_sonarr_settings))
        .route("/sonarr/test", post(test_sonarr_connection))
        .route("/radarr", get(get_radarr_settings))
        .route("/radarr", put(update_radarr_settings))
        .route("/radarr/test", post(test_radarr_connection))
        .route("/integration-status", get(get_integration_status))
        .route("/indexer", get(get_indexer_settings))
        .route("/indexer", put(update_indexer_settings))
        .route("/indexer/generate", post(generate_indexer_api_key))
        .route("/prowlarr", get(get_prowlarr_settings))
        .route("/prowlarr", put(update_prowlarr_settings))
        .route("/prowlarr/test", post(test_prowlarr_connection))
        .route("/torbox", get(get_torbox_settings))
        .route("/torbox", put(update_torbox_settings))
        .route("/torbox", delete(clear_torbox_settings))
        .route("/torbox/test", post(test_torbox_connection))
        .route("/torbox/profile", get(get_torbox_profile))
        .route("/qbittorrent", get(get_qbittorrent_settings))
        .route("/qbittorrent", put(update_qbittorrent_settings))
        .route("/qbittorrent/test", post(test_qbittorrent_connection))
        .merge(super::metadata_settings::router())
        .route(
            "/stremio/addons",
            get(list_stremio_addons).post(import_stremio_addon),
        )
        .route("/stremio/addons/preview", post(preview_stremio_addon))
        .route("/stremio/addons/refresh", post(refresh_stremio_addons))
        .route("/stremio/addons/reorder", put(reorder_stremio_addons))
        .route("/stremio/addons/:id/enabled", put(toggle_stremio_addon))
        .route(
            "/stremio/addons/:id",
            post(refresh_stremio_addon).delete(delete_stremio_addon),
        )
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StremioInstallRequest {
    install_url: String,
}
#[derive(Deserialize)]
struct StremioToggleRequest {
    enabled: bool,
}
#[derive(Deserialize)]
struct StremioReorderRequest {
    ids: Vec<String>,
}

async fn preview_stremio_addon(
    Json(r): Json<StremioInstallRequest>,
) -> Result<Json<crate::stremio::registry::AddonPreview>, (StatusCode, String)> {
    let url = crate::stremio::parser::parse_install_url(&r.install_url)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.into()))?;
    let manifest = crate::stremio::StremioClient::new()
        .map_err(internal)?
        .manifest(url.clone())
        .await
        .map_err(gateway)?;
    validate_manifest(&manifest)?;
    Ok(Json(crate::stremio::registry::AddonPreview {
        id: manifest.id,
        name: manifest.name,
        version: bounded_manifest_text(manifest.version, 64),
        description: bounded_manifest_text(manifest.description, 1_000),
        logo: sanitize_manifest_logo(manifest.logo),
        origin: url.origin().ascii_serialization(),
        credential_present: url.path_segments().is_some_and(|p| p.count() > 1),
    }))
}
async fn import_stremio_addon(
    State(state): State<Arc<AppState>>,
    Json(r): Json<StremioInstallRequest>,
) -> Result<(StatusCode, Json<crate::stremio::registry::AddonRecord>), (StatusCode, String)> {
    let url = crate::stremio::parser::parse_install_url(&r.install_url)
        .map_err(|e| (StatusCode::BAD_REQUEST, e.into()))?;
    let manifest = crate::stremio::StremioClient::new()
        .map_err(internal)?
        .manifest(url.clone())
        .await
        .map_err(gateway)?;
    validate_manifest(&manifest)?;
    let registry = crate::stremio::AddonRegistry::load_or_create().map_err(internal)?;
    let record = save_stremio_import(&state.db, &registry, &url, manifest)?;
    Ok((StatusCode::CREATED, Json(record)))
}

fn save_stremio_import(
    db: &crate::db::Db,
    registry: &crate::stremio::AddonRegistry,
    url: &url::Url,
    manifest: crate::stremio::parser::Manifest,
) -> Result<crate::stremio::registry::AddonRecord, (StatusCode, String)> {
    let stream_types = manifest
        .types
        .into_iter()
        .filter(|kind| kind == "movie" || kind == "series")
        .collect();
    let record = crate::stremio::registry::AddonRecord {
        id: manifest.id,
        name: manifest.name,
        version: bounded_manifest_text(manifest.version, 64),
        description: bounded_manifest_text(manifest.description, 1_000),
        logo: sanitize_manifest_logo(manifest.logo),
        origin: url.origin().ascii_serialization(),
        credential_present: url.path_segments().is_some_and(|p| p.count() > 1),
        enabled: true,
        sort_order: 0,
        stream_types: Some(stream_types),
    };
    let encrypted_config_url = registry.encrypt(url).map_err(internal)?;
    let mut stored = crate::stremio::registry::StoredAddon {
        public: record,
        encrypted_config_url,
    };
    match db.insert_stremio_addon(&mut stored).map_err(internal)? {
        crate::db::sqlite::StremioAddonInsert::Inserted => Ok(stored.public),
        crate::db::sqlite::StremioAddonInsert::Duplicate => {
            Err((StatusCode::CONFLICT, "addon already installed".into()))
        }
        crate::db::sqlite::StremioAddonInsert::LimitReached => {
            Err((StatusCode::CONFLICT, "stremio addon limit reached".into()))
        }
    }
}
async fn list_stremio_addons(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::stremio::registry::AddonRecord>>, (StatusCode, String)> {
    Ok(Json(
        state
            .db
            .list_stremio_addons()
            .map_err(internal)?
            .into_iter()
            .map(|x| x.public)
            .collect(),
    ))
}

async fn refresh_stremio_addon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<crate::stremio::registry::AddonRecord>, (StatusCode, String)> {
    let addon = state
        .db
        .list_stremio_addons()
        .map_err(internal)?
        .into_iter()
        .find(|addon| addon.public.id == id)
        .ok_or((StatusCode::NOT_FOUND, "addon not found".into()))?;
    let registry = crate::stremio::AddonRegistry::load_or_create().map_err(internal)?;
    let client = crate::stremio::StremioClient::new().map_err(internal)?;
    Ok(Json(
        refresh_stremio_addon_record(&state.db, &registry, &client, addon).await?,
    ))
}

async fn refresh_stremio_addons(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<crate::stremio::registry::AddonRecord>>, (StatusCode, String)> {
    let registry = crate::stremio::AddonRegistry::load_or_create().map_err(internal)?;
    let client = crate::stremio::StremioClient::new().map_err(internal)?;
    let addons = state.db.list_stremio_addons().map_err(internal)?;
    let mut refreshed = Vec::with_capacity(addons.len());

    for addon in addons {
        refreshed.push(refresh_stremio_addon_record(&state.db, &registry, &client, addon).await?);
    }
    Ok(Json(refreshed))
}

async fn refresh_stremio_addon_record(
    db: &crate::db::Db,
    registry: &crate::stremio::AddonRegistry,
    client: &crate::stremio::StremioClient,
    mut addon: crate::stremio::registry::StoredAddon,
) -> Result<crate::stremio::registry::AddonRecord, (StatusCode, String)> {
    let url = registry
        .decrypt(&addon.encrypted_config_url)
        .map_err(internal)?;
    let manifest = client.manifest(url).await.map_err(gateway)?;
    validate_manifest(&manifest)?;
    if manifest.id != addon.public.id {
        return Err((
            StatusCode::BAD_GATEWAY,
            "addon manifest identity changed".into(),
        ));
    }
    addon.public.name = manifest.name;
    addon.public.version = bounded_manifest_text(manifest.version, 64);
    addon.public.description = bounded_manifest_text(manifest.description, 1_000);
    addon.public.logo = sanitize_manifest_logo(manifest.logo);
    addon.public.stream_types = Some(
        manifest
            .types
            .into_iter()
            .filter(|kind| kind == "movie" || kind == "series")
            .collect(),
    );
    db.save_stremio_addon(&addon).map_err(internal)?;
    Ok(addon.public)
}

async fn toggle_stremio_addon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(r): Json<StremioToggleRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .set_stremio_addon_enabled(&id, r.enabled)
        .map_err(internal)?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or((StatusCode::NOT_FOUND, "addon not found".into()))
}
async fn reorder_stremio_addons(
    State(state): State<Arc<AppState>>,
    Json(r): Json<StremioReorderRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.db.reorder_stremio_addons(&r.ids).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "ids must exactly match registry".into(),
        )
    })?;
    Ok(StatusCode::NO_CONTENT)
}
async fn delete_stremio_addon(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .delete_stremio_addon(&id)
        .map_err(internal)?
        .then_some(StatusCode::NO_CONTENT)
        .ok_or((StatusCode::NOT_FOUND, "addon not found".into()))
}
fn bounded_manifest_text(value: String, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn sanitize_manifest_logo(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_owned();
    if value.is_empty() || value.len() > 2_048 {
        return None;
    }
    let url = url::Url::parse(&value).ok()?;
    crate::stremio::StremioClient::validate_url(&url)
        .is_ok()
        .then_some(value)
}

fn validate_manifest_text(value: &str, max_chars: usize) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= max_chars
        && !value.chars().any(char::is_control)
}

fn validate_manifest(m: &crate::stremio::parser::Manifest) -> Result<(), (StatusCode, String)> {
    let stream = m.resources.iter().any(|r| {
        r.as_str() == Some("stream") || r.get("name").and_then(|x| x.as_str()) == Some("stream")
    });
    if !validate_manifest_text(&m.id, 128)
        || !validate_manifest_text(&m.name, 128)
        || !stream
        || !m.types.iter().any(|t| t == "movie" || t == "series")
    {
        Err((
            StatusCode::BAD_REQUEST,
            "addon must provide movie or series streams".into(),
        ))
    } else {
        Ok(())
    }
}
fn internal(e: impl std::fmt::Display) -> (StatusCode, String) {
    let _ = e;
    (StatusCode::INTERNAL_SERVER_ERROR, "internal_error".into())
}
fn gateway(e: impl std::fmt::Display) -> (StatusCode, String) {
    let _ = e;
    (StatusCode::BAD_GATEWAY, "addon_unavailable".into())
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

#[derive(Serialize)]
struct StorageStatus {
    path: String,
    mounted: bool,
    read_only: bool,
    total_bytes: u64,
    available_bytes: u64,
    used_bytes: u64,
    used_percent: u8,
    message: String,
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
struct GeneratedIndexerKey {
    api_key: String,
}

#[derive(Serialize, Deserialize)]
struct ProwlarrSettings {
    enabled: bool,
    url: String,
    api_key: String,
    categories: Option<String>,
    limit: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct TorBoxSettings {
    pub(crate) enabled: bool,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) cache_timeout_secs: u64,
    pub(crate) show_cached_as_source: bool,
}

#[derive(Serialize)]
struct TorBoxProfileResponse {
    email: String,
    plan: String,
    status: String,
    premium_expires_at: Option<String>,
}

fn torbox_plan_label(plan: u8) -> String {
    match plan {
        0 => "Free".to_string(),
        1 => "Essential".to_string(),
        2 => "Pro".to_string(),
        3 => "Standard".to_string(),
        4 => "Premium".to_string(),
        _ => format!("Plan {plan}"),
    }
}

fn torbox_profile_response(profile: crate::torbox::client::TorBoxProfile) -> TorBoxProfileResponse {
    TorBoxProfileResponse {
        email: profile.email,
        plan: torbox_plan_label(profile.plan),
        status: if profile.is_subscribed {
            "Subscribed"
        } else {
            "Not subscribed"
        }
        .to_string(),
        premium_expires_at: profile.premium_expires_at,
    }
}

#[derive(Serialize, Deserialize)]
struct QBittorrentSettings {
    enabled: bool,
    url: String,
    username: String,
    password: String,
    category: Option<String>,
    save_path: Option<String>,
    poll_interval_secs: u64,
}

const API_KEY_VALIDATION_MESSAGE: &str =
    "API key must be non-empty ASCII without surrounding whitespace or control characters";

fn mask_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.is_empty() {
        String::new()
    } else if chars.len() <= 8 {
        "****".to_string()
    } else {
        let start: String = chars.iter().take(4).collect();
        let end: String = chars.iter().skip(chars.len() - 4).collect();
        format!("{}…{}", start, end)
    }
}

fn is_masked_secret(value: &str) -> bool {
    value.contains('…') || value.contains("...") || value.contains("****")
}

pub(crate) fn validate_api_key(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || value != value.trim()
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(API_KEY_VALIDATION_MESSAGE);
    }
    Ok(())
}

fn resolve_api_key(value: String, stored: String) -> Result<String, &'static str> {
    let value = if value.trim().is_empty() || is_masked_secret(&value) {
        stored
    } else {
        value
    };
    validate_api_key(&value)?;
    Ok(value)
}

pub(crate) fn effective_arr_config(
    db: &crate::db::Db,
    service: &str,
    fallback: Option<crate::config::ArrConfig>,
) -> Option<crate::config::ArrConfig> {
    let stored_url = db.get_setting(&format!("{service}_url")).ok().flatten();
    let stored_api_key = db.get_setting(&format!("{service}_api_key")).ok().flatten();
    let enabled = db
        .get_setting(&format!("{service}_enabled"))
        .ok()
        .flatten()
        .and_then(|value| value.parse().ok())
        .or_else(|| fallback.as_ref().map(|config| config.enabled))
        .unwrap_or(false);
    if !enabled {
        return None;
    }
    let url = stored_url.or_else(|| fallback.as_ref().map(|config| config.url.clone()))?;
    let api_key =
        stored_api_key.or_else(|| fallback.as_ref().map(|config| config.api_key.clone()))?;
    if url.is_empty() || api_key.is_empty() {
        return None;
    }
    Some(crate::config::ArrConfig {
        enabled: true,
        url,
        api_key,
        auto_import: db
            .get_setting(&format!("{service}_auto_import"))
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
            .or_else(|| fallback.map(|config| config.auto_import))
            .unwrap_or(true),
    })
}

fn validation_error(message: impl Into<String>) -> (StatusCode, Json<ActionResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ActionResponse {
            success: false,
            message: Some(message.into()),
        }),
    )
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Serialize)]
struct IntegrationHealth {
    enabled: bool,
    configured: bool,
    reachable: Option<bool>,
    message: String,
    checked_at: Option<String>,
}

#[derive(Serialize)]
struct IntegrationStatusResponse {
    sonarr: IntegrationHealth,
    radarr: IntegrationHealth,
    prowlarr: IntegrationHealth,
}

struct IntegrationConfig {
    enabled: bool,
    url: String,
    api_key: String,
}

fn load_integration_config(
    state: &AppState,
    service: &str,
    default_url: &str,
) -> rusqlite::Result<IntegrationConfig> {
    let fallback = match service {
        "sonarr" => state.config.sonarr.as_ref(),
        "radarr" => state.config.radarr.as_ref(),
        _ => None,
    };
    Ok(IntegrationConfig {
        enabled: state
            .db
            .get_setting(&format!("{service}_enabled"))?
            .and_then(|value| value.parse().ok())
            .or_else(|| fallback.map(|config| config.enabled))
            .unwrap_or(false),
        url: state
            .db
            .get_setting(&format!("{service}_url"))?
            .or_else(|| fallback.map(|config| config.url.clone()))
            .unwrap_or_else(|| default_url.to_string()),
        api_key: state
            .db
            .get_setting(&format!("{service}_api_key"))?
            .or_else(|| fallback.map(|config| config.api_key.clone()))
            .unwrap_or_default(),
    })
}

fn integration_message(enabled: bool, configured: bool, reachable: Option<bool>) -> &'static str {
    if !enabled {
        "Disabled"
    } else if !configured {
        "Not configured"
    } else if reachable == Some(true) {
        "Connected"
    } else if reachable == Some(false) {
        "Unreachable"
    } else {
        "Unknown"
    }
}

fn integration_status_url(config: &IntegrationConfig, status_path: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(config.url.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    url.set_path(status_path);
    url.set_query(None);
    url.set_fragment(None);
    Some(url)
}

async fn probe_integration(
    client: &reqwest::Client,
    config: IntegrationConfig,
    status_path: &'static str,
) -> IntegrationHealth {
    let url = integration_status_url(&config, status_path);
    let configured = validate_api_key(&config.api_key).is_ok() && url.is_some();
    if !config.enabled || !configured {
        return IntegrationHealth {
            enabled: config.enabled,
            configured,
            reachable: None,
            message: integration_message(config.enabled, configured, None).to_string(),
            checked_at: None,
        };
    }

    let url = url.expect("configured integration has a parsed URL");
    let reachable = match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        client.get(url).header("X-Api-Key", config.api_key).send(),
    )
    .await
    {
        Ok(Ok(response)) => Some(response.status().is_success()),
        Ok(Err(_)) | Err(_) => Some(false),
    };
    IntegrationHealth {
        enabled: true,
        configured: true,
        reachable,
        message: integration_message(true, true, reachable).to_string(),
        checked_at: Some(chrono::Utc::now().to_rfc3339()),
    }
}

async fn get_integration_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<IntegrationStatusResponse>, StatusCode> {
    let sonarr = load_integration_config(&state, "sonarr", "http://localhost:8989")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let radarr = load_integration_config(&state, "radarr", "http://localhost:7878")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let prowlarr = load_integration_config(&state, "prowlarr", "http://localhost:9696")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (sonarr, radarr, prowlarr) = tokio::join!(
        probe_integration(
            &state.integration_probe_client,
            sonarr,
            "/api/v3/system/status"
        ),
        probe_integration(
            &state.integration_probe_client,
            radarr,
            "/api/v3/system/status"
        ),
        probe_integration(
            &state.integration_probe_client,
            prowlarr,
            "/api/v1/system/status"
        ),
    );
    Ok(Json(IntegrationStatusResponse {
        sonarr,
        radarr,
        prowlarr,
    }))
}

pub(crate) fn notify_integration_settings_changed(state: &AppState) {
    let _ = state.integration_settings_tx.send(());
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
async fn get_settings(State(state): State<Arc<AppState>>) -> Json<SettingsResponse> {
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
            api_key: mask_secret(&s.api_key),
            auto_import: s.auto_import,
        }),
        radarr: config.radarr.as_ref().map(|r| ArrSettings {
            enabled: r.enabled,
            url: r.url.clone(),
            api_key: mask_secret(&r.api_key),
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
        message: Some(
            "Settings update not yet implemented. Edit config.toml directly.".to_string(),
        ),
    })
}

/// GET /api/settings/downloads - Get download settings
async fn get_downloads_settings(State(state): State<Arc<AppState>>) -> Json<DownloadsSettings> {
    let config = state.download_orchestrator.get_config().await;

    Json(DownloadsSettings {
        directory: config.download_dir.to_string_lossy().to_string(),
        max_concurrent: config.max_concurrent,
        segments_per_download: config.segments_per_download as u32,
    })
}

/// GET /api/settings/storage - Get effective mounted storage status
async fn get_storage_status(State(state): State<Arc<AppState>>) -> Json<StorageStatus> {
    let config = state.download_orchestrator.get_config().await;
    Json(storage_status(&config.download_dir))
}

#[cfg(unix)]
fn storage_status(path: &std::path::Path) -> StorageStatus {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};

    let display = path.to_string_lossy().to_string();
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return unavailable_storage(display, "Storage path is invalid");
    };
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: c_path is NUL-terminated and stats points to writable memory for statvfs.
    if unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return unavailable_storage(display, "Storage mount is unavailable");
    }
    // SAFETY: statvfs returned success and initialized stats.
    let stats = unsafe { stats.assume_init() };
    let block_size = stats.f_frsize as u64;
    let total_bytes = stats.f_blocks as u64 * block_size;
    let available_bytes = stats.f_bavail as u64 * block_size;
    let used_bytes = total_bytes.saturating_sub(stats.f_bfree as u64 * block_size);
    let used_percent = if total_bytes == 0 {
        0
    } else {
        ((used_bytes.saturating_mul(100) / total_bytes).min(100)) as u8
    };
    let read_only = stats.f_flag as libc::c_ulong & libc::ST_RDONLY as libc::c_ulong != 0;
    StorageStatus {
        path: display,
        mounted: true,
        read_only,
        total_bytes,
        available_bytes,
        used_bytes,
        used_percent,
        message: if read_only {
            "Mounted read-only"
        } else {
            "Mounted and writable"
        }
        .to_string(),
    }
}

#[cfg(not(unix))]
fn storage_status(path: &std::path::Path) -> StorageStatus {
    unavailable_storage(
        path.to_string_lossy().to_string(),
        "Storage status is unsupported on this platform",
    )
}

fn unavailable_storage(path: String, message: &str) -> StorageStatus {
    StorageStatus {
        path,
        mounted: false,
        read_only: false,
        total_bytes: 0,
        available_bytes: 0,
        used_bytes: 0,
        used_percent: 0,
        message: message.to_string(),
    }
}

/// PUT /api/settings/downloads - Update download settings
async fn update_downloads_settings(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DownloadsSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    let new_dir = std::path::PathBuf::from(&payload.directory);

    // Ensure the directory exists (create if absent)
    if let Err(e) = std::fs::create_dir_all(&new_dir) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ActionResponse {
                success: false,
                message: Some(format!(
                    "Cannot create directory '{}': {}",
                    new_dir.display(),
                    e
                )),
            }),
        ));
    }

    // Write-test: verify the process can actually write into it
    let test_path = new_dir.join(format!(".flasharr_write_test_{}", std::process::id()));
    match std::fs::File::create(&test_path) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_path);
        }
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ActionResponse {
                    success: false,
                    message: Some(format!(
                        "Directory '{}' is not writable: {}. Check volume mounts and ownership.",
                        new_dir.display(),
                        e
                    )),
                }),
            ));
        }
    }

    // Get current config to preserve other fields
    let mut config = state.download_orchestrator.get_config().await;

    config.max_concurrent = payload.max_concurrent;
    config.segments_per_download = payload.segments_per_download as usize;
    config.download_dir = new_dir;

    if let Err(e) = state
        .db
        .save_download_settings(&payload.directory, payload.max_concurrent as u32)
    {
        tracing::error!("Failed to persist download settings: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ActionResponse {
                success: false,
                message: Some("Failed to persist download settings".to_string()),
            }),
        ));
    }

    if let Err(e) = state.db.save_setting(
        "segments_per_download",
        &payload.segments_per_download.to_string(),
    ) {
        tracing::error!("Failed to persist segments_per_download: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ActionResponse {
                success: false,
                message: Some("Failed to persist download settings".to_string()),
            }),
        ));
    }

    state.download_orchestrator.update_config(config).await;

    Ok(Json(ActionResponse {
        success: true,
        message: Some("Download settings saved and applied successfully.".to_string()),
    }))
}

/// GET /api/settings/sonarr - Get Sonarr settings
async fn get_sonarr_settings(State(state): State<Arc<AppState>>) -> Json<ArrSettings> {
    let fallback = state.config.sonarr.as_ref();
    let url = state
        .db
        .get_setting("sonarr_url")
        .ok()
        .flatten()
        .or_else(|| fallback.map(|config| config.url.clone()))
        .unwrap_or_else(|| "http://localhost:8989".to_string());
    let api_key = state
        .db
        .get_setting("sonarr_api_key")
        .ok()
        .flatten()
        .or_else(|| fallback.map(|config| config.api_key.clone()))
        .unwrap_or_default();
    let enabled = state
        .db
        .get_setting("sonarr_enabled")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .or_else(|| state.config.sonarr.as_ref().map(|config| config.enabled))
        .unwrap_or(false);
    let auto_import = state
        .db
        .get_setting("sonarr_auto_import")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<bool>().ok())
        .or_else(|| {
            state
                .config
                .sonarr
                .as_ref()
                .map(|config| config.auto_import)
        })
        .unwrap_or(true);

    Json(ArrSettings {
        enabled,
        url,
        api_key: mask_secret(&api_key),
        auto_import,
    })
}

/// PUT /api/settings/sonarr - Update Sonarr settings
async fn update_sonarr_settings(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<ArrSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    let _update_lock = state.arr_config_update_lock.lock().await;
    let stored_api_key = state
        .db
        .get_setting("sonarr_api_key")
        .ok()
        .flatten()
        .or_else(|| {
            state
                .config
                .sonarr
                .as_ref()
                .map(|config| config.api_key.clone())
        })
        .unwrap_or_default();
    payload.api_key = if payload.enabled {
        resolve_api_key(payload.api_key, stored_api_key).map_err(validation_error)?
    } else if payload.api_key.trim().is_empty() || is_masked_secret(&payload.api_key) {
        stored_api_key
    } else {
        payload.api_key
    };

    if payload.enabled {
        let status = probe_integration(
            &state.integration_probe_client,
            IntegrationConfig {
                enabled: true,
                url: payload.url.clone(),
                api_key: payload.api_key.clone(),
            },
            "/api/v3/system/status",
        )
        .await;
        if status.reachable != Some(true) {
            return Ok(Json(ActionResponse {
                success: false,
                message: Some("Sonarr connection test failed".to_string()),
            }));
        }
    }

    state
        .db
        .save_settings(&[
            ("sonarr_url", payload.url.as_str()),
            ("sonarr_api_key", payload.api_key.as_str()),
            (
                "sonarr_enabled",
                if payload.enabled { "true" } else { "false" },
            ),
            (
                "sonarr_auto_import",
                if payload.auto_import { "true" } else { "false" },
            ),
        ])
        .map_err(|error| validation_error(format!("Failed to persist Sonarr settings: {error}")))?;

    let sonarr_config = payload.enabled.then(|| crate::config::ArrConfig {
        enabled: true,
        url: payload.url,
        api_key: payload.api_key,
        auto_import: payload.auto_import,
    });
    let radarr_config = effective_arr_config(&state.db, "radarr", state.config.radarr.clone());
    state
        .download_orchestrator
        .reload_arr_client(sonarr_config, radarr_config)
        .await;
    notify_integration_settings_changed(&state);

    Ok(Json(ActionResponse {
        success: true,
        message: Some("Sonarr settings saved and applied successfully.".to_string()),
    }))
}

/// GET /api/settings/radarr - Get Radarr settings
async fn get_radarr_settings(State(state): State<Arc<AppState>>) -> Json<ArrSettings> {
    let fallback = state.config.radarr.as_ref();
    let url = state
        .db
        .get_setting("radarr_url")
        .ok()
        .flatten()
        .or_else(|| fallback.map(|config| config.url.clone()))
        .unwrap_or_else(|| "http://localhost:7878".to_string());
    let api_key = state
        .db
        .get_setting("radarr_api_key")
        .ok()
        .flatten()
        .or_else(|| fallback.map(|config| config.api_key.clone()))
        .unwrap_or_default();
    let enabled = state
        .db
        .get_setting("radarr_enabled")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .or_else(|| state.config.radarr.as_ref().map(|config| config.enabled))
        .unwrap_or(false);
    let auto_import = state
        .db
        .get_setting("radarr_auto_import")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<bool>().ok())
        .or_else(|| {
            state
                .config
                .radarr
                .as_ref()
                .map(|config| config.auto_import)
        })
        .unwrap_or(true);

    Json(ArrSettings {
        enabled,
        url,
        api_key: mask_secret(&api_key),
        auto_import,
    })
}

/// PUT /api/settings/radarr - Update Radarr settings
async fn update_radarr_settings(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<ArrSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    let _update_lock = state.arr_config_update_lock.lock().await;
    let stored_api_key = state
        .db
        .get_setting("radarr_api_key")
        .ok()
        .flatten()
        .or_else(|| {
            state
                .config
                .radarr
                .as_ref()
                .map(|config| config.api_key.clone())
        })
        .unwrap_or_default();
    payload.api_key = if payload.enabled {
        resolve_api_key(payload.api_key, stored_api_key).map_err(validation_error)?
    } else if payload.api_key.trim().is_empty() || is_masked_secret(&payload.api_key) {
        stored_api_key
    } else {
        payload.api_key
    };

    if payload.enabled {
        let status = probe_integration(
            &state.integration_probe_client,
            IntegrationConfig {
                enabled: true,
                url: payload.url.clone(),
                api_key: payload.api_key.clone(),
            },
            "/api/v3/system/status",
        )
        .await;
        if status.reachable != Some(true) {
            return Ok(Json(ActionResponse {
                success: false,
                message: Some("Radarr connection test failed".to_string()),
            }));
        }
    }

    state
        .db
        .save_settings(&[
            ("radarr_url", payload.url.as_str()),
            ("radarr_api_key", payload.api_key.as_str()),
            (
                "radarr_enabled",
                if payload.enabled { "true" } else { "false" },
            ),
            (
                "radarr_auto_import",
                if payload.auto_import { "true" } else { "false" },
            ),
        ])
        .map_err(|error| validation_error(format!("Failed to persist Radarr settings: {error}")))?;

    let radarr_config = payload.enabled.then(|| crate::config::ArrConfig {
        enabled: true,
        url: payload.url,
        api_key: payload.api_key,
        auto_import: payload.auto_import,
    });
    let sonarr_config = effective_arr_config(&state.db, "sonarr", state.config.sonarr.clone());
    state
        .download_orchestrator
        .reload_arr_client(sonarr_config, radarr_config)
        .await;
    notify_integration_settings_changed(&state);

    Ok(Json(ActionResponse {
        success: true,
        message: Some("Radarr settings saved and applied successfully.".to_string()),
    }))
}

/// GET /api/settings/indexer - Get indexer settings
async fn get_indexer_settings(State(state): State<Arc<AppState>>) -> Json<IndexerSettings> {
    let host = &state.config.server.host;
    let port = state.config.server.port;
    let indexer_url = format!("http://{}:{}/newznab", host, port);

    // Read API key from database
    let api_key = state
        .db
        .get_setting("indexer_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();

    Json(IndexerSettings {
        enabled: !api_key.is_empty(),
        api_key: mask_secret(&api_key),
        indexer_url,
    })
}

/// PUT /api/settings/indexer - Update indexer settings
async fn update_indexer_settings(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<IndexerSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    payload.api_key = resolve_api_key(
        payload.api_key,
        state
            .db
            .get_setting("indexer_api_key")
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
    .map_err(validation_error)?;
    // Save API key to database
    if let Err(e) = state.db.save_setting("indexer_api_key", &payload.api_key) {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some(format!("Failed to save indexer settings: {}", e)),
        }));
    }

    tracing::info!("Indexer settings updated");

    Ok(Json(ActionResponse {
        success: true,
        message: Some("Indexer settings updated successfully".to_string()),
    }))
}

fn generate_and_save_indexer_api_key(db: &crate::db::Db) -> rusqlite::Result<String> {
    let api_key = format!("flasharr_{}", uuid::Uuid::new_v4().simple());
    db.save_setting("indexer_api_key", &api_key)?;
    Ok(api_key)
}

async fn generate_indexer_api_key(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GeneratedIndexerKey>, (StatusCode, Json<ActionResponse>)> {
    let api_key = generate_and_save_indexer_api_key(&state.db).map_err(|error| {
        tracing::error!("Failed to generate indexer API key: {error}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ActionResponse {
                success: false,
                message: Some("Failed to generate indexer API key".to_string()),
            }),
        )
    })?;
    Ok(Json(GeneratedIndexerKey { api_key }))
}

async fn get_prowlarr_settings(State(state): State<Arc<AppState>>) -> Json<ProwlarrSettings> {
    let url = state
        .db
        .get_setting("prowlarr_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "http://localhost:9696".to_string());
    let api_key = state
        .db
        .get_setting("prowlarr_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let enabled = state
        .db
        .get_setting("prowlarr_enabled")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let categories = state.db.get_setting("prowlarr_categories").ok().flatten();
    let limit = state
        .db
        .get_setting("prowlarr_limit")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok());

    Json(ProwlarrSettings {
        enabled,
        url,
        api_key: mask_secret(&api_key),
        categories,
        limit,
    })
}

async fn update_prowlarr_settings(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<ProwlarrSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    payload.api_key = resolve_api_key(
        payload.api_key,
        state
            .db
            .get_setting("prowlarr_api_key")
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
    .map_err(validation_error)?;
    if let Err(e) = save_prowlarr_settings(&state, &payload) {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some(format!("Failed to save Prowlarr settings: {}", e)),
        }));
    }
    bump_torrent_search_generation(&state);
    notify_integration_settings_changed(&state);
    Ok(Json(ActionResponse {
        success: true,
        message: Some("Prowlarr settings saved".to_string()),
    }))
}

async fn test_prowlarr_connection(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<ProwlarrSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    payload.api_key = resolve_api_key(
        payload.api_key,
        state
            .db
            .get_setting("prowlarr_api_key")
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
    .map_err(validation_error)?;
    let status = probe_integration(
        &state.integration_probe_client,
        IntegrationConfig {
            enabled: payload.enabled,
            url: payload.url,
            api_key: payload.api_key,
        },
        "/api/v1/system/status",
    )
    .await;
    Ok(Json(ActionResponse {
        success: !status.enabled || status.reachable == Some(true),
        message: Some(if !status.enabled {
            "Prowlarr settings saved but disabled".to_string()
        } else if !status.configured {
            "Connection failed: Prowlarr is not configured".to_string()
        } else if status.reachable == Some(true) {
            "Prowlarr connected".to_string()
        } else {
            "Connection failed: Prowlarr is unreachable".to_string()
        }),
    }))
}

async fn get_torbox_settings(State(state): State<Arc<AppState>>) -> Json<TorBoxSettings> {
    let mut settings = load_torbox_settings(&state);
    settings.api_key = mask_secret(&settings.api_key);
    Json(settings)
}

async fn update_torbox_settings(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<TorBoxSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    payload.api_key = resolve_api_key(
        payload.api_key,
        state
            .db
            .get_setting("torbox_api_key")
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
    .map_err(validation_error)?;
    payload.cache_timeout_secs = payload.cache_timeout_secs.clamp(1, 10);
    if let Err(e) = validate_torbox_base_url(&payload.base_url) {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }));
    }
    payload.base_url = crate::torbox::client::TORBOX_API_BASE.to_string();
    if let Err(e) = save_torbox_settings(&state, &payload) {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some(format!("Failed to save TorBox settings: {}", e)),
        }));
    }
    bump_torrent_search_generation(&state);
    Ok(Json(ActionResponse {
        success: true,
        message: Some("TorBox settings saved".to_string()),
    }))
}

async fn get_torbox_profile(
    State(state): State<Arc<AppState>>,
) -> Result<Json<TorBoxProfileResponse>, (StatusCode, String)> {
    let settings = load_torbox_settings(&state);
    if !settings.enabled || settings.api_key.is_empty() {
        return Err((StatusCode::CONFLICT, "torbox_not_configured".to_string()));
    }
    let client = crate::torbox::client::TorBoxClient::new(
        settings.base_url,
        settings.api_key,
        Arc::clone(&state.http_client),
    );
    let profile = client.profile().await.map_err(|error| {
        tracing::warn!("TorBox profile unavailable: {error}");
        (
            StatusCode::BAD_GATEWAY,
            "torbox_profile_unavailable".to_string(),
        )
    })?;
    Ok(Json(torbox_profile_response(profile)))
}

async fn test_torbox_connection(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<TorBoxSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    payload.api_key = resolve_api_key(
        payload.api_key,
        state
            .db
            .get_setting("torbox_api_key")
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
    .map_err(validation_error)?;
    if let Err(e) = validate_torbox_base_url(&payload.base_url) {
        return Ok(Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }));
    }
    if !payload.enabled {
        return Ok(Json(ActionResponse {
            success: true,
            message: Some("TorBox settings saved but disabled".to_string()),
        }));
    }
    let client = crate::torbox::client::TorBoxClient::new(
        payload.base_url,
        payload.api_key,
        Arc::clone(&state.http_client),
    );
    match client.test_connection().await {
        Ok(()) => Ok(Json(ActionResponse {
            success: true,
            message: Some("TorBox connected".to_string()),
        })),
        Err(e) => Ok(Json(ActionResponse {
            success: false,
            message: Some(format!("Connection failed: {}", e)),
        })),
    }
}

async fn clear_torbox_settings(State(state): State<Arc<AppState>>) -> Json<ActionResponse> {
    let cleared = TorBoxSettings {
        enabled: false,
        base_url: crate::torbox::client::TORBOX_API_BASE.to_string(),
        api_key: String::new(),
        cache_timeout_secs: 3,
        show_cached_as_source: true,
    };
    if let Err(e) = save_torbox_settings(&state, &cleared) {
        return Json(ActionResponse {
            success: false,
            message: Some(format!("Failed to clear TorBox settings: {}", e)),
        });
    }
    bump_torrent_search_generation(&state);
    Json(ActionResponse {
        success: true,
        message: Some("TorBox account removed".to_string()),
    })
}

async fn get_qbittorrent_settings(State(state): State<Arc<AppState>>) -> Json<QBittorrentSettings> {
    let mut settings = load_qbittorrent_settings(&state);
    settings.password = mask_secret(&settings.password);
    Json(settings)
}

async fn update_qbittorrent_settings(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<QBittorrentSettings>,
) -> Json<ActionResponse> {
    if payload.password.trim().is_empty() || is_masked_secret(&payload.password) {
        payload.password = load_qbittorrent_settings(&state).password;
    }
    payload.url = match crate::api::qbittorrent::validate_instance_url(&payload.url) {
        Ok(url) => url,
        Err(message) => {
            return Json(ActionResponse {
                success: false,
                message: Some(message),
            })
        }
    };
    if let Err(e) = save_qbittorrent_settings(&state, &payload) {
        return Json(ActionResponse {
            success: false,
            message: Some(format!("Failed to save qBittorrent settings: {}", e)),
        });
    }
    state.qbit_manager.reload().await;
    state
        .download_orchestrator
        .reload_qbittorrent_client(load_qbittorrent_config(&state))
        .await;
    Json(ActionResponse {
        success: true,
        message: Some("qBittorrent settings saved".to_string()),
    })
}

async fn test_qbittorrent_connection(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<QBittorrentSettings>,
) -> Json<ActionResponse> {
    if payload.password.trim().is_empty() || is_masked_secret(&payload.password) {
        payload.password = load_qbittorrent_settings(&state).password;
    }
    payload.url = match crate::api::qbittorrent::validate_instance_url(&payload.url) {
        Ok(url) => url,
        Err(message) => {
            return Json(ActionResponse {
                success: false,
                message: Some(message),
            })
        }
    };
    match crate::torrent::qbittorrent::QBittorrentClient::new(qbittorrent_config_from_settings(
        &payload,
    ))
    .and_then(|client| Ok(client))
    {
        Ok(client) => match client.test_connection().await {
            Ok(message) => Json(ActionResponse {
                success: true,
                message: Some(message),
            }),
            Err(e) => Json(ActionResponse {
                success: false,
                message: Some(format!("Connection failed: {}", e)),
            }),
        },
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(format!("Connection failed: {}", e)),
        }),
    }
}

pub fn load_qbittorrent_config(state: &AppState) -> crate::torrent::qbittorrent::QBittorrentConfig {
    qbittorrent_config_from_settings(&load_qbittorrent_settings(state))
}

fn load_qbittorrent_settings(state: &AppState) -> QBittorrentSettings {
    if let Ok(Some(instance)) = state.db.default_qbit_instance() {
        return QBittorrentSettings {
            enabled: instance.enabled,
            url: instance.url,
            username: instance.username,
            password: instance.password,
            category: instance.default_category,
            save_path: instance.save_path,
            poll_interval_secs: instance.poll_interval_secs.max(2),
        };
    }

    let url = state
        .db
        .get_setting("qbittorrent_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "http://localhost:8080".to_string());
    let username = state
        .db
        .get_setting("qbittorrent_username")
        .ok()
        .flatten()
        .unwrap_or_else(|| "admin".to_string());
    let password = state
        .db
        .get_setting("qbittorrent_password")
        .ok()
        .flatten()
        .unwrap_or_default();
    let enabled = state
        .db
        .get_setting("qbittorrent_enabled")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let category = state.db.get_setting("qbittorrent_category").ok().flatten();
    let save_path = state.db.get_setting("qbittorrent_save_path").ok().flatten();
    let poll_interval_secs = state
        .db
        .get_setting("qbittorrent_poll_interval_secs")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5)
        .max(2);

    QBittorrentSettings {
        enabled,
        url,
        username,
        password,
        category,
        save_path,
        poll_interval_secs,
    }
}

pub(crate) fn load_torbox_settings_public(state: &AppState) -> TorBoxSettings {
    load_torbox_settings(state)
}

fn load_torbox_settings(state: &AppState) -> TorBoxSettings {
    let stored_base_url = state
        .db
        .get_setting("torbox_base_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| crate::torbox::client::TORBOX_API_BASE.to_string());
    let base_url = normalize_stored_torbox_base_url(&stored_base_url);
    let api_key = state
        .db
        .get_setting("torbox_api_key")
        .ok()
        .flatten()
        .unwrap_or_default();
    let enabled = state
        .db
        .get_setting("torbox_enabled")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(false);
    let cache_timeout_secs = state
        .db
        .get_setting("torbox_cache_timeout_secs")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(3)
        .clamp(1, 10);
    let show_cached_as_source = state
        .db
        .get_setting("torbox_show_cached_as_source")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or(true);
    TorBoxSettings {
        enabled,
        base_url,
        api_key,
        cache_timeout_secs,
        show_cached_as_source,
    }
}

fn save_torbox_settings(state: &AppState, settings: &TorBoxSettings) -> rusqlite::Result<()> {
    state
        .db
        .save_setting("torbox_enabled", &settings.enabled.to_string())?;
    state
        .db
        .save_setting("torbox_base_url", &settings.base_url)?;
    state.db.save_setting("torbox_api_key", &settings.api_key)?;
    state.db.save_setting(
        "torbox_cache_timeout_secs",
        &settings.cache_timeout_secs.clamp(1, 10).to_string(),
    )?;
    state.db.save_setting(
        "torbox_show_cached_as_source",
        &settings.show_cached_as_source.to_string(),
    )?;
    Ok(())
}

fn validate_torbox_base_url(base_url: &str) -> anyhow::Result<()> {
    if base_url != crate::torbox::client::TORBOX_API_BASE {
        anyhow::bail!("TorBox API base must be https://api.torbox.app/v1/api");
    }
    Ok(())
}

fn normalize_stored_torbox_base_url(_base_url: &str) -> String {
    // Existing origin-only rows migrate in-memory; untrusted legacy values never reach the client.
    crate::torbox::client::TORBOX_API_BASE.to_string()
}

fn qbittorrent_config_from_settings(
    settings: &QBittorrentSettings,
) -> crate::torrent::qbittorrent::QBittorrentConfig {
    crate::torrent::qbittorrent::QBittorrentConfig {
        enabled: settings.enabled,
        url: settings.url.clone(),
        username: settings.username.clone(),
        password: settings.password.clone(),
        category: settings.category.clone().filter(|v| !v.trim().is_empty()),
        save_path: settings.save_path.clone().filter(|v| !v.trim().is_empty()),
        poll_interval_secs: settings.poll_interval_secs.max(2),
        instance_id: Some("default".to_string()),
        instance_name: Some("qBittorrent".to_string()),
    }
}

fn save_qbittorrent_settings(
    state: &AppState,
    settings: &QBittorrentSettings,
) -> rusqlite::Result<()> {
    let existing = state.db.default_qbit_instance().ok().flatten();
    let now = chrono::Utc::now().to_rfc3339();
    let instance = crate::db::QbitInstance {
        id: existing
            .as_ref()
            .map(|i| i.id.clone())
            .unwrap_or_else(|| "default".to_string()),
        name: existing
            .as_ref()
            .map(|i| i.name.clone())
            .unwrap_or_else(|| "qBittorrent".to_string()),
        url: settings.url.clone(),
        username: settings.username.clone(),
        password: settings.password.clone(),
        enabled: settings.enabled,
        default_category: settings.category.clone().filter(|v| !v.trim().is_empty()),
        save_path: settings.save_path.clone().filter(|v| !v.trim().is_empty()),
        poll_interval_secs: settings.poll_interval_secs.max(2),
        last_rid: existing.as_ref().map(|i| i.last_rid).unwrap_or(0),
        sync_health: existing
            .as_ref()
            .map(|i| i.sync_health.clone())
            .unwrap_or_else(|| "disconnected".to_string()),
        last_sync_at: existing.as_ref().and_then(|i| i.last_sync_at.clone()),
        app_version: existing.as_ref().and_then(|i| i.app_version.clone()),
        webapi_version: existing.as_ref().and_then(|i| i.webapi_version.clone()),
        last_seen_at: existing.as_ref().and_then(|i| i.last_seen_at.clone()),
        last_error: existing.as_ref().and_then(|i| i.last_error.clone()),
        created_at: existing
            .as_ref()
            .map(|i| i.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
    };
    state.db.save_qbit_instance(&instance)?;
    state
        .db
        .save_setting("qbittorrent_enabled", &settings.enabled.to_string())?;
    state.db.save_setting("qbittorrent_url", &settings.url)?;
    state
        .db
        .save_setting("qbittorrent_username", &settings.username)?;
    state
        .db
        .save_setting("qbittorrent_password", &settings.password)?;
    state.db.save_setting(
        "qbittorrent_category",
        settings.category.as_deref().unwrap_or(""),
    )?;
    state.db.save_setting(
        "qbittorrent_save_path",
        settings.save_path.as_deref().unwrap_or(""),
    )?;
    state.db.save_setting(
        "qbittorrent_poll_interval_secs",
        &settings.poll_interval_secs.max(2).to_string(),
    )?;
    Ok(())
}

fn bump_torrent_search_generation(state: &AppState) {
    let generation = chrono::Utc::now().timestamp_millis().to_string();
    if let Err(error) = state
        .db
        .save_setting("torrent_search_generation", &generation)
    {
        tracing::warn!(
            "Failed to invalidate Smart Search provider cache: {}",
            error
        );
    }
}

fn save_prowlarr_settings(state: &AppState, settings: &ProwlarrSettings) -> rusqlite::Result<()> {
    state
        .db
        .save_setting("prowlarr_enabled", &settings.enabled.to_string())?;
    state.db.save_setting("prowlarr_url", &settings.url)?;
    state
        .db
        .save_setting("prowlarr_api_key", &settings.api_key)?;
    state.db.save_setting(
        "prowlarr_categories",
        settings.categories.as_deref().unwrap_or(""),
    )?;
    state
        .db
        .save_setting("prowlarr_limit", &settings.limit.unwrap_or(50).to_string())?;
    Ok(())
}

/// POST /api/settings/sonarr/test - Test Sonarr connection
async fn test_sonarr_connection(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<ArrSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    payload.api_key = resolve_api_key(
        payload.api_key,
        state
            .db
            .get_setting("sonarr_api_key")
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
    .map_err(validation_error)?;
    if !payload.enabled {
        return Ok(Json(ActionResponse {
            success: true,
            message: Some("Sonarr is disabled; connection test skipped".to_string()),
        }));
    }
    let status = probe_integration(
        &state.integration_probe_client,
        IntegrationConfig {
            enabled: true,
            url: payload.url,
            api_key: payload.api_key,
        },
        "/api/v3/system/status",
    )
    .await;
    Ok(Json(ActionResponse {
        success: status.reachable == Some(true),
        message: Some(if status.reachable == Some(true) {
            "Sonarr connected".to_string()
        } else {
            "Connection failed: Sonarr is unreachable".to_string()
        }),
    }))
}

/// POST /api/settings/radarr/test - Test Radarr connection
async fn test_radarr_connection(
    State(state): State<Arc<AppState>>,
    Json(mut payload): Json<ArrSettings>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    payload.api_key = resolve_api_key(
        payload.api_key,
        state
            .db
            .get_setting("radarr_api_key")
            .ok()
            .flatten()
            .unwrap_or_default(),
    )
    .map_err(validation_error)?;
    if !payload.enabled {
        return Ok(Json(ActionResponse {
            success: true,
            message: Some("Radarr is disabled; connection test skipped".to_string()),
        }));
    }
    let status = probe_integration(
        &state.integration_probe_client,
        IntegrationConfig {
            enabled: true,
            url: payload.url,
            api_key: payload.api_key,
        },
        "/api/v3/system/status",
    )
    .await;
    Ok(Json(ActionResponse {
        success: status.reachable == Some(true),
        message: Some(if status.reachable == Some(true) {
            "Radarr connected".to_string()
        } else {
            "Connection failed: Radarr is unreachable".to_string()
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torbox_profile_response_is_sanitized() {
        let response = torbox_profile_response(crate::torbox::client::TorBoxProfile {
            email: "user@example.com".into(),
            plan: 2,
            is_subscribed: true,
            premium_expires_at: Some("2027-01-01T00:00:00+00:00".into()),
        });
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("user@example.com"));
        assert!(json.contains("Pro"));
        assert!(!json.contains("api_key"));
        assert!(!json.contains("settings"));
    }

    #[test]
    fn generated_indexer_key_persists_and_validates() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = crate::db::Db::new(file.path()).unwrap();
        let key = generate_and_save_indexer_api_key(&db).unwrap();
        assert!(key.starts_with("flasharr_"));
        assert_eq!(
            db.get_setting("indexer_api_key").unwrap().as_deref(),
            Some(key.as_str())
        );
        assert!(validate_api_key(&key).is_ok());
    }

    #[test]
    fn integration_messages_distinguish_configuration_and_health() {
        assert_eq!(integration_message(false, true, None), "Disabled");
        assert_eq!(integration_message(true, false, None), "Not configured");
        assert_eq!(integration_message(true, true, Some(true)), "Connected");
        assert_eq!(integration_message(true, true, Some(false)), "Unreachable");
        assert_eq!(integration_message(true, true, None), "Unknown");
    }

    async fn probe_server(status: StatusCode) -> (String, tokio::task::JoinHandle<()>) {
        let app = Router::new().fallback(
            move |headers: axum::http::HeaderMap, uri: axum::http::Uri| async move {
                assert_eq!(uri.path(), "/api/v3/system/status");
                assert_eq!(headers.get("X-Api-Key").unwrap(), "test-key");
                status
            },
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{address}"), handle)
    }

    #[tokio::test]
    async fn integration_probe_uses_expected_path_header_and_status() {
        let (url, server) = probe_server(StatusCode::NO_CONTENT).await;
        let health = probe_integration(
            &reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            IntegrationConfig {
                enabled: true,
                url,
                api_key: "test-key".into(),
            },
            "/api/v3/system/status",
        )
        .await;
        server.abort();
        assert_eq!(health.reachable, Some(true));
        assert_eq!(health.message, "Connected");
        assert!(health.checked_at.is_some());
    }

    #[test]
    fn integration_probe_rejects_malformed_urls() {
        for value in [
            "http://",
            "httpx://host",
            "not-a-url",
            "http://user@host",
            "http://user:password@host",
        ] {
            let config = IntegrationConfig {
                enabled: true,
                url: value.into(),
                api_key: "test-key".into(),
            };
            assert!(integration_status_url(&config, "/api/v3/system/status").is_none());
        }
    }

    #[tokio::test]
    async fn integration_probe_classifies_non_success_and_skips_disabled() {
        let (url, server) = probe_server(StatusCode::UNAUTHORIZED).await;
        let health = probe_integration(
            &reqwest::Client::new(),
            IntegrationConfig {
                enabled: true,
                url,
                api_key: "test-key".into(),
            },
            "/api/v3/system/status",
        )
        .await;
        server.abort();
        assert_eq!(health.reachable, Some(false));
        assert_eq!(health.message, "Unreachable");

        let disabled = probe_integration(
            &reqwest::Client::new(),
            IntegrationConfig {
                enabled: false,
                url: "http://127.0.0.1:1".into(),
                api_key: "test-key".into(),
            },
            "/api/v1/system/status",
        )
        .await;
        assert_eq!(disabled.reachable, None);
        assert_eq!(disabled.message, "Disabled");
        assert!(disabled.checked_at.is_none());
    }

    #[test]
    fn integration_status_serialization_contains_no_configuration() {
        let response = IntegrationStatusResponse {
            sonarr: IntegrationHealth {
                enabled: true,
                configured: true,
                reachable: Some(true),
                message: "Connected".into(),
                checked_at: Some("2026-07-27T00:00:00Z".into()),
            },
            radarr: IntegrationHealth {
                enabled: false,
                configured: false,
                reachable: None,
                message: "Disabled".into(),
                checked_at: None,
            },
            prowlarr: IntegrationHealth {
                enabled: true,
                configured: true,
                reachable: Some(false),
                message: "Unreachable".into(),
                checked_at: Some("2026-07-27T00:00:00Z".into()),
            },
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("api_key"));
        assert!(!json.contains("url"));
        assert!(!json.contains("secret"));
    }

    #[test]
    fn api_keys_require_visible_ascii_without_padding() {
        for value in ["valid_key-123", "abc.DEF_456"] {
            assert!(validate_api_key(value).is_ok());
        }
        for value in ["", " key", "key ", "key\nvalue", "key\u{00e9}"] {
            assert!(validate_api_key(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn masked_secret_resolution_preserves_ascii_value() {
        assert_eq!(
            resolve_api_key("abcd\u{2026}wxyz".to_string(), "stored_key".to_string()),
            Ok("stored_key".to_string())
        );
        assert_eq!(
            resolve_api_key("abcd...wxyz".to_string(), "stored_key".to_string()),
            Ok("stored_key".to_string())
        );
        assert!(resolve_api_key("\u{2026}".to_string(), "\u{00e9}".to_string()).is_err());
    }

    #[test]
    fn masking_never_panics_on_unicode() {
        assert_eq!(mask_secret("éabcdéwxyz"), "éabc…wxyz");
    }

    fn manifest() -> crate::stremio::parser::Manifest {
        crate::stremio::parser::Manifest {
            id: "addon.test".into(),
            name: "Secret addon".into(),
            version: "1".into(),
            description: "Addon description".into(),
            logo: Some("https://cdn.example/logo.png".into()),
            resources: vec![serde_json::json!("stream")],
            types: vec!["movie".into()],
        }
    }

    #[test]
    fn stremio_manifest_logo_accepts_public_https_and_omits_unsafe_urls() {
        assert_eq!(
            sanitize_manifest_logo(Some(" https://cdn.example/logo.png ".into())).as_deref(),
            Some("https://cdn.example/logo.png")
        );
        for value in [
            "http://cdn.example/logo.png",
            "https://user:pass@cdn.example/logo.png",
            "https://localhost/logo.png",
            "https://127.0.0.1/logo.png",
            "not a URL",
            "",
        ] {
            assert_eq!(sanitize_manifest_logo(Some(value.into())), None, "{value}");
        }
        assert_eq!(sanitize_manifest_logo(None), None);
    }

    #[test]
    fn stremio_manifest_identity_is_bounded_and_control_free() {
        assert!(validate_manifest_text("Đông Cung", 128));
        assert!(!validate_manifest_text("", 128));
        assert!(!validate_manifest_text("bad\nname", 128));
        assert!(!validate_manifest_text("\nvalid-looking", 128));
        assert!(!validate_manifest_text("valid-looking\t", 128));
        assert!(!validate_manifest_text(&"x".repeat(129), 128));
    }

    #[test]
    fn stremio_public_responses_never_serialize_install_or_encrypted_urls() {
        let secret = "https://addon.example/private-token/manifest.json";
        let preview = crate::stremio::registry::AddonPreview {
            id: "addon.test".into(),
            name: "Secret addon".into(),
            version: "1".into(),
            description: "Addon description".into(),
            logo: Some("https://cdn.example/logo.png".into()),
            origin: "https://addon.example".into(),
            credential_present: true,
        };
        let record = crate::stremio::registry::AddonRecord {
            id: "addon.test".into(),
            name: "Secret addon".into(),
            version: "1".into(),
            description: "Addon description".into(),
            logo: Some("https://cdn.example/logo.png".into()),
            origin: "https://addon.example".into(),
            credential_present: true,
            enabled: true,
            sort_order: 0,
            stream_types: Some(vec!["movie".into()]),
        };
        assert!(bounded_manifest_text("😀".repeat(65), 64).chars().count() == 64);
        for json in [
            serde_json::to_string(&preview).unwrap(),
            serde_json::to_string(&record).unwrap(),
            serde_json::to_string(&vec![record]).unwrap(),
        ] {
            assert!(!json.contains("encrypted_config_url"));
            assert!(!json.contains("encryptedConfigUrl"));
            assert!(!json.contains(secret));
            assert!(!json.contains("private-token"));
        }
    }

    #[test]
    fn stremio_import_encrypts_config_url_and_uses_next_free_order() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = crate::db::Db::new(file.path()).unwrap();
        let registry = crate::stremio::AddonRegistry::from_key(Some(&"07".repeat(32))).unwrap();
        for (id, order) in [("existing-a", 0), ("existing-b", 2)] {
            db.save_stremio_addon(&crate::stremio::registry::StoredAddon {
                public: crate::stremio::registry::AddonRecord {
                    id: id.into(),
                    name: id.into(),
                    version: String::new(),
                    description: String::new(),
                    logo: None,
                    origin: "https://addon.example".into(),
                    credential_present: false,
                    enabled: true,
                    sort_order: order,
                    stream_types: None,
                },
                encrypted_config_url: format!("cipher-{id}"),
            })
            .unwrap();
        }
        let url = url::Url::parse("https://addon.example/private-token/manifest.json").unwrap();
        let imported = save_stremio_import(&db, &registry, &url, manifest()).unwrap();
        assert_eq!(imported.sort_order, 3);
        let stored = db
            .list_stremio_addons()
            .unwrap()
            .into_iter()
            .find(|addon| addon.public.id == "addon.test")
            .unwrap();
        assert!(!stored.encrypted_config_url.contains("private-token"));
        assert_eq!(registry.decrypt(&stored.encrypted_config_url).unwrap(), url);
    }

    #[test]
    fn duplicate_stremio_import_returns_conflict_without_mutation() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = crate::db::Db::new(file.path()).unwrap();
        let registry = crate::stremio::AddonRegistry::from_key(Some(&"07".repeat(32))).unwrap();
        let original = crate::stremio::registry::StoredAddon {
            public: crate::stremio::registry::AddonRecord {
                id: "addon.test".into(),
                name: "Original".into(),
                version: "1".into(),
                description: "Original description".into(),
                logo: None,
                origin: "https://original.example".into(),
                credential_present: false,
                enabled: false,
                sort_order: 7,
                stream_types: Some(vec!["series".into()]),
            },
            encrypted_config_url: "original-ciphertext".into(),
        };
        db.save_stremio_addon(&original).unwrap();
        let url = url::Url::parse("https://replacement.example/private/manifest.json").unwrap();

        let error = save_stremio_import(&db, &registry, &url, manifest()).unwrap_err();
        assert_eq!(error.0, StatusCode::CONFLICT);

        let stored = db.list_stremio_addons().unwrap().pop().unwrap();
        assert_eq!(stored.public.name, original.public.name);
        assert_eq!(stored.public.origin, original.public.origin);
        assert_eq!(stored.public.enabled, original.public.enabled);
        assert_eq!(stored.public.sort_order, original.public.sort_order);
        assert_eq!(stored.encrypted_config_url, original.encrypted_config_url);
    }
}

#[cfg(test)]
mod phase7_security_tests {
    #[test]
    fn torbox_api_base_is_fixed_and_https() {
        assert_eq!(
            super::normalize_stored_torbox_base_url("https://api.torbox.app"),
            "https://api.torbox.app/v1/api"
        );
        assert!(super::validate_torbox_base_url("https://api.torbox.app/v1/api").is_ok());
        assert!(super::validate_torbox_base_url("https://api.torbox.app").is_err());
        assert!(super::validate_torbox_base_url("https://evil.example").is_err());
        assert!(super::validate_torbox_base_url("http://api.torbox.app").is_err());
        assert!(super::validate_torbox_base_url("https://127.0.0.1").is_err());
        assert!(super::validate_torbox_base_url("https://api.torbox.app:443/v1/api").is_err());
        assert!(super::validate_torbox_base_url("https://api.torbox.app/v1/api?x=1").is_err());
        assert!(super::validate_torbox_base_url("https://api.torbox.app/v1/api#x").is_err());
    }
}
