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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FshareConfig {
    pub email: String,
    pub password: String,
    pub session_id: Option<String>,
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
            },
            sonarr: None,
            radarr: None,
            indexer: Some(IndexerConfig {
                enabled: true,
                api_key: "flasharr-default-key".to_string(),
            }),
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

/// Get the database file path with fallback
/// Tries: appData/data/flasharr.db -> flasharr.db (old location)
pub fn get_db_path() -> PathBuf {
    let appdata_db = get_appdata_dir().join("data/flasharr.db");
    if appdata_db.exists() {
        appdata_db
    } else {
        PathBuf::from("flasharr.db")
    }
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
