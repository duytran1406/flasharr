pub mod registry;
pub mod base;
pub mod fshare;

use std::sync::Arc;
use reqwest::Client;
use crate::config::Config;
use crate::db::Db;
use self::registry::HostRegistry;
use self::fshare::FshareHandler;

/// Create a shared HTTP client for all host handlers
pub fn create_shared_client() -> Arc<Client> {
    Arc::new(
        Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .pool_max_idle_per_host(10)
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client")
    )
}

pub fn create_registry(config: &Config, client: Arc<Client>, db: Arc<Db>) -> HostRegistry {
    let mut registry = HostRegistry::new();
    registry.register(Box::new(
        FshareHandler::new(config.fshare.clone(), client)
            .with_db(db)
    ));
    registry
}
