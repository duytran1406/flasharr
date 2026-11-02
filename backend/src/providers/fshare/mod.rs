mod accounts;
mod compatibility;
mod folder;
mod folder_cache;
mod host;
mod search;
pub mod season_search;
mod setup;
mod status;

use crate::{config::Config, db::Db, hosts::registry::HostRegistry};
use reqwest::Client;
use std::sync::Arc;

pub use accounts::{canonical_router as canonical_accounts_router, router as accounts_router};
pub use compatibility::{
    active_search_backend, decode_fake_nzb, encode_fake_nzb, is_fshare_url, parse_locator,
    AcquisitionSource, FakeNzbMetadata, FshareSearchBackend, Locator, LocatorKind,
};
pub use folder::router as folder_router;
pub use folder_cache::{expand_folder_cache_query_variants, FolderCacheService, SyncStatus};
pub use host::FshareHandler;
pub use search::{is_media_file, is_safe_search_result, search_folder_cache, ProviderSearchResult};
pub use setup::setup;
pub use status::{is_available, status};

pub fn item_id_from_url(url: &str) -> Option<String> {
    parse_locator(url).map(|locator| locator.item_id)
}

pub fn register(registry: &mut HostRegistry, config: &Config, client: Arc<Client>, db: Arc<Db>) {
    registry.register(Box::new(
        FshareHandler::new(config.fshare.clone(), client).with_db(db),
    ));
}
