pub mod fshare;
pub mod model;
pub mod search;
pub mod torbox;
pub mod torrent;

use crate::AppState;
use model::{
    DownloadSelection, ExecutionStrategy, MaterializedDownload, SourceFailure, SourceKind,
};
use moka::future::Cache;
use std::sync::Arc;

pub async fn materialize_download(
    state: &Arc<AppState>,
    refs: &Cache<String, String>,
    selection: DownloadSelection,
) -> Result<MaterializedDownload, (axum::http::StatusCode, SourceFailure)> {
    let strategy = selection.strategy;
    let locator = selection
        .url
        .as_deref()
        .or(selection.download_ref.as_deref());
    match strategy {
        ExecutionStrategy::TorboxHttp => return torbox::materialize(state, selection).await,
        ExecutionStrategy::LocalQb => {}
    }
    match SourceKind::resolve(
        selection.source.as_deref(),
        selection.provider.as_deref(),
        locator,
    )
    .map_err(|failure| (axum::http::StatusCode::BAD_REQUEST, failure))?
    {
        SourceKind::Fshare => fshare::materialize(state, selection).await,
        SourceKind::Torrent => torrent::materialize(state, refs, selection).await,
        SourceKind::Torbox => torbox::materialize(state, selection).await,
        SourceKind::Stremio => Err((
            axum::http::StatusCode::BAD_REQUEST,
            SourceFailure::invalid("stremio is discovery state, not a download source"),
        )),
    }
}
