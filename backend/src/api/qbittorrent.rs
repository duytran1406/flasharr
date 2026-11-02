use crate::db::{QbitInstance, QbitTorrentListQuery};
use crate::torrent::qbittorrent::{
    AddTorrentOptions, CreateTorrentParams, TorrentUpload, MAX_TORRENT_UPLOAD_BYTES,
};
use crate::AppState;
use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use url::Url;

const MAX_INSTANCE_NAME_LEN: usize = 80;
const MAX_INSTANCE_USERNAME_LEN: usize = 256;
const MAX_INSTANCE_PASSWORD_LEN: usize = 1024;
const MIN_POLL_INTERVAL_SECS: u64 = 2;
const MAX_POLL_INTERVAL_SECS: u64 = 300;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_instances).post(create_instance))
        .route(
            "/:id",
            get(get_instance)
                .put(update_instance)
                .delete(delete_instance),
        )
        .route("/:id/test", post(test_instance))
        .route("/:id/qbit/torrents", get(list_torrents))
        .route("/:id/qbit/transfer-info", get(transfer_info))
        .route("/:id/qbit/categories", get(categories))
        .route("/:id/qbit/tags", get(tags))
        .route(
            "/:id/qbit/preferences",
            get(preferences).post(update_preferences),
        )
        .route("/:id/qbit/app-info", get(app_info))
        .route("/:id/qbit/torrents/add", post(add_torrent))
        .route("/:id/qbit/torrents/create", post(create_torrent_route))
        .route(
            "/:id/qbit/torrents/add-file",
            post(add_torrent_file).layer(DefaultBodyLimit::max(
                MAX_TORRENT_UPLOAD_BYTES + 1024 * 1024,
            )),
        )
        .route("/:id/qbit/torrents/pause", post(pause_torrents))
        .route("/:id/qbit/torrents/resume", post(resume_torrents))
        .route("/:id/qbit/torrents/delete", post(delete_torrents))
        .route("/:id/qbit/torrents/recheck", post(recheck_torrents))
        .route("/:id/qbit/torrents/reannounce", post(reannounce_torrents))
        .route("/:id/qbit/torrents/set-category", post(set_category))
        .route("/:id/qbit/torrents/set-tags", post(set_tags))
        .route(
            "/:id/qbit/torrents/set-speed-limits",
            post(set_speed_limits),
        )
        .route(
            "/:id/qbit/torrents/set-share-limits",
            post(set_share_limits),
        )
        .route("/:id/qbit/torrents/set-location", post(set_location))
        .route("/:id/qbit/torrents/queue/top", post(queue_top))
        .route("/:id/qbit/torrents/queue/up", post(queue_up))
        .route("/:id/qbit/torrents/queue/down", post(queue_down))
        .route("/:id/qbit/torrents/queue/bottom", post(queue_bottom))
        .route("/:id/qbit/torrents/:hash/cross-seed", post(cross_seed))
        .route("/:id/qbit/torrents/:hash/rename", post(rename_torrent))
        .route("/:id/qbit/torrents/:hash/rename-file", post(rename_file))
        .route(
            "/:id/qbit/torrents/:hash/rename-folder",
            post(rename_folder),
        )
        .route("/:id/qbit/torrents/:hash/properties", get(properties))
        .route("/:id/qbit/torrents/:hash/files", get(files))
        .route(
            "/:id/qbit/torrents/:hash/files/priority",
            post(set_file_priority),
        )
        .route("/:id/qbit/torrents/:hash/trackers", get(trackers))
        .route("/:id/qbit/torrents/:hash/peers", get(peers))
        .route("/:id/qbit/torrents/:hash/peers/add", post(add_peers))
        .route("/:id/qbit/torrents/peers/ban", post(ban_peers))
        .route("/:id/qbit/torrents/:hash/piece-states", get(piece_states))
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Serialize)]
struct HashActionResult {
    hash: String,
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[derive(Serialize)]
struct BulkActionResponse {
    success: bool,
    message: String,
    results: Vec<HashActionResult>,
}

#[derive(Serialize)]
struct InstanceResponse {
    id: String,
    name: String,
    url: String,
    username: String,
    enabled: bool,
    default_category: Option<String>,
    save_path: Option<String>,
    poll_interval_secs: u64,
    sync_health: String,
    last_sync_at: Option<String>,
    app_version: Option<String>,
    webapi_version: Option<String>,
    last_seen_at: Option<String>,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
    has_password: bool,
}

#[derive(Deserialize)]
struct InstancePayload {
    name: String,
    url: String,
    username: Option<String>,
    password: Option<String>,
    enabled: Option<bool>,
    default_category: Option<String>,
    save_path: Option<String>,
    poll_interval_secs: Option<u64>,
}

#[derive(Deserialize)]
struct InstanceQuery {
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Deserialize)]
struct TorrentQuery {
    page: Option<u32>,
    limit: Option<u32>,
    sort: Option<String>,
    order: Option<String>,
    search: Option<String>,
    filters: Option<String>,
}

#[derive(Deserialize, Default)]
struct FilterPayload {
    status: Option<Vec<String>>,
    categories: Option<Vec<String>>,
    tags: Option<Vec<String>>,
    trackers: Option<Vec<String>>,
}

#[derive(Serialize)]
struct TorrentListResponse {
    torrents: Vec<crate::api::downloads::UnifiedTorrentRow>,
    total: u64,
    page: u32,
    limit: u32,
}

#[derive(Serialize)]
struct TransferInfoResponse {
    transfer: crate::torrent::qbittorrent::QBittorrentTransferInfo,
    sync_health: String,
    last_sync_at: Option<String>,
}

#[derive(Deserialize)]
struct HashesPayload {
    hashes: Vec<String>,
    delete_files: Option<bool>,
    category: Option<String>,
    tags: Option<String>,
}

#[derive(Deserialize)]
struct AddTorrentPayload {
    url: String,
    category: Option<String>,
    savepath: Option<String>,
    tags: Option<String>,
    paused: Option<bool>,
    stopped: Option<bool>,
    skip_checking: Option<bool>,
    sequential_download: Option<bool>,
    first_last_piece_prio: Option<bool>,
}

#[derive(Deserialize)]
struct CreateTorrentPayload {
    source_path: String,
    save_path: Option<String>,
    torrent_name: Option<String>,
    trackers: Option<String>,
    source: Option<String>,
    private: Option<bool>,
    start_seeding: Option<bool>,
}

#[derive(Deserialize)]
struct FilePriorityPayload {
    id: u32,
    priority: i32,
}

#[derive(Deserialize)]
struct PreferencesPayload {
    dl_limit: Option<i64>,
    up_limit: Option<i64>,
    alt_dl_limit: Option<i64>,
    alt_up_limit: Option<i64>,
    max_ratio_enabled: Option<bool>,
    max_ratio: Option<f64>,
    max_seeding_time_enabled: Option<bool>,
    max_seeding_time: Option<i64>,
    queueing_enabled: Option<bool>,
    max_active_downloads: Option<i64>,
    max_active_uploads: Option<i64>,
}

#[derive(Deserialize)]
struct SpeedLimitsPayload {
    hashes: Vec<String>,
    download_limit: Option<i64>,
    upload_limit: Option<i64>,
}

#[derive(Deserialize)]
struct ShareLimitsPayload {
    hashes: Vec<String>,
    ratio_limit: Option<f64>,
    seeding_time_limit: Option<i64>,
}

#[derive(Deserialize)]
struct LocationPayload {
    hashes: Vec<String>,
    location: String,
}

#[derive(Deserialize)]
struct CrossSeedPayload {
    target_instance_id: String,
    save_path: Option<String>,
}

#[derive(Deserialize)]
struct PeersPayload {
    peers: String,
}

#[derive(Deserialize)]
struct RenameTorrentPayload {
    name: String,
}

#[derive(Deserialize)]
struct RenamePathPayload {
    old_path: String,
    new_name: String,
}

async fn list_instances(
    State(state): State<Arc<AppState>>,
    Query(query): Query<InstanceQuery>,
) -> Result<Json<Vec<InstanceResponse>>, StatusCode> {
    if query.kind.as_deref().is_some_and(|v| v != "qbittorrent") {
        return Ok(Json(Vec::new()));
    }
    let instances = state
        .db
        .list_qbit_instances()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(instances.into_iter().map(instance_response).collect()))
}

async fn get_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<InstanceResponse>, StatusCode> {
    let instance = state
        .db
        .get_qbit_instance(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(instance_response(instance)))
}

async fn create_instance(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InstancePayload>,
) -> Result<Json<InstanceResponse>, (StatusCode, Json<ActionResponse>)> {
    let (name, url, username, password, poll_interval_secs) = validate_instance_payload(&payload)?;
    let existing = state.db.list_qbit_instances().map_err(internal_error)?;
    if existing
        .iter()
        .any(|instance| instance.name.eq_ignore_ascii_case(&name) || instance.url == url)
    {
        return Err(bad_request(
            "qBittorrent instance name or URL already exists",
        ));
    }
    let now = chrono::Utc::now().to_rfc3339();
    let instance = QbitInstance {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        url,
        username,
        password,
        enabled: payload.enabled.unwrap_or(true),
        default_category: normalize_optional(payload.default_category.clone()),
        save_path: normalize_optional(payload.save_path.clone()),
        poll_interval_secs,
        last_rid: 0,
        sync_health: "disconnected".to_string(),
        last_sync_at: None,
        app_version: None,
        webapi_version: None,
        last_seen_at: None,
        last_error: None,
        created_at: now.clone(),
        updated_at: now,
    };
    state
        .db
        .save_qbit_instance(&instance)
        .map_err(internal_error)?;
    state.qbit_manager.reload().await;
    Ok(Json(instance_response(instance)))
}

async fn update_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<InstancePayload>,
) -> Result<Json<InstanceResponse>, (StatusCode, Json<ActionResponse>)> {
    let mut instance = state
        .db
        .get_qbit_instance(&id)
        .map_err(internal_error)?
        .ok_or_else(not_found)?;
    let (name, url, username, password, poll_interval_secs) = validate_instance_payload(&payload)?;
    let existing = state.db.list_qbit_instances().map_err(internal_error)?;
    if existing
        .iter()
        .any(|other| other.id != id && (other.name.eq_ignore_ascii_case(&name) || other.url == url))
    {
        return Err(bad_request(
            "qBittorrent instance name or URL already exists",
        ));
    }
    instance.name = name;
    instance.url = url;
    instance.username = username;
    if !password.is_empty() {
        instance.password = password;
    }
    instance.enabled = payload.enabled.unwrap_or(instance.enabled);
    instance.default_category = normalize_optional(payload.default_category.clone());
    instance.save_path = normalize_optional(payload.save_path.clone());
    instance.poll_interval_secs = poll_interval_secs;
    state
        .db
        .save_qbit_instance(&instance)
        .map_err(internal_error)?;
    state.qbit_manager.reload().await;
    Ok(Json(instance_response(instance)))
}

async fn delete_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ActionResponse>, (StatusCode, Json<ActionResponse>)> {
    if state
        .db
        .get_qbit_instance(&id)
        .map_err(internal_error)?
        .is_none()
    {
        return Err(not_found());
    }
    if state
        .db
        .count_active_tasks_for_qbit_instance(&id)
        .map_err(internal_error)?
        > 0
    {
        return Err(bad_request(
            "Cannot delete an instance with active Flasharr torrent tasks",
        ));
    }
    state.db.delete_qbit_instance(&id).map_err(internal_error)?;
    state.qbit_manager.reload().await;
    Ok(Json(ActionResponse {
        success: true,
        message: None,
    }))
}

fn validate_instance_payload(
    payload: &InstancePayload,
) -> Result<(String, String, String, String, u64), (StatusCode, Json<ActionResponse>)> {
    let name = payload.name.trim().to_string();
    if name.is_empty() || name.len() > MAX_INSTANCE_NAME_LEN {
        return Err(bad_request("Instance name must be 1-80 characters"));
    }
    let url = validate_instance_url(&payload.url).map_err(|message| bad_request(&message))?;
    let username = payload
        .username
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_string();
    if username.len() > MAX_INSTANCE_USERNAME_LEN {
        return Err(bad_request("Username exceeds 256 characters"));
    }
    let password = payload.password.as_deref().unwrap_or_default().to_string();
    if password.len() > MAX_INSTANCE_PASSWORD_LEN {
        return Err(bad_request("Password exceeds 1024 characters"));
    }
    let password = if password.contains('…') || password.contains("****") {
        String::new()
    } else {
        password
    };
    let poll_interval_secs = payload
        .poll_interval_secs
        .unwrap_or(MIN_POLL_INTERVAL_SECS)
        .clamp(MIN_POLL_INTERVAL_SECS, MAX_POLL_INTERVAL_SECS);
    Ok((name, url, username, password, poll_interval_secs))
}

pub(crate) fn validate_instance_url(value: &str) -> Result<String, String> {
    let parsed = Url::parse(value.trim())
        .map_err(|_| "Instance URL must be a valid HTTP(S) URL".to_string())?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || parsed.username() != ""
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err(
            "Instance URL must be an absolute HTTP(S) endpoint without embedded credentials"
                .to_string(),
        );
    }
    if !allow_public_qbit_hosts() && !is_private_qbit_host(parsed.host_str().unwrap_or_default()) {
        return Err("qBittorrent URL must use localhost, a private network address, or an approved Docker hostname; set FLASHARR_ALLOW_PUBLIC_QBIT_HOSTS=true to override".to_string());
    }
    Ok(parsed.to_string().trim_end_matches('/').to_string())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn bad_request(message: &str) -> (StatusCode, Json<ActionResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ActionResponse {
            success: false,
            message: Some(message.to_string()),
        }),
    )
}

fn not_found() -> (StatusCode, Json<ActionResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        }),
    )
}

fn internal_error(_: rusqlite::Error) -> (StatusCode, Json<ActionResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ActionResponse {
            success: false,
            message: Some("Database operation failed".to_string()),
        }),
    )
}

// ponytail: qBittorrent is normally deployed on a private LAN; enforce an explicit opt-in before allowing arbitrary public endpoints.
fn allow_public_qbit_hosts() -> bool {
    std::env::var("FLASHARR_ALLOW_PUBLIC_QBIT_HOSTS")
        .ok()
        .as_deref()
        == Some("true")
}

fn is_private_qbit_host(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") || !host.contains('.') && !host.contains(':') {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .map_or(false, |ip| match ip {
            std::net::IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
            std::net::IpAddr::V6(ip) => {
                ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
            }
        })
}

#[cfg(test)]
mod instance_tests {
    use super::*;

    fn payload(url: &str) -> InstancePayload {
        InstancePayload {
            name: "qBittorrent".to_string(),
            url: url.to_string(),
            username: Some("admin".to_string()),
            password: Some("password".to_string()),
            enabled: Some(true),
            default_category: None,
            save_path: None,
            poll_interval_secs: Some(1),
        }
    }

    #[test]
    fn instance_payload_requires_clean_http_endpoint() {
        let validated = validate_instance_payload(&payload("http://192.168.1.10:8080/"));
        assert!(validated.is_ok());
        let (_, normalized, _, _, interval) = validated.ok().expect("valid instance payload");
        assert_eq!(normalized, "http://192.168.1.10:8080");
        assert!(validate_instance_payload(&payload("https://qbit.example.test/")).is_err());
        assert_eq!(interval, MIN_POLL_INTERVAL_SECS);
        assert!(validate_instance_payload(&payload("ftp://qbit.example.test")).is_err());
        assert!(
            validate_instance_payload(&payload("https://user:pass@qbit.example.test")).is_err()
        );
        assert!(
            validate_instance_payload(&payload("https://qbit.example.test/?token=secret")).is_err()
        );
    }
}

async fn test_instance(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<ActionResponse> {
    let Some(instance) = state.db.get_qbit_instance(&id).ok().flatten() else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    match state.qbit_manager.test_instance(&instance).await {
        Ok(message) => Json(ActionResponse {
            success: true,
            message: Some(message),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

async fn list_torrents(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<TorrentQuery>,
) -> Result<Json<TorrentListResponse>, StatusCode> {
    let filters = query
        .filters
        .as_deref()
        .and_then(|v| serde_json::from_str::<FilterPayload>(v).ok())
        .unwrap_or_default();
    let q = QbitTorrentListQuery {
        page: query.page.unwrap_or(1),
        limit: query.limit.unwrap_or(300).clamp(1, 2000),
        sort: query.sort.unwrap_or_else(|| "added_on".to_string()),
        order: query.order.unwrap_or_else(|| "desc".to_string()),
        search: query.search,
        status: filters.status,
        categories: filters.categories,
        tags: filters.tags,
        trackers: filters.trackers,
    };
    let (torrents, total) = state
        .db
        .list_qbit_torrents(&id, &q)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let instance = state.db.get_qbit_instance(&id).ok().flatten();
    let instances = instance
        .clone()
        .into_iter()
        .map(|item| (item.id.clone(), item))
        .collect();
    let tasks = state.db.get_all_tasks().unwrap_or_default();
    Ok(Json(TorrentListResponse {
        torrents: crate::api::downloads::project_managed_torrents(tasks, torrents, &instances),
        total,
        page: q.page,
        limit: q.limit,
    }))
}

async fn transfer_info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let client = state
        .qbit_manager
        .client(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let transfer = client
        .transfer_info()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;
    let instance = state.db.get_qbit_instance(&id).ok().flatten();
    Ok(Json(
        serde_json::to_value(TransferInfoResponse {
            transfer,
            sync_health: instance
                .as_ref()
                .map(|row| row.sync_health.clone())
                .unwrap_or_else(|| "unknown".into()),
            last_sync_at: instance.and_then(|row| row.last_sync_at),
        })
        .unwrap_or(Value::Null),
    ))
}

async fn categories(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let client = state
        .qbit_manager
        .client(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(
        client
            .categories()
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?,
    ))
}

async fn tags(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Vec<String>>, StatusCode> {
    let client = state
        .qbit_manager
        .client(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(
        client.tags().await.map_err(|_| StatusCode::BAD_GATEWAY)?,
    ))
}

async fn preferences(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let client = state
        .qbit_manager
        .client(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(
        client
            .preferences()
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?,
    ))
}

async fn update_preferences(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<PreferencesPayload>,
) -> Json<ActionResponse> {
    let mut prefs = serde_json::Map::new();
    macro_rules! set_i64 {
        ($field:ident) => {
            if let Some(v) = payload.$field {
                if v < 0 {
                    return Json(ActionResponse {
                        success: false,
                        message: Some(format!("{} must be non-negative", stringify!($field))),
                    });
                }
                prefs.insert(stringify!($field).to_string(), Value::from(v));
            }
        };
    }
    macro_rules! set_bool {
        ($field:ident) => {
            if let Some(v) = payload.$field {
                prefs.insert(stringify!($field).to_string(), Value::from(v));
            }
        };
    }
    for (name, value) in [("max_ratio", payload.max_ratio)] {
        if let Some(v) = value {
            if v < 0.0 {
                return Json(ActionResponse {
                    success: false,
                    message: Some(format!("{name} must be non-negative")),
                });
            }
            prefs.insert(name.to_string(), Value::from(v));
        }
    }
    set_i64!(dl_limit);
    set_i64!(up_limit);
    set_i64!(alt_dl_limit);
    set_i64!(alt_up_limit);
    set_i64!(max_seeding_time);
    set_i64!(max_active_downloads);
    set_i64!(max_active_uploads);
    set_bool!(max_ratio_enabled);
    set_bool!(max_seeding_time_enabled);
    set_bool!(queueing_enabled);
    if prefs.is_empty() {
        return Json(ActionResponse {
            success: true,
            message: Some("No preferences changed".to_string()),
        });
    }
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    match client.set_preferences(Value::Object(prefs)).await {
        Ok(()) => Json(ActionResponse {
            success: true,
            message: Some("Preferences saved".to_string()),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

async fn app_info(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let client = state
        .qbit_manager
        .client(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(
        serde_json::to_value(
            client
                .app_info()
                .await
                .map_err(|_| StatusCode::BAD_GATEWAY)?,
        )
        .unwrap_or(Value::Null),
    ))
}

async fn add_torrent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<AddTorrentPayload>,
) -> Json<ActionResponse> {
    let opts = AddTorrentOptions {
        urls: None,
        category: payload.category,
        savepath: payload.savepath,
        tags: payload.tags,
        paused: payload.paused,
        stopped: payload.stopped,
        skip_checking: payload.skip_checking,
        sequential_download: payload.sequential_download,
        first_last_piece_prio: payload.first_last_piece_prio,
    };
    match state
        .qbit_manager
        .add_torrent(&id, &payload.url, opts)
        .await
    {
        Ok(hash) => Json(ActionResponse {
            success: true,
            message: hash,
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

async fn add_torrent_file(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Json<ActionResponse> {
    let mut upload: Option<TorrentUpload> = None;
    let mut opts = AddTorrentOptions::default();
    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(e) => {
                return Json(ActionResponse {
                    success: false,
                    message: Some(e.to_string()),
                })
            }
        };
        let name = field.name().unwrap_or_default().to_string();
        match name.as_str() {
            "torrent" | "file" | "torrents" => {
                let file_name = field.file_name().unwrap_or("flasharr.torrent").to_string();
                match field.bytes().await {
                    Ok(bytes) => {
                        upload = Some(TorrentUpload {
                            name: file_name,
                            bytes: bytes.to_vec(),
                        })
                    }
                    Err(e) => {
                        return Json(ActionResponse {
                            success: false,
                            message: Some(e.to_string()),
                        })
                    }
                }
            }
            _ => {
                let value = match field.text().await {
                    Ok(value) => value,
                    Err(e) => {
                        return Json(ActionResponse {
                            success: false,
                            message: Some(e.to_string()),
                        })
                    }
                };
                if let Err(message) = apply_add_option(&mut opts, &name, value) {
                    return Json(ActionResponse {
                        success: false,
                        message: Some(message),
                    });
                }
            }
        }
    }
    let Some(upload) = upload else {
        return Json(ActionResponse {
            success: false,
            message: Some("Missing .torrent file".to_string()),
        });
    };
    match state
        .qbit_manager
        .add_torrent_upload(&id, upload, opts)
        .await
    {
        Ok(hash) => Json(ActionResponse {
            success: true,
            message: hash,
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

fn apply_add_option(opts: &mut AddTorrentOptions, key: &str, value: String) -> Result<(), String> {
    match key {
        "category" => opts.category = Some(value),
        "savepath" | "save_path" => opts.savepath = Some(value),
        "tags" => opts.tags = Some(value),
        "paused" => {
            opts.paused =
                Some(parse_bool(&value).ok_or_else(|| format!("Invalid boolean for {key}"))?)
        }
        "stopped" => {
            opts.stopped =
                Some(parse_bool(&value).ok_or_else(|| format!("Invalid boolean for {key}"))?)
        }
        "skip_checking" | "skipChecking" => {
            opts.skip_checking =
                Some(parse_bool(&value).ok_or_else(|| format!("Invalid boolean for {key}"))?)
        }
        "sequential_download" | "sequentialDownload" => {
            opts.sequential_download =
                Some(parse_bool(&value).ok_or_else(|| format!("Invalid boolean for {key}"))?)
        }
        "first_last_piece_prio" | "firstLastPiecePrio" => {
            opts.first_last_piece_prio =
                Some(parse_bool(&value).ok_or_else(|| format!("Invalid boolean for {key}"))?)
        }
        _ => {}
    }
    Ok(())
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "1" | "on" | "yes" => Some(true),
        "false" | "0" | "off" | "no" => Some(false),
        _ => None,
    }
}

async fn create_torrent_route(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<CreateTorrentPayload>,
) -> Json<ActionResponse> {
    if !crate::torrent::qbittorrent::valid_create_torrent_path(&payload.source_path) {
        return Json(ActionResponse {
            success: false,
            message: Some("Source path is required".to_string()),
        });
    }
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    let params = CreateTorrentParams {
        source_path: payload.source_path,
        save_path: payload.save_path,
        torrent_name: payload.torrent_name,
        trackers: payload.trackers,
        source: payload.source,
        private: payload.private.unwrap_or(false),
        start_seeding: payload.start_seeding.unwrap_or(false),
    };
    match client.create_torrent(&params).await {
        Ok(_) => Json(ActionResponse {
            success: true,
            message: Some("Torrent created".to_string()),
        }),
        Err(e) if e.to_string().contains("HTTP 404") => Json(ActionResponse {
            success: false,
            message: Some("qBittorrent version does not support torrent creation".to_string()),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

async fn pause_torrents(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    bulk(&state, &id, &payload.hashes, |c, h| {
        Box::pin(async move { c.pause(h).await })
    })
    .await
}
async fn resume_torrents(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    bulk(&state, &id, &payload.hashes, |c, h| {
        Box::pin(async move { c.resume(h).await })
    })
    .await
}
async fn delete_torrents(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    let delete_files = payload.delete_files.unwrap_or(false);
    bulk(&state, &id, &payload.hashes, move |c, h| {
        Box::pin(async move { c.delete(h, delete_files).await })
    })
    .await
}
async fn recheck_torrents(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    bulk(&state, &id, &payload.hashes, |c, h| {
        Box::pin(async move { c.recheck(h).await })
    })
    .await
}
async fn reannounce_torrents(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    bulk(&state, &id, &payload.hashes, |c, h| {
        Box::pin(async move { c.reannounce(h).await })
    })
    .await
}
async fn set_category(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    let category = payload.category.unwrap_or_default();
    bulk(&state, &id, &payload.hashes, move |c, h| {
        let category = category.clone();
        Box::pin(async move { c.set_category(h, &category).await })
    })
    .await
}
async fn set_tags(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    let tags = payload.tags.unwrap_or_default();
    bulk(&state, &id, &payload.hashes, move |c, h| {
        let tags = tags.clone();
        Box::pin(async move { c.set_tags(h, &tags).await })
    })
    .await
}
async fn set_speed_limits(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<SpeedLimitsPayload>,
) -> Json<BulkActionResponse> {
    if payload.hashes.is_empty() {
        return bulk_validation_error("No torrents selected");
    }
    if payload.download_limit.is_none() && payload.upload_limit.is_none() {
        return bulk_validation_error("No speed limits supplied");
    }
    if payload.download_limit.is_some_and(|v| v < 0) || payload.upload_limit.is_some_and(|v| v < 0)
    {
        return bulk_validation_error("Speed limits must be non-negative");
    }
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return bulk_validation_error("Instance not found");
    };
    let mut results = Vec::with_capacity(payload.hashes.len());
    for hash in payload.hashes {
        let result = async {
            if let Some(limit) = payload.download_limit {
                client.set_download_limit(&hash, limit).await?;
            }
            if let Some(limit) = payload.upload_limit {
                client.set_upload_limit(&hash, limit).await?;
            }
            Ok::<(), anyhow::Error>(())
        }
        .await;
        results.push(hash_action_result(hash, result));
    }
    bulk_response(results, "Speed limits updated")
}
async fn set_share_limits(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ShareLimitsPayload>,
) -> Json<BulkActionResponse> {
    if payload.hashes.is_empty() {
        return bulk_validation_error("No torrents selected");
    }
    let (Some(ratio), Some(seeding_time)) = (payload.ratio_limit, payload.seeding_time_limit)
    else {
        return bulk_validation_error("Ratio and seeding time must be supplied together");
    };
    if !crate::torrent::qbittorrent::valid_share_ratio_limit(ratio)
        || !crate::torrent::qbittorrent::valid_seeding_time_limit(seeding_time)
    {
        return bulk_validation_error(
            "Share limits must be -1 for unlimited or a non-negative value",
        );
    }
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return bulk_validation_error("Instance not found");
    };
    let mut results = Vec::with_capacity(payload.hashes.len());
    for hash in payload.hashes {
        let result = client.set_share_limits(&hash, ratio, seeding_time).await;
        results.push(hash_action_result(hash, result));
    }
    bulk_response(results, "Share limits updated")
}

async fn queue_top(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    bulk(&state, &id, &payload.hashes, |c, h| {
        Box::pin(async move { c.top_priority(h).await })
    })
    .await
}
async fn queue_up(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    bulk(&state, &id, &payload.hashes, |c, h| {
        Box::pin(async move { c.increase_priority(h).await })
    })
    .await
}
async fn queue_down(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    bulk(&state, &id, &payload.hashes, |c, h| {
        Box::pin(async move { c.decrease_priority(h).await })
    })
    .await
}
async fn queue_bottom(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<HashesPayload>,
) -> Json<BulkActionResponse> {
    bulk(&state, &id, &payload.hashes, |c, h| {
        Box::pin(async move { c.bottom_priority(h).await })
    })
    .await
}

async fn set_location(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<LocationPayload>,
) -> Json<BulkActionResponse> {
    if !crate::torrent::qbittorrent::valid_non_empty_hashes(&payload.hashes) {
        return bulk_validation_error("No torrents selected");
    }
    if !crate::torrent::qbittorrent::valid_location(&payload.location) {
        return bulk_validation_error("Location is required");
    }
    let location = payload.location.trim().to_string();
    bulk(&state, &id, &payload.hashes, move |c, h| {
        let location = location.clone();
        Box::pin(async move { c.set_location(h, &location).await })
    })
    .await
}

async fn cross_seed(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
    Json(payload): Json<CrossSeedPayload>,
) -> Json<ActionResponse> {
    if !crate::torrent::qbittorrent::valid_torrent_hash(&hash) {
        return Json(ActionResponse {
            success: false,
            message: Some("Invalid torrent hash".to_string()),
        });
    }
    if !crate::torrent::qbittorrent::valid_cross_seed_targets(&id, &payload.target_instance_id) {
        return Json(ActionResponse {
            success: false,
            message: Some("Target instance must differ from source".to_string()),
        });
    }
    let savepath = match payload.save_path {
        Some(path) if crate::torrent::qbittorrent::valid_location(&path) => {
            Some(path.trim().to_string())
        }
        Some(_) => {
            return Json(ActionResponse {
                success: false,
                message: Some("Save path cannot be empty".to_string()),
            })
        }
        None => None,
    };
    let Ok(source) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Source instance not found".to_string()),
        });
    };
    let Ok(target) = state.qbit_manager.client(&payload.target_instance_id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Target instance not found".to_string()),
        });
    };
    let Some(torrent) = source.get_torrent(&hash).await.ok().flatten() else {
        return Json(ActionResponse {
            success: false,
            message: Some("Torrent not found".to_string()),
        });
    };
    let magnet = format!("magnet:?xt=urn:btih:{}", torrent.hash);
    let opts = AddTorrentOptions {
        savepath,
        ..Default::default()
    };
    match target.add_torrent_with_options(&magnet, opts).await {
        Ok(_) => Json(ActionResponse {
            success: true,
            message: Some(format!("Cross-seeded {}", torrent.name)),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

async fn rename_torrent(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
    Json(payload): Json<RenameTorrentPayload>,
) -> Json<ActionResponse> {
    if !crate::torrent::qbittorrent::valid_torrent_rename(&payload.name) {
        return Json(ActionResponse {
            success: false,
            message: Some("Name is required and cannot contain path separators".to_string()),
        });
    }
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    match client.rename_torrent(&hash, payload.name.trim()).await {
        Ok(()) => Json(ActionResponse {
            success: true,
            message: Some("Torrent renamed".to_string()),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

async fn rename_file(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
    Json(payload): Json<RenamePathPayload>,
) -> Json<ActionResponse> {
    let Some(new_path) =
        crate::torrent::qbittorrent::rename_path_with_leaf(&payload.old_path, &payload.new_name)
    else {
        return Json(ActionResponse {
            success: false,
            message: Some("New name is required and cannot contain path separators".to_string()),
        });
    };
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    match client
        .rename_file(&hash, payload.old_path.trim(), &new_path)
        .await
    {
        Ok(()) => Json(ActionResponse {
            success: true,
            message: Some("File renamed".to_string()),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

async fn rename_folder(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
    Json(payload): Json<RenamePathPayload>,
) -> Json<ActionResponse> {
    let Some(new_path) =
        crate::torrent::qbittorrent::rename_path_with_leaf(&payload.old_path, &payload.new_name)
    else {
        return Json(ActionResponse {
            success: false,
            message: Some("New name is required and cannot contain path separators".to_string()),
        });
    };
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    match client
        .rename_folder(&hash, payload.old_path.trim(), &new_path)
        .await
    {
        Ok(()) => Json(ActionResponse {
            success: true,
            message: Some("Folder renamed".to_string()),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}

async fn properties(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    detail(state, id, hash, "/api/v2/torrents/properties").await
}
async fn files(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    detail(state, id, hash, "/api/v2/torrents/files").await
}
async fn set_file_priority(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
    Json(payload): Json<FilePriorityPayload>,
) -> Json<ActionResponse> {
    if !crate::torrent::qbittorrent::valid_file_priority(payload.priority) {
        return Json(ActionResponse {
            success: false,
            message: Some("Invalid priority. Use skip, normal, high, or max.".to_string()),
        });
    }
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    match client
        .set_file_priority(&hash, payload.id, payload.priority)
        .await
    {
        Ok(()) => Json(ActionResponse {
            success: true,
            message: Some("File priority updated".to_string()),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}
async fn trackers(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    detail(state, id, hash, "/api/v2/torrents/trackers").await
}
async fn peers(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    detail(state, id, hash, "/api/v2/sync/torrentPeers").await
}
async fn add_peers(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
    Json(payload): Json<PeersPayload>,
) -> Json<ActionResponse> {
    let Some(peers) = crate::torrent::qbittorrent::normalize_peer_list(&payload.peers) else {
        return Json(ActionResponse {
            success: false,
            message: Some("Peers must be host:port".to_string()),
        });
    };
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    match client.add_peers(&hash, &peers).await {
        Ok(()) => Json(ActionResponse {
            success: true,
            message: Some("Peer(s) added".to_string()),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}
async fn ban_peers(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<PeersPayload>,
) -> Json<ActionResponse> {
    let Some(peers) = crate::torrent::qbittorrent::normalize_peer_list(&payload.peers) else {
        return Json(ActionResponse {
            success: false,
            message: Some("Peers must be host:port".to_string()),
        });
    };
    let Ok(client) = state.qbit_manager.client(&id).await else {
        return Json(ActionResponse {
            success: false,
            message: Some("Instance not found".to_string()),
        });
    };
    match client.ban_peers(&peers).await {
        Ok(()) => Json(ActionResponse {
            success: true,
            message: Some("Peer(s) banned".to_string()),
        }),
        Err(e) => Json(ActionResponse {
            success: false,
            message: Some(e.to_string()),
        }),
    }
}
async fn piece_states(
    State(state): State<Arc<AppState>>,
    Path((id, hash)): Path<(String, String)>,
) -> Result<Json<Value>, StatusCode> {
    detail(state, id, hash, "/api/v2/torrents/pieceStates").await
}

async fn detail(
    state: Arc<AppState>,
    id: String,
    hash: String,
    path: &str,
) -> Result<Json<Value>, StatusCode> {
    let client = state
        .qbit_manager
        .client(&id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Ok(Json(
        client
            .torrent_value(&hash, path)
            .await
            .map_err(|_| StatusCode::BAD_GATEWAY)?,
    ))
}

fn bulk_validation_error(message: &str) -> Json<BulkActionResponse> {
    Json(BulkActionResponse {
        success: false,
        message: message.to_string(),
        results: Vec::new(),
    })
}

fn hash_action_result(hash: String, result: anyhow::Result<()>) -> HashActionResult {
    match result {
        Ok(()) => HashActionResult {
            hash,
            success: true,
            message: None,
        },
        Err(error) => HashActionResult {
            hash,
            success: false,
            message: Some(error.to_string()),
        },
    }
}

fn bulk_response(results: Vec<HashActionResult>, action: &str) -> Json<BulkActionResponse> {
    let affected = results.iter().filter(|result| result.success).count();
    let failed = results.len() - affected;
    Json(BulkActionResponse {
        success: failed == 0,
        message: if failed == 0 {
            format!("{action}: {affected} torrent(s) affected")
        } else {
            format!("{action}: {affected} succeeded, {failed} failed")
        },
        results,
    })
}

async fn bulk<F>(
    state: &Arc<AppState>,
    id: &str,
    hashes: &[String],
    f: F,
) -> Json<BulkActionResponse>
where
    F: for<'a> Fn(
        Arc<crate::torrent::qbittorrent::QBittorrentClient>,
        &'a str,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send + 'a>,
    >,
{
    if !crate::torrent::qbittorrent::valid_non_empty_hashes(hashes) {
        return bulk_validation_error("No torrents selected");
    }
    let Ok(client) = state.qbit_manager.client(id).await else {
        return bulk_validation_error("Instance not found");
    };
    let mut results = Vec::with_capacity(hashes.len());
    for hash in hashes {
        results.push(hash_action_result(
            hash.clone(),
            f(Arc::clone(&client), hash).await,
        ));
    }
    bulk_response(results, "qBittorrent action")
}

fn instance_response(i: QbitInstance) -> InstanceResponse {
    InstanceResponse {
        has_password: !i.password.is_empty(),
        id: i.id,
        name: i.name,
        url: i.url,
        username: i.username,
        enabled: i.enabled,
        default_category: i.default_category,
        save_path: i.save_path,
        poll_interval_secs: i.poll_interval_secs,
        sync_health: i.sync_health,
        last_sync_at: i.last_sync_at,
        app_version: i.app_version,
        webapi_version: i.webapi_version,
        last_seen_at: i.last_seen_at,
        last_error: i.last_error,
        created_at: i.created_at,
        updated_at: i.updated_at,
    }
}
