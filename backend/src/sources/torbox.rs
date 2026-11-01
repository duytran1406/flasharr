use super::model::{DownloadSelection, MaterializedDownload, SourceFailure, SourceKind};
use crate::debrid::selection::{select_files, Episode, ProviderFile, SelectionInput};
use crate::torbox::client::TorBoxClient;
use crate::AppState;
use futures_util::{stream, StreamExt};
use std::{sync::Arc, time::Duration};

const METADATA_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CacheLookupError {
    Unavailable,
    Partial,
}

impl std::fmt::Display for CacheLookupError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => "TorBox cache check failed",
            Self::Partial => "TorBox cache check timed out",
        })
    }
}

#[derive(Debug)]
pub(crate) struct CacheLookupOutcome {
    pub(crate) hits: std::collections::HashMap<String, crate::torbox::client::TorBoxCacheHit>,
    pub(crate) status: Option<CacheLookupError>,
}

impl CacheLookupOutcome {
    fn complete(
        hits: std::collections::HashMap<String, crate::torbox::client::TorBoxCacheHit>,
    ) -> Self {
        Self { hits, status: None }
    }

    fn degraded(
        hits: std::collections::HashMap<String, crate::torbox::client::TorBoxCacheHit>,
        status: CacheLookupError,
    ) -> Self {
        Self {
            hits,
            status: Some(status),
        }
    }
}

fn metadata_enrichment_limits(cache_timeout_secs: u64) -> (usize, Duration) {
    (
        METADATA_CONCURRENCY,
        Duration::from_secs(cache_timeout_secs.max(1)),
    )
}

#[derive(Debug, PartialEq, Eq)]
enum MetadataEntry {
    Hit(crate::torbox::client::TorBoxCacheHit),
    Miss,
    Incomplete,
}

fn metadata_entry(
    cache_hit: Option<crate::torbox::client::TorBoxCacheHit>,
    metadata: Result<Option<crate::torbox::client::TorBoxCacheHit>, ()>,
) -> MetadataEntry {
    match cache_hit {
        Some(hit) if has_usable_metadata(&hit) => MetadataEntry::Hit(hit),
        Some(_) => match metadata {
            Ok(Some(hit)) if has_usable_metadata(&hit) => MetadataEntry::Hit(hit),
            Ok(_) => MetadataEntry::Incomplete,
            Err(()) => MetadataEntry::Incomplete,
        },
        None => MetadataEntry::Miss,
    }
}

async fn enrich_cache_hit(
    client: TorBoxClient,
    hash: String,
    hit: Option<crate::torbox::client::TorBoxCacheHit>,
    timeout: Duration,
) -> (String, MetadataEntry) {
    let metadata = match hit.as_ref().filter(|hit| !has_usable_metadata(hit)) {
        Some(_) => match tokio::time::timeout(timeout, client.torrent_info(&hash)).await {
            Ok(Ok(hit)) => Ok(Some(hit)),
            Ok(Err(_)) | Err(_) => Err(()),
        },
        None => Ok(None),
    };
    (hash, metadata_entry(hit, metadata))
}

async fn enrich_cache_hits<F>(
    hashes: Vec<String>,
    deadline: tokio::time::Instant,
    fetch: F,
) -> Vec<(String, MetadataEntry)>
where
    F: Fn(String) -> futures_util::future::BoxFuture<'static, (String, MetadataEntry)>,
{
    let (tx, mut rx) = tokio::sync::mpsc::channel(hashes.len().max(1));
    let work_tx = tx.clone();
    let work = stream::iter(hashes.into_iter().map(move |hash| {
        let tx = work_tx.clone();
        let future = fetch(hash);
        async move {
            let _ = tx.send(future.await).await;
        }
    }))
    .buffer_unordered(METADATA_CONCURRENCY)
    .collect::<Vec<_>>();
    drop(tx);
    tokio::pin!(work);
    let _ = tokio::time::timeout_at(deadline, &mut work).await;
    let mut completed = Vec::new();
    while let Ok(item) = rx.try_recv() {
        completed.push(item);
    }
    completed
}

fn finalize_cache_lookup(
    mut hits: std::collections::HashMap<String, crate::torbox::client::TorBoxCacheHit>,
    completed: Vec<(String, MetadataEntry)>,
    expected: usize,
) -> CacheLookupOutcome {
    let mut status = (completed.len() != expected).then_some(CacheLookupError::Partial);
    for (hash, entry) in completed {
        match entry {
            MetadataEntry::Hit(hit) => {
                hits.insert(hash, hit);
            }
            MetadataEntry::Miss => {}
            MetadataEntry::Incomplete => status = Some(CacheLookupError::Unavailable),
        }
    }
    match status {
        Some(status) => CacheLookupOutcome::degraded(hits, status),
        None => CacheLookupOutcome::complete(hits),
    }
}

async fn cached_hits_from_cache_until<F, Fut>(
    cache: &moka::future::Cache<String, Option<crate::torbox::client::TorBoxCacheHit>>,
    hashes: std::collections::BTreeSet<String>,
    deadline: tokio::time::Instant,
    fetch_missing: F,
) -> CacheLookupOutcome
where
    F: FnOnce(Vec<String>) -> Fut,
    Fut: std::future::Future<Output = CacheLookupOutcome>,
{
    let mut hits = std::collections::HashMap::new();
    let mut missing = Vec::new();
    for hash in hashes {
        match cache.get(&hash).await {
            Some(Some(hit)) if has_usable_metadata(&hit) => {
                hits.insert(hash, hit);
            }
            Some(None) => {}
            Some(Some(_)) | None => missing.push(hash),
        }
    }
    if missing.is_empty() {
        return CacheLookupOutcome::complete(hits);
    }
    let remote = match tokio::time::timeout_at(deadline, fetch_missing(missing)).await {
        Ok(outcome) => outcome,
        Err(_) => return CacheLookupOutcome::degraded(hits, CacheLookupError::Partial),
    };
    hits.extend(remote.hits);
    match remote.status {
        Some(status) => CacheLookupOutcome::degraded(hits, status),
        None => CacheLookupOutcome::complete(hits),
    }
}

pub(crate) async fn cached_hits_until(
    state: &Arc<AppState>,
    hashes: &[String],
    deadline: tokio::time::Instant,
) -> CacheLookupOutcome {
    let settings = crate::api::settings::load_torbox_settings_public(state);
    if !settings.enabled || !settings.show_cached_as_source {
        return CacheLookupOutcome::complete(Default::default());
    }
    if crate::api::settings::validate_api_key(&settings.api_key).is_err() {
        return CacheLookupOutcome::degraded(Default::default(), CacheLookupError::Unavailable);
    }
    let hashes = hashes
        .iter()
        .map(|hash| hash.trim().to_ascii_lowercase())
        .filter(|hash| hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .collect::<std::collections::BTreeSet<_>>();
    cached_hits_from_cache_until(
        &state.torbox_cache_hits,
        hashes,
        deadline,
        |missing| async move {
            let Ok((client, cache_timeout_secs)) = configured_client(state) else {
                return CacheLookupOutcome::degraded(
                    Default::default(),
                    CacheLookupError::Unavailable,
                );
            };
            let cache_deadline = deadline.min(
                tokio::time::Instant::now() + std::time::Duration::from_secs(cache_timeout_secs),
            );
            let fetched =
                match tokio::time::timeout_at(cache_deadline, client.check_cache(&missing)).await {
                    Ok(Ok(hits)) => hits,
                    Ok(Err(_)) => {
                        return CacheLookupOutcome::degraded(
                            Default::default(),
                            CacheLookupError::Unavailable,
                        )
                    }
                    Err(_) => {
                        return CacheLookupOutcome::degraded(
                            Default::default(),
                            CacheLookupError::Partial,
                        )
                    }
                };
            let (_, metadata_timeout) = metadata_enrichment_limits(cache_timeout_secs);
            let metadata_deadline = deadline.min(tokio::time::Instant::now() + metadata_timeout);
            let fetched = Arc::new(fetched);
            let completed = enrich_cache_hits(missing.clone(), metadata_deadline, move |hash| {
                let client = client.clone();
                let hit = fetched.get(&hash).cloned();
                Box::pin(enrich_cache_hit(client, hash, hit, metadata_timeout))
            })
            .await;
            for (hash, entry) in &completed {
                match entry {
                    MetadataEntry::Hit(hit) => {
                        state
                            .torbox_cache_hits
                            .insert(hash.clone(), Some(hit.clone()))
                            .await;
                    }
                    MetadataEntry::Miss => {
                        state.torbox_cache_hits.insert(hash.clone(), None).await;
                    }
                    MetadataEntry::Incomplete => {}
                }
            }
            finalize_cache_lookup(Default::default(), completed, missing.len())
        },
    )
    .await
}

pub(crate) async fn cached_hits(state: &Arc<AppState>, hashes: &[String]) -> CacheLookupOutcome {
    cached_hits_until(
        state,
        hashes,
        tokio::time::Instant::now() + std::time::Duration::from_secs(10),
    )
    .await
}

fn has_usable_metadata(hit: &crate::torbox::client::TorBoxCacheHit) -> bool {
    hit.size > 0 || hit.files.iter().any(|file| file.size > 0)
}

pub(crate) fn resolved_metadata(
    hit: &crate::torbox::client::TorBoxCacheHit,
    file_idx: Option<u32>,
) -> (Option<&crate::torbox::client::TorBoxFile>, Option<u64>) {
    if let Some(file) = file_idx
        .and_then(|index| hit.files.get(index as usize))
        .filter(|file| file.size > 0)
    {
        return (Some(file), Some(file.size));
    }
    if hit.size > 0 {
        return (None, Some(hit.size));
    }
    // Display-only fallback. Acquisition still requires exact `file_idx`/episode selection.
    let file = crate::torbox::client::cache_display_file(hit).filter(|file| file.size > 0);
    (file, file.map(|file| file.size))
}

pub async fn cached_hashes(
    state: &Arc<AppState>,
    hashes: &[String],
) -> Result<std::collections::HashSet<String>, String> {
    let outcome = cached_hits(state, hashes).await;
    outcome
        .status
        .map(|error| Err(error.to_string()))
        .unwrap_or_else(|| Ok(outcome.hits.into_keys().collect()))
}

fn configured_client(state: &Arc<AppState>) -> Result<(TorBoxClient, u64), CacheLookupError> {
    let settings = crate::api::settings::load_torbox_settings_public(state);
    if !settings.enabled || crate::api::settings::validate_api_key(&settings.api_key).is_err() {
        return Err(CacheLookupError::Unavailable);
    }
    Ok((
        TorBoxClient::new(
            settings.base_url,
            settings.api_key,
            Arc::clone(&state.http_client),
        ),
        settings.cache_timeout_secs,
    ))
}

pub fn parse_remote_id(value: &str) -> Option<(u64, u64)> {
    let value = value.strip_prefix("torbox:")?;
    let (torrent_id, file_id) = value.split_once(':')?;
    Some((torrent_id.parse().ok()?, file_id.parse().ok()?))
}

pub async fn materialize(
    state: &Arc<AppState>,
    selection: DownloadSelection,
) -> Result<MaterializedDownload, (axum::http::StatusCode, SourceFailure)> {
    let hash = selection
        .info_hash
        .as_deref()
        .map(str::trim)
        .filter(|hash| !hash.is_empty())
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| SourceFailure::bad_request("info_hash is required for TorBox downloads"))?;
    let (client, cache_timeout_secs) = match configured_client(state) {
        Ok(configured) => configured,
        Err(error) => return Err(SourceFailure::torbox(error.to_string(), hash)),
    };
    let hits = match tokio::time::timeout(
        std::time::Duration::from_secs(cache_timeout_secs),
        client.check_cache(&[hash.clone()]),
    )
    .await
    {
        Ok(Ok(hits)) => hits,
        Ok(Err(error)) => {
            return Err(SourceFailure::torbox(
                format!("TorBox cache check failed: {error}"),
                hash,
            ))
        }
        Err(_) => return Err(SourceFailure::torbox("TorBox cache check timed out", hash)),
    };
    match hits.get(&hash) {
        Some(_) => {}
        None => return Err(SourceFailure::torbox("TorBox cache miss", hash)),
    }
    let torrent_id = match client.create_cached_torrent(&hash).await {
        Ok(torrent_id) => torrent_id,
        Err(error) => {
            return Err(SourceFailure::torbox(
                format!("TorBox create torrent failed: {error}"),
                hash,
            ))
        }
    };
    let details = client
        .poll_remote_ready(
            torrent_id,
            std::time::Duration::from_secs(1),
            std::time::Duration::from_secs(cache_timeout_secs),
        )
        .await
        .map_err(|error| {
            SourceFailure::torbox(format!("TorBox readiness failed: {error}"), hash.clone())
        })?;
    let episodes = selection
        .selected_episodes
        .as_deref()
        .map(|json| serde_json::from_str::<Vec<Episode>>(json))
        .transpose()
        .map_err(|_| SourceFailure::bad_request("selected_episodes is invalid"))?
        .unwrap_or_default();
    let input = SelectionInput::new(
        &hash,
        selection.file_idx.map(|value| value as u64),
        selection.source_filename.as_deref(),
        episodes,
    );
    let manifest = details
        .files
        .iter()
        .map(|file| ProviderFile {
            id: file.id,
            name: file.name.clone(),
            size: file.size,
        })
        .collect::<Vec<_>>();
    let selected = select_files(&input, &manifest).map_err(|error| {
        SourceFailure::torbox(
            format!("TorBox exact selection failed: {error:?}"),
            hash.clone(),
        )
    })?;
    if selected.len() != 1 {
        return Err(SourceFailure::torbox(
            "TorBox multi-file acquisition is not representable by the current download task",
            hash,
        ));
    }
    let file = &selected[0];
    let file_id = file.id;
    let size = Some(file.size);
    let queue_url = match client.get_download_link(torrent_id, file_id).await {
        Ok(url) => url,
        Err(error) => {
            return Err(SourceFailure::torbox(
                format!("TorBox link failed: {error}"),
                hash,
            ));
        }
    };

    Ok(MaterializedDownload {
        queue_url,
        host: "torbox".to_string(),
        source: SourceKind::Torbox,
        provider: "torbox".to_string(),
        size,
        external_id: Some(format!("torbox:{torrent_id}:{file_id}")),
        info_hash: Some(hash),
        execution_strategy: super::model::ExecutionStrategy::TorboxHttp,
        remote_state: Some(details.state),
        remote_completed_at: details.download_finished.then(|| chrono::Utc::now().to_rfc3339()),
        selected_remote_file_ids: Some(serde_json::to_string(&[file_id]).expect("file IDs serialize")),
        selected_artifacts: Some(serde_json::to_string(&selected.iter().map(|file| serde_json::json!({"remoteFileId": file.id, "filename": file.name, "size": file.size})).collect::<Vec<_>>()).expect("artifacts serialize")),
    })
}

pub async fn refresh_link(state: &Arc<AppState>, remote_id: &str) -> anyhow::Result<String> {
    let (torrent_id, file_id) =
        parse_remote_id(remote_id).ok_or_else(|| anyhow::anyhow!("invalid TorBox remote ID"))?;
    let (client, _) =
        configured_client(state).map_err(|error| anyhow::Error::msg(error.to_string()))?;
    client.get_download_link(torrent_id, file_id).await
}

pub async fn cleanup(state: &Arc<AppState>, remote_id: &str) -> anyhow::Result<()> {
    let (torrent_id, _) =
        parse_remote_id(remote_id).ok_or_else(|| anyhow::anyhow!("invalid TorBox remote ID"))?;
    let (client, _) =
        configured_client(state).map_err(|error| anyhow::Error::msg(error.to_string()))?;
    client.delete_torrent(torrent_id).await
}

fn client_from_db(db: &crate::db::Db) -> anyhow::Result<TorBoxClient> {
    let enabled = db
        .get_setting("torbox_enabled")?
        .as_deref()
        .and_then(|value| value.parse::<bool>().ok())
        .unwrap_or(false);
    let api_key = db.get_setting("torbox_api_key")?.unwrap_or_default();
    if !enabled {
        anyhow::bail!("TorBox is not configured");
    }
    crate::api::settings::validate_api_key(&api_key).map_err(anyhow::Error::msg)?;
    let base_url = db
        .get_setting("torbox_base_url")?
        .unwrap_or_else(|| "https://api.torbox.app".to_string());
    Ok(TorBoxClient::new(
        base_url,
        api_key,
        Arc::new(reqwest::Client::new()),
    ))
}

pub async fn refresh_link_from_db(db: &crate::db::Db, remote_id: &str) -> anyhow::Result<String> {
    let (torrent_id, file_id) =
        parse_remote_id(remote_id).ok_or_else(|| anyhow::anyhow!("invalid TorBox remote ID"))?;
    client_from_db(db)?
        .get_download_link(torrent_id, file_id)
        .await
}

pub async fn resume_link_from_db(
    db: &crate::db::Db,
    remote_id: &str,
    deadline: std::time::Duration,
) -> anyhow::Result<String> {
    let (torrent_id, file_id) =
        parse_remote_id(remote_id).ok_or_else(|| anyhow::anyhow!("invalid TorBox remote ID"))?;
    let client = client_from_db(db)?;
    client
        .poll_remote_ready(torrent_id, std::time::Duration::from_secs(1), deadline)
        .await?;
    client.get_download_link(torrent_id, file_id).await
}

pub async fn cleanup_from_db(db: &crate::db::Db, remote_id: &str) -> anyhow::Result<()> {
    let (torrent_id, _) =
        parse_remote_id(remote_id).ok_or_else(|| anyhow::anyhow!("invalid TorBox remote ID"))?;
    client_from_db(db)?.delete_torrent(torrent_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(
        size: u64,
        files: Vec<crate::torbox::client::TorBoxFile>,
    ) -> crate::torbox::client::TorBoxCacheHit {
        crate::torbox::client::TorBoxCacheHit {
            hash: "abc".into(),
            name: None,
            size,
            files,
        }
    }

    fn file(id: u64, name: &str, size: u64) -> crate::torbox::client::TorBoxFile {
        crate::torbox::client::TorBoxFile {
            id,
            name: name.into(),
            size,
            mimetype: None,
        }
    }

    #[test]
    fn cache_metadata_prefers_exact_file_then_hit_size_then_largest_media() {
        let files = vec![file(1, "first.mkv", 10), file(2, "second.mkv", 20)];
        let with_total = hit(99, files.clone());
        assert_eq!(resolved_metadata(&with_total, Some(0)).1, Some(10));
        assert_eq!(resolved_metadata(&with_total, None), (None, Some(99)));

        let without_total = hit(0, files);
        assert_eq!(resolved_metadata(&without_total, None).1, Some(20));
        assert!(has_usable_metadata(&without_total));
        assert!(!has_usable_metadata(&hit(0, vec![file(1, "empty.mkv", 0)])));
    }

    #[tokio::test]
    async fn cache_lookup_owner_retains_local_hit_when_remote_stalls() {
        let local_hash = "a".repeat(40);
        let remote_hash = "b".repeat(40);
        let cache = moka::future::Cache::new(2);
        cache.insert(local_hash.clone(), Some(hit(1, vec![]))).await;

        let outcome = cached_hits_from_cache_until(
            &cache,
            std::collections::BTreeSet::from([local_hash.clone(), remote_hash]),
            tokio::time::Instant::now() + Duration::from_millis(10),
            |_| async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                CacheLookupOutcome::complete(Default::default())
            },
        )
        .await;

        assert_eq!(outcome.status, Some(CacheLookupError::Partial));
        assert_eq!(outcome.hits.len(), 1);
        assert!(outcome.hits.contains_key(&local_hash));
    }

    #[test]
    fn metadata_timeout_is_partial_not_a_negative_cache_miss() {
        assert_eq!(
            metadata_entry(Some(hit(0, vec![])), Err(())),
            MetadataEntry::Incomplete
        );
    }

    #[test]
    fn metadata_entry_marks_non_cached_hash_as_negative_cacheable_miss() {
        assert_eq!(metadata_entry(None, Ok(None)), MetadataEntry::Miss);
    }

    #[test]
    fn metadata_enrichment_limits_are_configured_and_bounded() {
        assert_eq!(metadata_enrichment_limits(0), (4, Duration::from_secs(1)));
        assert_eq!(metadata_enrichment_limits(3), (4, Duration::from_secs(3)));
    }

    #[tokio::test]
    async fn metadata_stage_returns_completed_hits_at_deadline() {
        let completed = enrich_cache_hits(
            vec!["fast".into(), "slow".into()],
            tokio::time::Instant::now() + Duration::from_millis(20),
            |hash| {
                Box::pin(async move {
                    if hash == "slow" {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    (hash, MetadataEntry::Hit(hit(1, Vec::new())))
                })
            },
        )
        .await;

        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].0, "fast");
    }

    #[test]
    fn parses_remote_id() {
        assert_eq!(parse_remote_id("torbox:12:34"), Some((12, 34)));
        assert_eq!(parse_remote_id("12"), None);
        assert_eq!(parse_remote_id("x:34"), None);
    }
}
