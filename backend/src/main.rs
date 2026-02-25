use axum::{
    routing::get,
    Router,
    Json,
};
use serde::Serialize;
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use figment::providers::Format;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::cors::{CorsLayer, Any};
use moka::future::Cache;
use std::time::Duration;

mod api;
mod downloader;
mod hosts;
mod websocket;
mod db;
mod config;
mod utils;
mod arr;
mod constants;
mod error;
mod services;


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
    /// Movie collection parts: (title, year, tmdb_id, poster)
    pub collections: Vec<(String, String, u64, Option<String>)>,
}

#[derive(Clone)]
pub struct AppState {
    pub host_registry: Arc<hosts::registry::HostRegistry>,
    pub download_orchestrator: Arc<downloader::DownloadOrchestrator>,
    pub download_service: Arc<services::DownloadService>,
    pub tmdb_service: Arc<services::TmdbService>,
    pub tx_broadcast: tokio::sync::broadcast::Sender<downloader::task::DownloadTask>,
    pub config: config::Config,
    pub db: Arc<db::Db>,
    pub search_cache: Cache<String, api::smart_search::SmartSearchResponse>,
    pub tmdb_cache: Cache<String, TmdbEnrichmentCache>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "flasharr=debug,tower_http=debug".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Flasharr v{}", env!("CARGO_PKG_VERSION"));

    // Create appData directory structure if needed
    if let Err(e) = config::ensure_appdata_dirs() {
        tracing::warn!("Failed to create appData directories: {}. Continuing with legacy paths.", e);
    }

    // Get paths with fallback to old locations
    let config_path = config::get_config_path();
    let db_path = config::get_db_path();
    
    tracing::info!("Loading config from: {}", config_path.display());
    tracing::info!("Using database at: {}", db_path.display());

    // Load Config
    let config: config::Config = figment::Figment::new()
        .merge(figment::providers::Serialized::defaults(config::Config::default()))
        .merge(figment::providers::Toml::file(config_path))
        .merge(figment::providers::Env::prefixed("FLASHARR_"))
        .extract()
        .expect("Failed to load configuration");

    // Initialize Database
    let db = Arc::new(db::Db::new(&db_path).expect("Failed to initialize database"));
    
    // Ensure indexer API key exists (generate if needed)
    if db.get_setting("indexer_api_key").ok().flatten().is_none() {
        let api_key = format!("flasharr_{}", uuid::Uuid::new_v4().simple());
        if let Err(e) = db.save_setting("indexer_api_key", &api_key) {
            tracing::warn!("Failed to save generated API key: {}", e);
        } else {
            tracing::info!("Generated new indexer API key: {}", api_key);
        }
    }

    // Initialize components
    let (tx_broadcast, _) = tokio::sync::broadcast::channel(100);
    let shared_http_client = hosts::create_shared_client();
    let host_registry = Arc::new(hosts::create_registry(&config, shared_http_client, Arc::clone(&db)));
    
    // Create download config from app config
    let download_config = downloader::config::DownloadConfig {
        max_concurrent: config.downloads.max_concurrent,
        segments_per_download: config.downloads.segments_per_download as usize,
        download_dir: config.downloads.directory.clone(),
        chunk_size: 1024 * 1024, // 1 MB
        retry_attempts: 3,
        retry_backoff_base: 30,
        retry_max_wait: 300,
        retry: downloader::config::RetryConfig::default(),
    };
    
    // Load Arr configuration from database (setup wizard saves here)
    // This ensures webhook handler initializes correctly even if settings were saved before
    let sonarr_config = if let (Some(url), Some(api_key)) = (
        db.get_setting("sonarr_url").ok().flatten(),
        db.get_setting("sonarr_api_key").ok().flatten()
    ) {
        if !url.is_empty() && !api_key.is_empty() {
            tracing::info!("Loading Sonarr config from database: {}", url);
            Some(config::ArrConfig {
                enabled: true,
                url,
                api_key,
                auto_import: true,
            })
        } else {
            config.sonarr.clone()
        }
    } else {
        config.sonarr.clone()
    };
    
    let radarr_config = if let (Some(url), Some(api_key)) = (
        db.get_setting("radarr_url").ok().flatten(),
        db.get_setting("radarr_api_key").ok().flatten()
    ) {
        if !url.is_empty() && !api_key.is_empty() {
            tracing::info!("Loading Radarr config from database: {}", url);
            Some(config::ArrConfig {
                enabled: true,
                url,
                api_key,
                auto_import: true,
            })
        } else {
            config.radarr.clone()
        }
    } else {
        config.radarr.clone()
    };
    
    // Create download orchestrator with new architecture
    let download_orchestrator = Arc::new(downloader::DownloadOrchestrator::new(
        download_config,
        Arc::clone(&host_registry),
        Some(Arc::clone(&db)),
        sonarr_config.clone(),
        radarr_config.clone(),
    ));
    
    // Start orchestrator workers
    download_orchestrator.start().await;
    tracing::info!("Download orchestrator started with new architecture");
    
    // Load pending tasks (QUEUED, PAUSED) from database into TaskManager
    // This ensures resume/pause operations work correctly via WebSocket updates
    let pending_count = download_orchestrator.load_pending_tasks().await;
    tracing::info!("Loaded {} pending tasks from database", pending_count);

    
    // Initialize Caches
    let search_cache = Cache::builder()
        .max_capacity(100)
        .time_to_live(Duration::from_secs(600)) // 10 minutes — keeps episode counts fresh for ongoing shows
        .build();

    let tmdb_cache = Cache::builder()
        .max_capacity(500)
        .time_to_live(Duration::from_secs(86400)) // 24 hours
        .build();
    
    // Create DownloadService for business logic abstraction
    let download_service = Arc::new(services::DownloadService::new(
        Arc::clone(&db),
        Arc::clone(&download_orchestrator),
    ));
    
    // Create TmdbService for centralized TMDB API access
    let tmdb_service = Arc::new(services::TmdbService::new_with_default_client());
    
    // Update config with loaded Arr settings so API sees them
    let mut app_config = config.clone();
    app_config.sonarr = sonarr_config.clone();
    app_config.radarr = radarr_config.clone();

    let state = Arc::new(AppState { 
        host_registry,
        download_orchestrator,
        download_service,
        tmdb_service,
        tx_broadcast,
        config: app_config,
        db,
        search_cache,
        tmdb_cache,
    });

    // Build router
    let app = Router::new()
        .route("/health", get(health))
        .route("/api/health", get(health))
        .route("/api/health/status", get(api::health::health_status))
        .route("/api/ws", get(websocket::handler))
        .nest("/api/downloads", api::downloads::router())
        .nest("/api/stats", api::stats::router())
        .nest("/api/system", api::system::router())
        .nest("/api/search", api::search::router())
        .nest("/api/accounts", api::accounts::router())
        .nest("/api/settings", api::settings::router())
        .nest("/api/tmdb", api::tmdb::router())
        .nest("/api/discovery", api::discovery::router())
        .nest("/api/setup", api::setup::router())
        .nest("/sabnzbd", api::sabnzbd::router())
        .nest("/api/indexer", api::indexer::router())
        .nest("/newznab/api", api::indexer::router())  // Standard Newznab path
        .nest("/api/arr", api::arr::router())
        .nest("/api/media", api::media::router())
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        )
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .fallback_service(ServeDir::new("static").not_found_service(ServeFile::new("static/index.html")))
        .with_state(state);

    // Run server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    tracing::info!("Listening on {}", addr);
    
    // Create socket with SO_REUSEADDR to allow immediate restart after crash
    use socket2::{Socket, Domain, Type};
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None)
        .expect("Failed to create socket");
    socket.set_reuse_address(true)
        .expect("Failed to set SO_REUSEADDR");
    socket.bind(&addr.into())
        .expect("Failed to bind socket");
    socket.listen(1024)
        .expect("Failed to listen on socket");
    
    // Set non-blocking mode before converting to tokio
    socket.set_nonblocking(true)
        .expect("Failed to set non-blocking mode");
    
    // Convert to tokio listener
    let listener = tokio::net::TcpListener::from_std(socket.into())
        .expect("Failed to convert to tokio listener");
    
    axum::serve(listener, app).await.unwrap();
}
