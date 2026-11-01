use super::model::{DownloadSelection, MaterializedDownload, SourceFailure, SourceKind};
use crate::AppState;
use std::sync::Arc;

pub async fn materialize(
    _state: &Arc<AppState>,
    selection: DownloadSelection,
) -> Result<MaterializedDownload, (axum::http::StatusCode, SourceFailure)> {
    let queue_url = selection
        .url
        .or(selection.download_ref)
        .filter(|url| !url.trim().is_empty())
        .ok_or_else(|| SourceFailure::bad_request("url is required for Fshare downloads"))?;
    crate::providers::fshare::parse_locator(&queue_url)
        .ok_or_else(|| SourceFailure::bad_request("canonical Fshare URL is required"))?;

    Ok(MaterializedDownload {
        queue_url,
        host: "fshare".to_string(),
        source: SourceKind::Fshare,
        provider: "fshare".to_string(),
        size: selection.size,
        external_id: selection.external_id,
        info_hash: selection.info_hash,
        execution_strategy: super::model::ExecutionStrategy::LocalQb,
        remote_state: None,
        remote_completed_at: None,
        selected_remote_file_ids: None,
        selected_artifacts: None,
    })
}
