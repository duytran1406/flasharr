//! Configuration module with appData support

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::env;

/// Application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub downloads: DownloadsConfig,
    pub fshare: FshareConfig,
    pub sonarr: Option<ArrConfig>,
    pub radarr: Option<ArrConfig>,
    pub indexer: Option<IndexerConfig>,
    #[serde(default)]
    pub external: ExternalConfig,
}

/// External API and cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalConfig {
    /// TMDB API key for metadata enrichment (env: TMDB_API_KEY)
    pub tmdb_api_key: String,
    /// Cache TTL for TMDB queries in seconds (default: 3600)
    pub tmdb_cache_ttl_secs: u64,
    /// Cache TTL for Fshare search results in seconds (default: 300)
    pub fshare_cache_ttl_secs: u64,
}

impl Default for ExternalConfig {
    fn default() -> Self {
        Self {
            // Use env var if set, otherwise fallback to the bundled key
            tmdb_api_key: env::var("TMDB_API_KEY")
                .unwrap_or_else(|_| "8d95150f3391194ca66fef44df497ad6".to_string()),
            tmdb_cache_ttl_secs: 3600,
            fshare_cache_ttl_secs: 300,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FshareConfig {
    pub email: String,
    pub password: String,
    pub session_id: Option<String>,
    /// Skip primary API (download.fsharegroup.site) and use api.fshare.vn directly
    /// Default: true (since primary API is currently down)
    #[serde(default = "default_prefer_api2")]
    pub prefer_api2: bool,
}

fn default_prefer_api2() -> bool {
    // Check env var, default to true
    env::var("FSHARE_PREFER_API2")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(true)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadsConfig {
    pub directory: PathBuf,
    pub max_concurrent: usize,
    pub segments_per_download: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrConfig {
    pub enabled: bool,
    pub url: String,
    pub api_key: String,
    pub auto_import: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexerConfig {
    pub enabled: bool,
    pub api_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8484,
            },
            downloads: DownloadsConfig {
                directory: PathBuf::from("/downloads"),
                max_concurrent: 3,
                segments_per_download: 4,
            },
            fshare: FshareConfig {
                email: "".to_string(),
                password: "".to_string(),
                session_id: None,
                prefer_api2: default_prefer_api2(),
            },
            sonarr: None,
            radarr: None,
            indexer: Some(IndexerConfig {
                enabled: true,
                api_key: "flasharr-default-key".to_string(),
            }),
            external: ExternalConfig::default(),
        }
    }
}

/// Get the appData directory path
/// Priority: FLASHARR_APPDATA_DIR env var > ./appData
pub fn get_appdata_dir() -> PathBuf {
    env::var("FLASHARR_APPDATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("./appData"))
}

/// Get the config file path with fallback
/// Tries: appData/config/config.toml -> config.toml (old location)
pub fn get_config_path() -> PathBuf {
    let appdata_config = get_appdata_dir().join("config/config.toml");
    if appdata_config.exists() {
        appdata_config
    } else {
        PathBuf::from("config.toml")
    }
}

/// Get the database file path
/// Always uses: appData/data/flasharr.db (creates directory if needed)
pub fn get_db_path() -> PathBuf {
    let appdata_dir = get_appdata_dir();
    let data_dir = appdata_dir.join("data");
    
    // Create data directory if it doesn't exist
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir).ok();
    }
    
    data_dir.join("flasharr.db")
}

/// Create appData directory structure if it doesn't exist
pub fn ensure_appdata_dirs() -> std::io::Result<()> {
    let appdata_dir = get_appdata_dir();
    
    std::fs::create_dir_all(appdata_dir.join("config"))?;
    std::fs::create_dir_all(appdata_dir.join("data"))?;
    std::fs::create_dir_all(appdata_dir.join("downloads"))?;
    std::fs::create_dir_all(appdata_dir.join("logs"))?;
    
    Ok(())
}

/// Save configuration to config.toml
pub fn save_config(config: &Config) -> anyhow::Result<()> {
    let config_path = get_config_path();
    
    // Ensure parent directory exists
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    // Serialize to TOML
    let toml_string = toml::to_string_pretty(config)?;
    
    // Write to file
    std::fs::write(&config_path, toml_string)?;
    
    tracing::info!("Configuration saved to {:?}", config_path);
    Ok(())
}
