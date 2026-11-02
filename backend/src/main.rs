use axum::http::{HeaderValue, Method};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{any, get},
    Json, Router,
};
use chrono::Timelike;
use figment::providers::Format;
use moka::future::Cache;
use serde::Serialize;
use std::net::SocketAddr;
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const TORRENT_DOWNLOAD_REF_CAPACITY: u64 = 50_000;

#[cfg(test)]
mod cache_tests {
    use super::TORRENT_DOWNLOAD_REF_CAPACITY;
    use moka::future::Cache;

    #[tokio::test]
    async fn cached_search_refs_survive_a_large_result_set() {
        let refs = Cache::builder()
            .max_capacity(TORRENT_DOWNLOAD_REF_CAPACITY)
            .build();
        refs.insert("first".into(), "locator".into()).await;
        for index in 0..1_000 {
            refs.insert(format!("ref-{index}"), format!("url-{index}"))
                .await;
        }
        refs.run_pending_tasks().await;
        assert_eq!(refs.get("first").await.as_deref(), Some("locator"));
    }
}

mod api;
mod arr;
mod config;
mod constants;
mod db;
mod debrid;
mod downloader;
mod error;
mod hosts;
mod metadata;
mod providers;
mod services;
mod sources;
mod stremio;
mod torbox;
mod torrent;
mod utils;
mod websocket;

use std::sync::Arc;

/// Cached TMDB enrichment — stored keyed by tmdb_id (String).
/// Holds everything the search pipeline needs without re-calling TMDB.
#[derive(Clone)]
pub struct TmdbEnrichmentCache {
    /// Official display title (e.g. "Tales of Herding Gods")
    pub official: Option<String>,
    /// Original title in content's native language (e.g. "牧神记")
    pub original_name: Option<String>,
    /// All aliases merged (for similarity scoring)
    pub all_aliases: Vec<String>,
    /// VN-only titles (searched directly on Fshare as a priority)
    pub vn_titles: Vec<String>,
    /// Titles in the content's original language (CN/KR/JP/etc.)
    pub original_lang_titles: Vec<String>,
    /// US/English alternative titles (different from official name)
    pub us_titles: Vec<String>,
    /// Poster path
    pub poster: Option<String>,
    /// Release/first-air year from TMDB.
    pub year: Option<u32>,
    /// Movie collection parts: (title, year, tmdb_id, poster)
    pub collections: Vec<(String, String, u64, Option<String>)>,
}

#[derive(Clone)]
pub struct AppState {
    pub host_registry: Arc<hosts::registry::HostRegistry>,
    pub download_orchestrator: Arc<downloader::DownloadOrchestrator>,
    pub download_service: Arc<services::DownloadService>,
    pub tmdb_service: Arc<services::TmdbService>,
    pub tvdb_service: Arc<services::TvdbService>,
    pub folder_cache_service: Arc<services::FolderCacheService>,
    pub tx_broadcast: tokio::sync::broadcast::Sender<downloader::task::DownloadTask>,
    pub integration_settings_tx: tokio::sync::broadcast::Sender<()>,
    pub config: config::Config,
    pub arr_config_update_lock: Arc<tokio::sync::Mutex<()>>,
    pub metadata_update_lock: Arc<tokio::sync::Mutex<()>>,
    pub db: Arc<db::Db>,
    pub search_cache: Cache<String, api::smart_search::SmartSearchResponse>,
    pub tmdb_cache: Cache<String, TmdbEnrichmentCache>,
    /// Shared HTTP client for search endpoints (connection pooling + TLS reuse)
    pub http_client: Arc<reqwest::Client>,
    /// Integration probes must never forward API keys across redirects.
    pub integration_probe_client: Arc<reqwest::Client>,
    /// Short-lived server-side provider download refs; keeps Prowlarr API keys out of UI.
    pub torrent_download_refs: Cache<String, String>,
    pub torrent_info_hashes: Cache<String, String>,
    /// Full read-only TorBox cache manifests, shared across Smart Search stages.
    pub torbox_cache_hits: Cache<String, Option<torbox::client::TorBoxCacheHit>>,
    pub qbit_manager: Arc<torrent::instance_manager::QbitInstanceManager>,
    pub discovery_service: Arc<services::DiscoveryService>,
    pub library_sync_service: Arc<services::LibrarySyncService>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct ApiErrorResponse {
    error: &'static str,
    message: &'static str,
}

async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(ApiErrorResponse {
            error: "not_found",
            message: "API endpoint not found",
        }),
    )
}

async fn health(State(state): State<Arc<AppState>>) -> Json<HealthResponse> {
    let status = if state.db.get_folder_cache_count().is_ok() {
        "ok"
    } else {
        "unhealthy"
    };

    Json(HealthResponse {
        status,
        version: env!("CARGO_PKG_VERSION"),
    })
}

fn duration_until_next_daily_sync() -> Duration {
    use chrono::{Days, Local, LocalResult, TimeZone};

    let now = Local::now();
    let today = now.date_naive();
    let next_day = today.checked_add_days(Days::new(1)).unwrap_or(today);
    let target_date = if now.hour() < 1 { today } else { next_day };
    let target_naive = target_date.and_hms_opt(1, 0, 0).unwrap();
    let next_run = match Local.from_local_datetime(&target_naive) {
        LocalResult::Single(dt) => dt,
        LocalResult::Ambiguous(dt, _) => dt,
        LocalResult::None => now + chrono::Duration::hours(1),
    };

    (next_run - now)
        .to_std()
        .unwrap_or_else(|_| Duration::from_secs(60))
}

#[tokio::main]
async fn main() {
    // Create appData directory structure before configuring the file-backed log sink.
    if let Err(e) = config::ensure_appdata_dirs() {
        eprintln!("Failed to create appData directories: {e}. Continuing with legacy paths.");
    }
    let log_dir = config::get_appdata_dir().join("logs");
    let file_appender = tracing_appender::rolling::daily(log_dir, "flasharr.log");
    let (file_writer, _log_guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "flasharr=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(file_writer),
        )
        .init();

    tracing::info!("Starting Flasharr v{}", env!("CARGO_PKG_VERSION"));

    if let Err(e) = config::ensure_appdata_dirs() {
        tracing::warn!(
            "Failed to create appData directories: {}. Continuing with legacy paths.",
            e
        );
    }

    // Get paths with fallback to old locations
    let config_path = config::get_config_path();
    let db_path = config::get_db_path();

    tracing::info!("Loading config from: {}", config_path.display());
    tracing::info!("Using database at: {}", db_path.display());

    // Load Config
    let config: config::Config = figment::Figment::new()
        .merge(figment::providers::Serialized::defaults(
            config::Config::default(),
        ))
        .merge(figment::providers::Toml::file(config_path))
        .merge(figment::providers::Env::prefixed("FLASHARR_"))
        .extract()
        .expect("Failed to load configuration");

    // Initialize Database
    let db = Arc::new(db::Db::new(&db_path).expect("Failed to initialize database"));

    // Security hardening: do not auto-generate or log indexer API keys.

    // Initialize components
    let (tx_broadcast, _) = tokio::sync::broadcast::channel(100);
    let (integration_settings_tx, _) = tokio::sync::broadcast::channel(16);
    let shared_http_client = hosts::create_shared_client();
    let host_registry = Arc::new(hosts::create_registry(
        &config,
        shared_http_client,
        Arc::clone(&db),
    ));

    // Create download config from app config
    let mut download_config = downloader::config::DownloadConfig {
        max_concurrent: config.downloads.max_concurrent,
        segments_per_download: config.downloads.segments_per_download as usize,
        download_dir: config.downloads.directory.clone(),
        chunk_size: 1024 * 1024, // 1 MB
        retry_attempts: 3,
        retry_backoff_base: 30,
        retry_max_wait: 300,
        symlink_real_base: config.downloads.symlink_real_base.clone().or_else(|| {
            std::env::var("FLASHARR_SYMLINK_REAL_BASE")
                .ok()
                .map(std::path::PathBuf::from)
        }),
        retry: downloader::config::RetryConfig::default(),
    };

    // V2: Robust Directory Validation for Staging Environments
    // If the configured download directory is not writable (common with /downloads volume issues),
    // we fallback to appData/downloads which is guaranteed to be writable and managed by our entrypoint.
    let app_data_dir = std::env::var("FLASHARR_APPDATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join("appData"));

    if let Some(shared_dir) = download_config.symlink_real_base.clone() {
        let app_downloads = app_data_dir.join("downloads");
        let current_dir = download_config.download_dir.clone();
        let uses_container_download_path =
            current_dir == std::path::PathBuf::from("/downloads") || current_dir == app_downloads;

        if uses_container_download_path {
            let shared_test_file =
                shared_dir.join(format!(".write_test_{}", uuid::Uuid::new_v4().simple()));
            let shared_dir_is_writable = std::fs::create_dir_all(&shared_dir)
                .and_then(|_| std::fs::File::create(&shared_test_file).map(|_| ()))
                .map(|_| {
                    let _ = std::fs::remove_file(&shared_test_file);
                    true
                })
                .unwrap_or(false);

            if shared_dir_is_writable {
                tracing::info!(
                    "Using shared download directory {} instead of container path {}",
                    shared_dir.display(),
                    current_dir.display()
                );
                download_config.download_dir = shared_dir;
            } else {
                tracing::warn!(
                    "Shared download directory {} is not writable; keeping configured directory {}",
                    shared_dir.display(),
                    current_dir.display()
                );
            }
        }
    }

    let current_dir = &download_config.download_dir;
    let fallback_dir = app_data_dir.join("downloads");

    let is_not_writable = if !current_dir.exists() {
        std::fs::create_dir_all(current_dir).is_err()
    } else {
        // Try creating a temporary test file
        let test_file = current_dir.join(format!(".write_test_{}", uuid::Uuid::new_v4().simple()));
        match std::fs::File::create(&test_file) {
            Ok(_) => {
                let _ = std::fs::remove_file(test_file);
                false
            }
            Err(_) => true,
        }
    };

    if is_not_writable {
        tracing::warn!(
            "Download directory {} is not writable! Falling back to managed appData path: {}",
            current_dir.display(),
            fallback_dir.display()
        );

        // Ensure fallback directory exists
        if let Err(e) = std::fs::create_dir_all(&fallback_dir) {
            tracing::error!(
                "CRITICAL: Failed to create fallback download directory {}: {}",
                fallback_dir.display(),
                e
            );
        } else {
            download_config.download_dir = fallback_dir;
        }
    } else {
        tracing::info!("Using download directory: {}", current_dir.display());
    }

    // Database settings override config.toml only when an integration is explicitly enabled.
    let sonarr_config = api::settings::effective_arr_config(&db, "sonarr", config.sonarr.clone());
    let radarr_config = api::settings::effective_arr_config(&db, "radarr", config.radarr.clone());

    let qbittorrent_config = torrent::qbittorrent::QBittorrentConfig {
        enabled: db
            .get_setting("qbittorrent_enabled")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<bool>().ok())
            .unwrap_or(false),
        url: db
            .get_setting("qbittorrent_url")
            .ok()
            .flatten()
            .unwrap_or_else(|| "http://localhost:8080".to_string()),
        username: db
            .get_setting("qbittorrent_username")
            .ok()
            .flatten()
            .unwrap_or_else(|| "admin".to_string()),
        password: db
            .get_setting("qbittorrent_password")
            .ok()
            .flatten()
            .unwrap_or_default(),
        category: db.get_setting("qbittorrent_category").ok().flatten(),
        save_path: db.get_setting("qbittorrent_save_path").ok().flatten(),
        poll_interval_secs: db
            .get_setting("qbittorrent_poll_interval_secs")
            .ok()
            .flatten()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(5)
            .max(2),
        instance_id: Some("default".to_string()),
        instance_name: Some("qBittorrent".to_string()),
    };

    let qbit_manager = Arc::new(torrent::instance_manager::QbitInstanceManager::new(
        Arc::clone(&db),
    ));

    // Create download orchestrator with new architecture
    let download_orchestrator = Arc::new(downloader::DownloadOrchestrator::new(
        download_config,
        Arc::clone(&host_registry),
        Some(Arc::clone(&db)),
        sonarr_config.clone(),
        radarr_config.clone(),
        qbittorrent_config,
        Some(Arc::clone(&qbit_manager)),
    ));

    let _ = qbit_manager.initial_full_sync().await;

    // Restore persisted tasks only after the qB full-sync barrier; workers start last.
    let pending_count = download_orchestrator.load_pending_tasks().await;
    tracing::info!("Loaded {} pending tasks from database", pending_count);
    qbit_manager.clone().start();
    download_orchestrator.start().await;
    tracing::info!("Download orchestrator started with new architecture");

    // Initialize Caches
    let search_cache = Cache::builder()
        .max_capacity(100)
        .time_to_live(Duration::from_secs(600)) // 10 minutes — keeps episode counts fresh for ongoing shows
        .build();

    let tmdb_cache = Cache::builder()
        .max_capacity(500)
        .time_to_live(Duration::from_secs(86400)) // 24 hours
        .build();

    // Shared HTTP client for search endpoints (connection pooling + TLS reuse)
    let http_client = Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .pool_max_idle_per_host(20)
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap_or_default(),
    );
    let integration_probe_client = Arc::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("integration probe client must build"),
    );

    // Keep opaque locators alive for every cached search response. A single
    // multi-query search can register hundreds of refs; 500 allowed valid
    // 10-minute search responses to outlive their download selections.
    let torrent_download_refs = Cache::builder()
        .max_capacity(TORRENT_DOWNLOAD_REF_CAPACITY)
        .time_to_live(Duration::from_secs(1800)) // 30 minutes
        .build();
    let torrent_info_hashes = Cache::builder()
        .max_capacity(2_000)
        .time_to_live(Duration::from_secs(86_400))
        .build();
    let torbox_cache_hits = Cache::builder()
        .max_capacity(10_000)
        .time_to_live(Duration::from_secs(30))
        .build();

    // Create DownloadService for business logic abstraction
    let download_service = Arc::new(services::DownloadService::new(
        Arc::clone(&db),
        Arc::clone(&download_orchestrator),
    ));

    // Encrypted database credentials override environment/config fallbacks at startup.
    let tmdb_key =
        api::metadata_settings::startup_key(&db, "tmdb", config.external.tmdb_api_key.clone())
            .unwrap_or_else(|_| {
                tracing::error!("Stored TMDB key unavailable; provider disabled");
                String::new()
            });
    let tvdb_key =
        api::metadata_settings::startup_key(&db, "tvdb", config.external.tvdb_api_key.clone())
            .unwrap_or_else(|_| {
                tracing::error!("Stored TVDB key unavailable; provider disabled");
                String::new()
            });
    let tmdb_service = Arc::new(services::TmdbService::new_with_api_key(tmdb_key));
    let tvdb_service = Arc::new(services::TvdbService::new_with_default_client(Some(
        tvdb_key,
    )));

    // Create FolderCacheService for folder source caching
    let folder_cache_service = Arc::new(services::FolderCacheService::new(
        Arc::clone(&db),
        Arc::clone(&tmdb_service),
    ));

    // Update config with loaded Arr settings so API sees them
    let mut app_config = config.clone();
    app_config.sonarr = sonarr_config.clone();
    app_config.radarr = radarr_config.clone();

    // Create DiscoveryService
    let discovery_service = Arc::new(services::DiscoveryService::new(
        Arc::clone(&db),
        Arc::clone(&download_orchestrator),
        Arc::clone(&tmdb_service),
        Arc::clone(&folder_cache_service),
    ));

    let library_sync_service = Arc::new(services::LibrarySyncService::new(
        Arc::clone(&db),
        Arc::clone(&download_orchestrator),
    ));

    let state = Arc::new(AppState {
        host_registry,
        download_orchestrator,
        download_service,
        tmdb_service,
        tvdb_service,
        folder_cache_service: Arc::clone(&folder_cache_service),
        tx_broadcast,
        integration_settings_tx,
        config: app_config,
        arr_config_update_lock: Arc::new(tokio::sync::Mutex::new(())),
        metadata_update_lock: Arc::new(tokio::sync::Mutex::new(())),
        db,
        search_cache,
        tmdb_cache,
        http_client,
        integration_probe_client,
        torrent_download_refs,
        torrent_info_hashes,
        torbox_cache_hits,
        qbit_manager: Arc::clone(&qbit_manager),
        discovery_service,
        library_sync_service: Arc::clone(&library_sync_service),
    });
    // Build router
    // Web app routes — no key required (served to the browser UI)
    let protected_api_routes = Router::new()
        .route("/api/ws", get(websocket::handler))
        .nest("/api/kpi", api::kpi::router())
        .nest("/api/stats", api::stats::router())
        .nest("/api/system", api::system::router())
        .nest("/api/accounts", api::accounts::router())
        .nest("/api/providers", providers::router())
        .nest("/api/arr", api::arr::router())
        .nest("/api/media", api::media::router())
        .nest("/api/downloads", api::downloads::router())
        .nest("/api/instances", api::qbittorrent::router())
        .nest("/api/settings", api::settings::router())
        .nest("/api/folder-source", api::folder_source::router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            api::auth::auth_middleware,
        ));

    let public_api_routes = Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
        .route("/api/health/status", get(api::health::health_status))
        .nest("/api/auth", api::auth::router())
        .nest(
            "/api/setup",
            api::setup::router().layer(axum::middleware::from_fn_with_state(
                state.clone(),
                api::auth::setup_auth_middleware,
            )),
        )
        .nest("/api/search", api::search::router())
        .nest("/api/tmdb", api::tmdb::router())
        .nest("/api/discovery", api::discovery::router())
        .nest("/api/discover", api::discovery::router())
        .merge(protected_api_routes)
        .nest(
            "/sabnzbd",
            api::sabnzbd::router().layer(axum::middleware::from_fn_with_state(
                state.clone(),
                api::auth::indexer_auth_middleware,
            )),
        );

    let app_routes = public_api_routes
        .route_service("/downloads", ServeFile::new("static/index.html"))
        .route_service("/downloads/", ServeFile::new("static/index.html"));

    // External integration routes — require API key (called by Jellyflix/other integrations)
    let external_routes = Router::new()
        .nest("/api/indexer", api::indexer::router())
        .nest("/newznab/api", api::indexer::router()) // Standard Newznab path
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            api::auth::indexer_auth_middleware,
        ));

    let app = Router::new()
        .merge(app_routes)
        .merge(external_routes)
        .route("/api/does-not-exist", any(api_not_found))
        .fallback_service(ServeDir::new("static").fallback(ServeFile::new("static/index.html")))
        .layer(
            CorsLayer::new()
                .allow_origin(AllowOrigin::list([
                    HeaderValue::from_static("http://localhost:3000"),
                    HeaderValue::from_static("http://127.0.0.1:3000"),
                    HeaderValue::from_static("http://192.168.1.112:8484"),
                    HeaderValue::from_static("http://192.168.1.110:8096"),
                ]))
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::HeaderName::from_static("x-api-key"),
                ]),
        )
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state);

    // Spawn folder cache background sync (daily at local 01:00)
    tokio::spawn(async move {
        folder_cache_service.sync_if_stale().await;

        loop {
            let sleep_for = duration_until_next_daily_sync();
            tracing::info!(
                "[FOLDER-CACHE] Next scheduled sync in {}s",
                sleep_for.as_secs()
            );
            tokio::time::sleep(sleep_for).await;

            tracing::info!("[FOLDER-CACHE] Starting scheduled daily sync");
            match folder_cache_service
                .sync_all_sources_with_options(true)
                .await
            {
                Ok(report) => {
                    tracing::info!(
                        "[FOLDER-CACHE] Daily sync complete: {} items from {} sources in {:.1}s",
                        report.total_items,
                        report.total_sources,
                        report.duration_secs
                    );
                }
                Err(e) => {
                    tracing::error!("[FOLDER-CACHE] Daily sync failed: {}", e);
                }
            }
        }
    });

    // Run server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    tracing::info!("Listening on {}", addr);

    // Create socket with SO_REUSEADDR to allow immediate restart after crash
    use socket2::{Domain, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None).expect("Failed to create socket");
    socket
        .set_reuse_address(true)
        .expect("Failed to set SO_REUSEADDR");
    socket.bind(&addr.into()).expect("Failed to bind socket");
    socket.listen(1024).expect("Failed to listen on socket");

    // Set non-blocking mode before converting to tokio
    socket
        .set_nonblocking(true)
        .expect("Failed to set non-blocking mode");

    // Convert to tokio listener
    let listener = tokio::net::TcpListener::from_std(socket.into())
        .expect("Failed to convert to tokio listener");

    axum::serve(listener, app).await.unwrap();
}
