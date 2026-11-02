use crate::torrent::info_hash::torrent_info_hash;
use anyhow::{anyhow, Result};
use reqwest::{
    header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE},
    multipart,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub struct QBittorrentConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub category: Option<String>,
    pub save_path: Option<String>,
    pub poll_interval_secs: u64,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
}

impl QBittorrentConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            url: "http://localhost:8080".to_string(),
            username: String::new(),
            password: String::new(),
            category: None,
            save_path: None,
            poll_interval_secs: 5,
            instance_id: None,
            instance_name: None,
        }
    }
}

#[derive(Clone)]
pub struct QBittorrentClient {
    config: QBittorrentConfig,
    client: reqwest::Client,
    sid: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QBittorrentInfo {
    pub hash: String,
    pub name: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub dlspeed: i64,
    #[serde(default)]
    pub eta: i64,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub save_path: String,
    #[serde(default)]
    pub content_path: String,
    #[serde(default)]
    pub num_seeds: Option<i64>,
    #[serde(default)]
    pub num_leechs: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QBittorrentTorrent {
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub dlspeed: i64,
    #[serde(default)]
    pub upspeed: i64,
    #[serde(default)]
    pub eta: i64,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub save_path: String,
    #[serde(default)]
    pub content_path: String,
    #[serde(default)]
    pub tracker: String,
    #[serde(default)]
    pub added_on: i64,
    #[serde(default)]
    pub completion_on: i64,
    #[serde(default)]
    pub downloaded: i64,
    #[serde(default)]
    pub uploaded: i64,
    #[serde(default)]
    pub amount_left: i64,
    #[serde(default)]
    pub num_seeds: i64,
    #[serde(default)]
    pub num_leechs: i64,
    #[serde(default)]
    pub availability: f64,
    #[serde(default)]
    pub ratio: f64,
    #[serde(default)]
    pub seen_complete: i64,
    #[serde(default)]
    pub last_activity: i64,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QBittorrentSync {
    #[serde(default)]
    pub rid: i64,
    #[serde(default)]
    pub full_update: bool,
    #[serde(default)]
    pub torrents: HashMap<String, QBittorrentTorrent>,
    #[serde(default)]
    pub torrents_removed: Vec<String>,
    #[serde(default)]
    pub server_state: Value,
    #[serde(default)]
    pub categories: Value,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QBittorrentTransferInfo {
    #[serde(default)]
    pub dl_info_speed: i64,
    #[serde(default)]
    pub up_info_speed: i64,
    #[serde(default)]
    pub dl_info_data: i64,
    #[serde(default)]
    pub up_info_data: i64,
    #[serde(default)]
    pub connection_status: String,
    #[serde(default)]
    pub free_space_on_disk: i64,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QBittorrentAppInfo {
    pub version: String,
    pub webapi_version: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct AddTorrentOptions {
    pub urls: Option<String>,
    pub category: Option<String>,
    pub savepath: Option<String>,
    pub tags: Option<String>,
    pub paused: Option<bool>,
    pub stopped: Option<bool>,
    pub skip_checking: Option<bool>,
    #[serde(default, alias = "sequentialDownload")]
    pub sequential_download: Option<bool>,
    #[serde(default, alias = "firstLastPiecePrio")]
    pub first_last_piece_prio: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CreateTorrentParams {
    pub source_path: String,
    pub save_path: Option<String>,
    pub torrent_name: Option<String>,
    pub trackers: Option<String>,
    pub source: Option<String>,
    pub private: bool,
    pub start_seeding: bool,
}

#[derive(Debug, Clone)]
pub struct TorrentUpload {
    pub name: String,
    pub bytes: Vec<u8>,
}

impl TorrentUpload {
    pub fn validate(self, max_bytes: usize) -> Result<Self> {
        if self.bytes.is_empty() {
            return Err(anyhow!("Torrent file is empty"));
        }
        if self.bytes.len() > max_bytes {
            return Err(anyhow!(
                "Torrent file exceeds {} MiB",
                max_bytes / 1024 / 1024
            ));
        }
        if torrent_info_hash(&self.bytes).is_none() {
            return Err(anyhow!("Invalid .torrent file"));
        }
        Ok(self)
    }
}

pub const MAX_TORRENT_UPLOAD_BYTES: usize = 10 * 1024 * 1024;

pub fn sanitize_torrent_filename(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "flasharr.torrent".to_string();
    }
    let sanitized: String = trimmed
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | '\0' => '_',
            _ => ch,
        })
        .collect();
    if sanitized.is_empty() {
        "flasharr.torrent".to_string()
    } else if sanitized.to_ascii_lowercase().ends_with(".torrent") {
        sanitized
    } else {
        format!("{}.torrent", sanitized)
    }
}

pub fn bool_form_fields(options: &AddTorrentOptions) -> Vec<(&'static str, String)> {
    let mut fields = Vec::new();
    if let Some(paused) = options.paused {
        fields.push(("paused", paused.to_string()));
    }
    if let Some(stopped) = options.stopped {
        fields.push(("stopped", stopped.to_string()));
    }
    if let Some(skip) = options.skip_checking {
        fields.push(("skip_checking", skip.to_string()));
    }
    if let Some(seq) = options.sequential_download {
        fields.push(("sequentialDownload", seq.to_string()));
    }
    if let Some(first_last) = options.first_last_piece_prio {
        fields.push(("firstLastPiecePrio", first_last.to_string()));
    }
    fields
}

fn multipart_form_from_options(
    file_part: multipart::Part,
    options: &AddTorrentOptions,
) -> multipart::Form {
    let mut form = multipart::Form::new().part("torrents", file_part);
    if let Some(category) = options.category.as_ref() {
        if !category.trim().is_empty() {
            form = form.text("category", category.clone());
        }
    }
    if let Some(save_path) = options.savepath.as_ref() {
        if !save_path.trim().is_empty() {
            form = form.text("savepath", save_path.clone());
        }
    }
    if let Some(tags) = &options.tags {
        if !tags.trim().is_empty() {
            form = form.text("tags", tags.clone());
        }
    }
    for (key, value) in bool_form_fields(options) {
        form = form.text(key, value);
    }
    form
}

impl QBittorrentClient {
    pub fn new(config: QBittorrentConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()?;
        Ok(Self {
            config,
            client,
            sid: Arc::new(Mutex::new(None)),
        })
    }

    pub fn config(&self) -> &QBittorrentConfig {
        &self.config
    }

    pub async fn test_connection(&self) -> Result<String> {
        let version = self.app_version().await?;
        Ok(format!(
            "qBittorrent connected{}",
            if version.is_empty() {
                String::new()
            } else {
                format!(" ({})", version)
            }
        ))
    }

    pub async fn app_info(&self) -> Result<QBittorrentAppInfo> {
        Ok(QBittorrentAppInfo {
            version: self.app_version().await.unwrap_or_default(),
            webapi_version: self.webapi_version().await.unwrap_or_default(),
        })
    }

    pub async fn app_version(&self) -> Result<String> {
        let resp = self.get_authed("/api/v2/app/version").await?;
        ok_text(resp, "qBittorrent version").await
    }

    pub async fn webapi_version(&self) -> Result<String> {
        let resp = self.get_authed("/api/v2/app/webapiVersion").await?;
        ok_text(resp, "qBittorrent Web API version").await
    }

    pub async fn sync_maindata(&self, rid: i64) -> Result<QBittorrentSync> {
        let cookie = self.cookie().await?;
        let resp = self
            .client
            .get(self.endpoint("/api/v2/sync/maindata")?)
            .header(COOKIE, cookie_header_value(&cookie)?)
            .query(&[("rid", rid)])
            .send()
            .await?;
        let resp = self
            .retry_if_unauthorized(resp, &cookie, |cookie| async move {
                self.client
                    .get(self.endpoint("/api/v2/sync/maindata")?)
                    .header(COOKIE, cookie_header_value(&cookie)?)
                    .query(&[("rid", rid)])
                    .send()
                    .await
                    .map_err(Into::into)
            })
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("qBittorrent sync failed: HTTP {}", resp.status()));
        }
        let mut sync = resp.json::<QBittorrentSync>().await?;
        for (hash, torrent) in sync.torrents.iter_mut() {
            if torrent.hash.is_empty() {
                torrent.hash = hash.to_ascii_lowercase();
            }
        }
        Ok(sync)
    }

    pub async fn transfer_info(&self) -> Result<QBittorrentTransferInfo> {
        let resp = self.get_authed("/api/v2/transfer/info").await?;
        ok_json(resp, "qBittorrent transfer info").await
    }

    pub async fn categories(&self) -> Result<Value> {
        let resp = self.get_authed("/api/v2/torrents/categories").await?;
        ok_json(resp, "qBittorrent categories").await
    }

    pub async fn tags(&self) -> Result<Vec<String>> {
        let resp = self.get_authed("/api/v2/torrents/tags").await?;
        ok_json(resp, "qBittorrent tags").await
    }

    pub async fn preferences(&self) -> Result<Value> {
        let resp = self.get_authed("/api/v2/app/preferences").await?;
        ok_json(resp, "qBittorrent preferences").await
    }

    pub async fn set_preferences(&self, prefs: Value) -> Result<()> {
        self.post_form(
            "/api/v2/app/setPreferences",
            vec![("json", prefs.to_string())],
        )
        .await
    }

    pub async fn torrent_value(&self, hash: &str, path: &str) -> Result<Value> {
        let cookie = self.cookie().await?;
        let resp = self
            .client
            .get(self.endpoint(path)?)
            .header(COOKIE, cookie_header_value(&cookie)?)
            .query(&[("hash", hash)])
            .send()
            .await?;
        let path_string = path.to_string();
        let hash_string = hash.to_string();
        let resp = self
            .retry_if_unauthorized(resp, &cookie, |cookie| async move {
                self.client
                    .get(self.endpoint(&path_string)?)
                    .header(COOKIE, cookie_header_value(&cookie)?)
                    .query(&[("hash", hash_string.as_str())])
                    .send()
                    .await
                    .map_err(Into::into)
            })
            .await?;
        ok_json(resp, "qBittorrent torrent detail").await
    }

    pub async fn add_torrent(&self, url_or_magnet: &str) -> Result<Option<String>> {
        self.add_torrent_with_options(url_or_magnet, AddTorrentOptions::default())
            .await
    }

    pub async fn add_torrent_with_options(
        &self,
        url_or_magnet: &str,
        options: AddTorrentOptions,
    ) -> Result<Option<String>> {
        // qBittorrent fetches remote URLs itself. Flasharr never dereferences user-provided URLs.
        self.add_torrent_url(url_or_magnet, &options).await?;
        Ok(extract_btih(url_or_magnet))
    }

    pub async fn add_torrent_upload(
        &self,
        upload: TorrentUpload,
        options: AddTorrentOptions,
    ) -> Result<Option<String>> {
        let TorrentUpload { name, bytes } = upload.validate(MAX_TORRENT_UPLOAD_BYTES)?;
        let hash = torrent_info_hash(&bytes);
        self.add_torrent_bytes(&name, bytes, &options).await?;
        Ok(hash)
    }

    async fn add_torrent_url(
        &self,
        url_or_magnet: &str,
        options: &AddTorrentOptions,
    ) -> Result<()> {
        let mut form = vec![("urls", url_or_magnet.to_string())];
        self.push_optional_form_fields(&mut form, options);
        self.post_form("/api/v2/torrents/add", form).await
    }

    async fn fetch_torrent_bytes(&self, torrent_url: &str) -> Result<Vec<u8>> {
        let torrent_resp = self.client.get(torrent_url).send().await.map_err(|e| {
            anyhow!(
                "qBittorrent torrent fetch failed: {}",
                reqwest_error_kind(&e)
            )
        })?;
        if !torrent_resp.status().is_success() {
            return Err(anyhow!(
                "qBittorrent torrent fetch failed: HTTP {}",
                torrent_resp.status()
            ));
        }
        let torrent_bytes = torrent_resp
            .bytes()
            .await
            .map_err(|e| {
                anyhow!(
                    "qBittorrent torrent fetch body failed: {}",
                    reqwest_error_kind(&e)
                )
            })?
            .to_vec();
        if torrent_bytes.is_empty() {
            return Err(anyhow!("qBittorrent torrent fetch returned empty body"));
        }
        Ok(torrent_bytes)
    }

    async fn add_torrent_bytes(
        &self,
        file_name: &str,
        torrent_bytes: Vec<u8>,
        options: &AddTorrentOptions,
    ) -> Result<()> {
        let cookie = self.cookie().await?;
        let file_part =
            multipart::Part::bytes(torrent_bytes).file_name(sanitize_torrent_filename(file_name));
        let mut merged = options.clone();
        if merged.category.is_none() {
            merged.category = self.config.category.clone();
        }
        if merged.savepath.is_none() {
            merged.savepath = self.config.save_path.clone();
        }
        let form = multipart_form_from_options(file_part, &merged);

        let resp = self
            .client
            .post(self.endpoint("/api/v2/torrents/add")?)
            .header(COOKIE, cookie_header_value(&cookie)?)
            .multipart(form)
            .send()
            .await?;
        let resp = self
            .retry_if_unauthorized(resp, &cookie, |_cookie| async move {
                Err(anyhow!(
                    "qBittorrent upload session expired; retry the upload"
                ))
            })
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!("qBittorrent add failed: HTTP {}", resp.status()));
        }
        Ok(())
    }

    fn push_optional_form_fields(
        &self,
        form: &mut Vec<(&'static str, String)>,
        options: &AddTorrentOptions,
    ) {
        if let Some(category) = options.category.as_ref().or(self.config.category.as_ref()) {
            if !category.trim().is_empty() {
                form.push(("category", category.clone()));
            }
        }
        if let Some(save_path) = options.savepath.as_ref().or(self.config.save_path.as_ref()) {
            if !save_path.trim().is_empty() {
                form.push(("savepath", save_path.clone()));
            }
        }
        if let Some(tags) = &options.tags {
            if !tags.trim().is_empty() {
                form.push(("tags", tags.clone()));
            }
        }
        form.extend(bool_form_fields(options));
    }

    pub async fn get_torrent(&self, hash: &str) -> Result<Option<QBittorrentInfo>> {
        let torrents = self.list_torrents_filtered(Some(hash)).await?;
        Ok(torrents
            .into_iter()
            .find(|torrent| torrent.hash.eq_ignore_ascii_case(hash)))
    }

    pub async fn pause(&self, hash: &str) -> Result<()> {
        match self.command_hashes("/api/v2/torrents/pause", hash).await {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().contains("HTTP 404") => {
                self.command_hashes("/api/v2/torrents/stop", hash).await
            }
            Err(e) => Err(e),
        }
    }

    pub async fn resume(&self, hash: &str) -> Result<()> {
        match self.command_hashes("/api/v2/torrents/resume", hash).await {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().contains("HTTP 404") => {
                self.command_hashes("/api/v2/torrents/start", hash).await
            }
            Err(e) => Err(e),
        }
    }

    pub async fn delete(&self, hash: &str, delete_files: bool) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/delete",
            vec![
                ("hashes", hash.to_string()),
                (
                    "deleteFiles",
                    if delete_files { "true" } else { "false" }.to_string(),
                ),
            ],
        )
        .await
    }

    pub async fn recheck(&self, hash: &str) -> Result<()> {
        self.command_hashes("/api/v2/torrents/recheck", hash).await
    }

    pub async fn reannounce(&self, hash: &str) -> Result<()> {
        self.command_hashes("/api/v2/torrents/reannounce", hash)
            .await
    }

    pub async fn set_category(&self, hash: &str, category: &str) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/setCategory",
            vec![
                ("hashes", hash.to_string()),
                ("category", category.to_string()),
            ],
        )
        .await
    }

    pub async fn set_tags(&self, hash: &str, tags: &str) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/setTags",
            vec![("hashes", hash.to_string()), ("tags", tags.to_string())],
        )
        .await
    }

    pub async fn set_file_priority(&self, hash: &str, file_id: u32, priority: i32) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/filePrio",
            vec![
                ("hash", hash.to_string()),
                ("id", file_id.to_string()),
                ("priority", priority.to_string()),
            ],
        )
        .await
    }

    pub async fn set_download_limit(&self, hash: &str, limit: i64) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/setDownloadLimit",
            vec![("hashes", hash.to_string()), ("limit", limit.to_string())],
        )
        .await
    }

    pub async fn set_upload_limit(&self, hash: &str, limit: i64) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/setUploadLimit",
            vec![("hashes", hash.to_string()), ("limit", limit.to_string())],
        )
        .await
    }

    pub async fn set_share_limits(
        &self,
        hash: &str,
        ratio_limit: f64,
        seeding_time_limit: i64,
    ) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/setShareLimits",
            vec![
                ("hashes", hash.to_string()),
                ("ratioLimit", ratio_limit.to_string()),
                ("seedingTimeLimit", seeding_time_limit.to_string()),
            ],
        )
        .await
    }

    pub async fn top_priority(&self, hash: &str) -> Result<()> {
        self.command_hashes("/api/v2/torrents/topPrio", hash).await
    }

    pub async fn increase_priority(&self, hash: &str) -> Result<()> {
        self.command_hashes("/api/v2/torrents/increasePrio", hash)
            .await
    }

    pub async fn decrease_priority(&self, hash: &str) -> Result<()> {
        self.command_hashes("/api/v2/torrents/decreasePrio", hash)
            .await
    }

    pub async fn bottom_priority(&self, hash: &str) -> Result<()> {
        self.command_hashes("/api/v2/torrents/bottomPrio", hash)
            .await
    }

    pub async fn rename_torrent(&self, hash: &str, name: &str) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/rename",
            vec![("hash", hash.to_string()), ("name", name.to_string())],
        )
        .await
    }

    pub async fn rename_file(&self, hash: &str, old_path: &str, new_path: &str) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/renameFile",
            vec![
                ("hash", hash.to_string()),
                ("oldPath", old_path.to_string()),
                ("newPath", new_path.to_string()),
            ],
        )
        .await
    }

    pub async fn rename_folder(&self, hash: &str, old_path: &str, new_path: &str) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/renameFolder",
            vec![
                ("hash", hash.to_string()),
                ("oldPath", old_path.to_string()),
                ("newPath", new_path.to_string()),
            ],
        )
        .await
    }

    pub async fn set_location(&self, hash: &str, location: &str) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/setLocation",
            vec![
                ("hashes", hash.to_string()),
                ("location", location.to_string()),
            ],
        )
        .await
    }

    pub async fn add_peers(&self, hash: &str, peers: &str) -> Result<()> {
        let peers = normalize_peer_list(peers).ok_or_else(|| anyhow!("Peers must be host:port"))?;
        self.post_form(
            "/api/v2/torrents/addPeers",
            vec![("hashes", hash.to_string()), ("peers", peers)],
        )
        .await
    }

    pub async fn ban_peers(&self, peers: &str) -> Result<()> {
        let peers = normalize_peer_list(peers).ok_or_else(|| anyhow!("Peers must be host:port"))?;
        self.post_form("/api/v2/transfer/banPeers", vec![("peers", peers)])
            .await
    }

    pub async fn add_tags(&self, hash: &str, tags: &str) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/addTags",
            vec![("hashes", hash.to_string()), ("tags", tags.to_string())],
        )
        .await
    }

    pub async fn remove_tags(&self, hash: &str, tags: &str) -> Result<()> {
        self.post_form(
            "/api/v2/torrents/removeTags",
            vec![("hashes", hash.to_string()), ("tags", tags.to_string())],
        )
        .await
    }

    async fn command_hashes(&self, path: &str, hash: &str) -> Result<()> {
        self.post_form(path, vec![("hashes", hash.to_string())])
            .await
    }

    async fn post_form(&self, path: &str, form: Vec<(&'static str, String)>) -> Result<()> {
        let cookie = self.cookie().await?;
        let resp = self
            .client
            .post(self.endpoint(path)?)
            .header(COOKIE, cookie_header_value(&cookie)?)
            .form(&form)
            .send()
            .await?;
        let path_string = path.to_string();
        let form_clone = form.clone();
        let resp = self
            .retry_if_unauthorized(resp, &cookie, |cookie| async move {
                self.client
                    .post(self.endpoint(&path_string)?)
                    .header(COOKIE, cookie_header_value(&cookie)?)
                    .form(&form_clone)
                    .send()
                    .await
                    .map_err(Into::into)
            })
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "qBittorrent command failed: HTTP {}",
                resp.status()
            ));
        }
        Ok(())
    }

    async fn find_hash_by_source(&self, source: &str) -> Result<Option<String>> {
        let torrents = self.list_torrents().await?;
        if let Some(hash) = extract_btih(source) {
            return Ok(torrents
                .into_iter()
                .find(|t| t.hash.eq_ignore_ascii_case(&hash))
                .map(|t| t.hash));
        }
        Ok(None)
    }

    async fn find_new_hash(&self, before: HashSet<String>) -> Result<Option<String>> {
        Ok(self
            .list_torrents()
            .await?
            .into_iter()
            .find(|t| !before.contains(&t.hash.to_ascii_lowercase()))
            .map(|t| t.hash))
    }

    async fn list_hashes(&self) -> Result<HashSet<String>> {
        Ok(self
            .list_torrents()
            .await?
            .into_iter()
            .map(|t| t.hash.to_ascii_lowercase())
            .collect())
    }

    async fn list_torrents(&self) -> Result<Vec<QBittorrentInfo>> {
        self.list_torrents_filtered(None).await
    }

    async fn list_torrents_filtered(&self, hashes: Option<&str>) -> Result<Vec<QBittorrentInfo>> {
        let cookie = self.cookie().await?;
        let mut req = self
            .client
            .get(self.endpoint("/api/v2/torrents/info")?)
            .header(COOKIE, cookie_header_value(&cookie)?);
        if let Some(hashes) = hashes {
            req = req.query(&[("hashes", hashes)]);
        }
        let resp = req.send().await?;
        let hash_string = hashes.map(str::to_string);
        let resp = self
            .retry_if_unauthorized(resp, &cookie, |cookie| async move {
                let mut req = self
                    .client
                    .get(self.endpoint("/api/v2/torrents/info")?)
                    .header(COOKIE, cookie_header_value(&cookie)?);
                if let Some(hashes) = &hash_string {
                    req = req.query(&[("hashes", hashes.as_str())]);
                }
                req.send().await.map_err(Into::into)
            })
            .await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }
        Ok(resp
            .json::<Vec<QBittorrentInfo>>()
            .await
            .unwrap_or_default())
    }

    async fn get_authed(&self, path: &str) -> Result<reqwest::Response> {
        let cookie = self.cookie().await?;
        let resp = self
            .client
            .get(self.endpoint(path)?)
            .header(COOKIE, cookie_header_value(&cookie)?)
            .send()
            .await?;
        let path_string = path.to_string();
        self.retry_if_unauthorized(resp, &cookie, |cookie| async move {
            self.client
                .get(self.endpoint(&path_string)?)
                .header(COOKIE, cookie_header_value(&cookie)?)
                .send()
                .await
                .map_err(Into::into)
        })
        .await
    }

    async fn retry_if_unauthorized<F, Fut>(
        &self,
        resp: reqwest::Response,
        expected_cookie: &str,
        retry: F,
    ) -> Result<reqwest::Response>
    where
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<reqwest::Response>>,
    {
        if matches!(resp.status().as_u16(), 401 | 403) {
            self.clear_cookie(expected_cookie).await;
            let cookie = self.cookie().await?;
            retry(cookie).await
        } else {
            Ok(resp)
        }
    }

    async fn cookie(&self) -> Result<String> {
        let mut sid = self.sid.lock().await;
        if let Some(cookie) = sid.clone() {
            return Ok(cookie);
        }
        let cookie = self.login().await?;
        *sid = Some(cookie.clone());
        Ok(cookie)
    }

    async fn clear_cookie(&self, expected_cookie: &str) {
        let mut sid = self.sid.lock().await;
        if sid.as_deref() == Some(expected_cookie) {
            *sid = None;
        }
    }

    async fn login(&self) -> Result<String> {
        if !self.config.enabled {
            return Err(anyhow!("qBittorrent is disabled"));
        }
        let resp = self
            .client
            .post(self.endpoint("/api/v2/auth/login")?)
            .form(&[
                ("username", self.config.username.as_str()),
                ("password", self.config.password.as_str()),
            ])
            .send()
            .await
            .map_err(|e| anyhow!("qBittorrent login failed: {}", reqwest_error_kind(&e)))?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let body = resp.text().await.unwrap_or_default();
        let cookie = validate_login_response(status, &headers, &body)?;
        Ok(cookie)
    }

    fn endpoint(&self, path: &str) -> Result<String> {
        let base = self.config.url.trim().trim_end_matches('/');
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err(anyhow!(
                "qBittorrent URL must start with http:// or https://"
            ));
        }
        Ok(format!("{}{}", base, path))
    }

    pub async fn create_torrent(&self, params: &CreateTorrentParams) -> Result<Vec<u8>> {
        if !valid_create_torrent_path(&params.source_path) {
            return Err(anyhow!("Source path is required"));
        }
        let mut form: Vec<(&'static str, String)> = vec![
            ("sourcePath", params.source_path.trim().to_string()),
            ("isPrivate", params.private.to_string()),
            ("startSeeding", params.start_seeding.to_string()),
        ];
        if let Some(name) = &params.torrent_name {
            if !name.trim().is_empty() {
                form.push(("torrentName", name.trim().to_string()));
            }
        }
        if let Some(save_path) = &params.save_path {
            if !save_path.trim().is_empty() {
                form.push(("savePath", save_path.trim().to_string()));
            }
        }
        if let Some(trackers) = &params.trackers {
            if !trackers.trim().is_empty() {
                form.push(("trackers", trackers.clone()));
            }
        }
        if let Some(source) = &params.source {
            if !source.trim().is_empty() {
                form.push(("torrentSource", source.trim().to_string()));
            }
        }
        self.post_form_bytes("/api/v2/torrents/createTorrent", form)
            .await
    }

    async fn post_form_bytes(
        &self,
        path: &str,
        form: Vec<(&'static str, String)>,
    ) -> Result<Vec<u8>> {
        let cookie = self.cookie().await?;
        let resp = self
            .client
            .post(self.endpoint(path)?)
            .header(COOKIE, cookie_header_value(&cookie)?)
            .form(&form)
            .send()
            .await?;
        let path_string = path.to_string();
        let form_clone = form.clone();
        let resp = self
            .retry_if_unauthorized(resp, &cookie, |cookie| async move {
                self.client
                    .post(self.endpoint(&path_string)?)
                    .header(COOKIE, cookie_header_value(&cookie)?)
                    .form(&form_clone)
                    .send()
                    .await
                    .map_err(Into::into)
            })
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow!(
                "qBittorrent command failed: HTTP {}",
                resp.status()
            ));
        }
        Ok(resp.bytes().await?.to_vec())
    }
}

async fn ok_text(resp: reqwest::Response, label: &str) -> Result<String> {
    if !resp.status().is_success() {
        return Err(anyhow!("{} failed: HTTP {}", label, resp.status()));
    }
    Ok(resp.text().await.unwrap_or_default())
}

async fn ok_json<T: for<'de> Deserialize<'de>>(resp: reqwest::Response, label: &str) -> Result<T> {
    if !resp.status().is_success() {
        return Err(anyhow!("{} failed: HTTP {}", label, resp.status()));
    }
    Ok(resp.json::<T>().await?)
}

fn extract_sid(headers: &HeaderMap) -> Option<String> {
    for value in headers.get_all(SET_COOKIE) {
        if let Ok(raw) = value.to_str() {
            if let Some(part) = raw.split(';').next() {
                let cookie = part.trim();
                if let Some((name, _)) = cookie.split_once('=') {
                    if name == "SID" || name.starts_with("QBT_SID") {
                        return Some(cookie.to_string());
                    }
                }
            }
        }
    }
    None
}

fn validate_login_response(
    status: reqwest::StatusCode,
    headers: &HeaderMap,
    body: &str,
) -> Result<String> {
    if !status.is_success() {
        return Err(anyhow!("qBittorrent login failed: HTTP {}", status));
    }
    if status != reqwest::StatusCode::NO_CONTENT
        && !body.trim().trim_end_matches('.').eq_ignore_ascii_case("ok")
    {
        if body.to_ascii_lowercase().contains("banned") {
            return Err(anyhow!("qBittorrent login rejected: IP banned"));
        }
        return Err(anyhow!("qBittorrent login rejected: bad credentials"));
    }
    extract_sid(headers).ok_or_else(|| anyhow!("qBittorrent login returned no SID cookie"))
}

fn cookie_header_value(cookie: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(cookie).map_err(|e| anyhow!("invalid qBittorrent cookie: {}", e))
}

fn reqwest_error_kind(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() {
        "timeout"
    } else if e.is_connect() {
        "connect"
    } else if e.is_decode() {
        "decode"
    } else {
        "request"
    }
}

fn extract_btih(value: &str) -> Option<String> {
    let marker = "btih:";
    let start = value.find(marker)? + marker.len();
    let tail = &value[start..];
    let end = tail.find('&').unwrap_or(tail.len());
    Some(tail[..end].to_ascii_lowercase())
}

pub fn should_upload_torrent(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    !lower.starts_with("magnet:")
        && (lower.contains("apikey=")
            || lower.contains("/download?")
            || lower.contains("download.php?id=")
            || lower.contains(".torrent"))
}

pub fn valid_create_torrent_path(path: &str) -> bool {
    let trimmed = path.trim();
    !trimmed.is_empty() && !trimmed.contains('\0')
}

pub fn valid_file_priority(priority: i32) -> bool {
    matches!(priority, 0 | 1 | 6 | 7)
}

pub fn valid_share_ratio_limit(limit: f64) -> bool {
    limit == -1.0 || limit >= 0.0
}

pub fn valid_seeding_time_limit(limit: i64) -> bool {
    limit == -1 || limit >= 0
}

pub fn valid_torrent_hash(hash: &str) -> bool {
    hash.len() == 40 && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn valid_non_empty_hashes(hashes: &[String]) -> bool {
    !hashes.is_empty() && hashes.iter().all(|hash| valid_torrent_hash(hash.trim()))
}

pub fn valid_cross_seed_targets(src: &str, dst: &str) -> bool {
    let (src, dst) = (src.trim(), dst.trim());
    !src.is_empty() && !dst.is_empty() && src != dst
}

pub fn valid_torrent_rename(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && !trimmed.chars().any(|ch| matches!(ch, '/' | '\\' | '\0'))
}

pub fn valid_location(location: &str) -> bool {
    let trimmed = location.trim();
    !trimmed.is_empty() && !trimmed.contains('\0')
}

pub fn normalize_peer_list(peers: &str) -> Option<String> {
    let items = peers
        .split(|ch| matches!(ch, '|' | ',' | '\n' | '\r'))
        .map(str::trim)
        .filter(|peer| !peer.is_empty())
        .collect::<Vec<_>>();
    if items.is_empty()
        || items.iter().any(|peer| {
            peer.contains('\0')
                || !peer
                    .rsplit_once(':')
                    .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
        })
    {
        None
    } else {
        Some(items.join("|"))
    }
}

pub fn valid_path_leaf(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty() && !trimmed.chars().any(|ch| matches!(ch, '/' | '\\' | '\0'))
}

pub fn rename_path_with_leaf(old_path: &str, new_leaf: &str) -> Option<String> {
    if !valid_path_leaf(new_leaf) {
        return None;
    }
    let old_path = old_path.trim();
    if old_path.is_empty() {
        return None;
    }
    let Some(separator) = old_path.rfind(|ch| matches!(ch, '/' | '\\')) else {
        return Some(new_leaf.trim().to_string());
    };
    let prefix = &old_path[..=separator];
    Some(format!("{}{}", prefix, new_leaf.trim()))
}

pub fn qbit_state_to_flasharr(state: &str, progress: f64) -> crate::downloader::DownloadState {
    crate::torrent::state::classify_qbit(
        state,
        crate::torrent::state::TransferMetrics {
            downloaded: 0,
            previous_downloaded: None,
            progress,
            dlspeed: 0,
            upspeed: 0,
            amount_left: if progress >= 1.0 { 0 } else { 1 },
        },
        "healthy",
    )
    .task_state
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        extract::State,
        http::{header, HeaderMap as AxumHeaderMap, StatusCode},
        response::IntoResponse,
        routing::{get, post},
        Router,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn login_headers(with_sid: bool) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if with_sid {
            headers.insert(
                SET_COOKIE,
                HeaderValue::from_static("SID=test-session; HttpOnly"),
            );
        }
        headers
    }

    #[tokio::test]
    async fn concurrent_requests_share_one_login() {
        let logins = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route(
                "/api/v2/auth/login",
                post(|State(logins): State<Arc<AtomicUsize>>| async move {
                    logins.fetch_add(1, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    (
                        StatusCode::NO_CONTENT,
                        [(header::SET_COOKIE, "QBT_SID_8080=test-session; HttpOnly")],
                    )
                }),
            )
            .route(
                "/api/v2/app/version",
                get(|headers: AxumHeaderMap| async move {
                    if headers.get(header::COOKIE).is_some() {
                        "5.0.0".into_response()
                    } else {
                        StatusCode::FORBIDDEN.into_response()
                    }
                }),
            )
            .route(
                "/api/v2/sync/maindata",
                get(|headers: AxumHeaderMap| async move {
                    if headers.get(header::COOKIE).is_some() {
                        axum::Json(serde_json::json!({"rid": 1, "full_update": true}))
                            .into_response()
                    } else {
                        StatusCode::FORBIDDEN.into_response()
                    }
                }),
            )
            .with_state(logins.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = QBittorrentClient::new(QBittorrentConfig {
            enabled: true,
            url,
            username: "test".into(),
            password: "test".into(),
            category: None,
            save_path: None,
            poll_interval_secs: 5,
            instance_id: None,
            instance_name: None,
        })
        .unwrap();

        let (version, sync) = tokio::join!(client.app_version(), client.sync_maindata(0));
        assert_eq!(version.unwrap(), "5.0.0");
        assert_eq!(sync.unwrap().rid, 1);
        assert_eq!(logins.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn accepts_qbit_v5_no_content_login_with_sid() {
        assert_eq!(
            validate_login_response(reqwest::StatusCode::NO_CONTENT, &login_headers(true), "")
                .unwrap(),
            "SID=test-session"
        );
    }

    #[test]
    fn rejects_qbit_v5_no_content_login_without_sid() {
        assert!(validate_login_response(
            reqwest::StatusCode::NO_CONTENT,
            &login_headers(false),
            ""
        )
        .is_err());
    }

    #[test]
    fn accepts_legacy_ok_login_with_sid() {
        assert!(
            validate_login_response(reqwest::StatusCode::OK, &login_headers(true), " Ok. \n")
                .is_ok()
        );
    }

    #[test]
    fn extracts_qbit_v5_sid_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_static("QBT_SID_8080=abc; HttpOnly; path=/"),
        );
        assert_eq!(extract_sid(&headers).as_deref(), Some("QBT_SID_8080=abc"));
    }

    #[test]
    fn extracts_legacy_sid_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(SET_COOKIE, HeaderValue::from_static("SID=xyz; path=/"));
        assert_eq!(extract_sid(&headers).as_deref(), Some("SID=xyz"));
    }

    #[test]
    fn extracts_auth_cookie_from_multiple_set_cookie_headers() {
        let mut headers = HeaderMap::new();
        headers.append(SET_COOKIE, HeaderValue::from_static("theme=dark; path=/"));
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("QBT_SID_9090=synthetic; HttpOnly"),
        );
        assert_eq!(
            extract_sid(&headers).as_deref(),
            Some("QBT_SID_9090=synthetic")
        );
    }

    #[test]
    fn ignores_set_cookie_without_auth_cookie() {
        let mut headers = HeaderMap::new();
        headers.append(SET_COOKIE, HeaderValue::from_static("theme=dark; path=/"));
        headers.append(SET_COOKIE, HeaderValue::from_static("other=value"));
        assert_eq!(extract_sid(&headers), None);
    }

    #[test]
    fn rejects_success_with_unexpected_body() {
        let error = validate_login_response(
            reqwest::StatusCode::OK,
            &login_headers(true),
            "unexpected-sensitive-value",
        )
        .unwrap_err()
        .to_string();
        assert_eq!(error, "qBittorrent login rejected: bad credentials");
        assert!(!error.contains("unexpected-sensitive-value"));
    }

    #[test]
    fn preserves_legacy_banned_login_detection() {
        assert_eq!(
            validate_login_response(
                reqwest::StatusCode::OK,
                &login_headers(true),
                "Fails. Your IP is banned."
            )
            .unwrap_err()
            .to_string(),
            "qBittorrent login rejected: IP banned"
        );
    }

    #[test]
    fn rejects_non_success_login_as_http_error() {
        assert_eq!(
            validate_login_response(
                reqwest::StatusCode::UNAUTHORIZED,
                &login_headers(true),
                "Ok."
            )
            .unwrap_err()
            .to_string(),
            "qBittorrent login failed: HTTP 401 Unauthorized"
        );
    }

    #[test]
    fn maps_qbit_states() {
        assert_eq!(
            qbit_state_to_flasharr("pausedDL", 0.2),
            crate::downloader::DownloadState::Paused
        );
        assert_eq!(
            qbit_state_to_flasharr("downloading", 0.2),
            crate::downloader::DownloadState::Downloading
        );
        assert_eq!(
            qbit_state_to_flasharr("uploading", 1.0),
            crate::downloader::DownloadState::Completed
        );
        assert_eq!(
            qbit_state_to_flasharr("error", 0.2),
            crate::downloader::DownloadState::Failed
        );
        assert_eq!(
            qbit_state_to_flasharr("", 0.0),
            crate::downloader::DownloadState::Starting,
            "blank mirror placeholders are not active torrents"
        );
    }

    #[test]
    fn extracts_btih() {
        assert_eq!(
            extract_btih("magnet:?xt=urn:btih:ABC123&dn=x"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn detects_secret_torrent_urls_for_upload() {
        assert!(should_upload_torrent(
            "http://prowlarr/1/download?apikey=secret&link=x"
        ));
        assert!(should_upload_torrent(
            "https://example.test/download.php?id=204885"
        ));
        assert!(should_upload_torrent("https://example.test/file.torrent"));
        assert!(!should_upload_torrent("magnet:?xt=urn:btih:ABC123"));
    }

    #[test]
    fn validates_create_torrent_params() {
        assert!(valid_create_torrent_path("/downloads/show"));
        assert!(!valid_create_torrent_path(""));
        assert!(!valid_create_torrent_path("bad\0path"));
    }

    #[test]
    fn file_priority_values_match_ui_contract() {
        for value in [0, 1, 6, 7] {
            assert!(super::valid_file_priority(value));
        }
        for value in [-1, 2, 3, 4, 5, 8] {
            assert!(!super::valid_file_priority(value));
        }
    }

    #[test]
    fn share_limit_values_match_ui_contract() {
        assert!(super::valid_share_ratio_limit(-1.0));
        assert!(super::valid_share_ratio_limit(0.0));
        assert!(super::valid_share_ratio_limit(2.5));
        assert!(!super::valid_share_ratio_limit(-2.0));
        assert!(super::valid_seeding_time_limit(-1));
        assert!(super::valid_seeding_time_limit(0));
        assert!(super::valid_seeding_time_limit(60));
        assert!(!super::valid_seeding_time_limit(-2));
    }

    #[test]
    fn sanitizes_uploaded_torrent_names() {
        assert_eq!(
            sanitize_torrent_filename(" Show/Season 1 "),
            "Show_Season 1.torrent"
        );
        assert_eq!(sanitize_torrent_filename("file.torrent"), "file.torrent");
        assert_eq!(sanitize_torrent_filename(""), "flasharr.torrent");
    }

    #[test]
    fn validates_torrent_upload_bytes() {
        let valid = TorrentUpload {
            name: "x.torrent".to_string(),
            bytes: b"d4:infod6:lengthi123e4:name4:testee".to_vec(),
        };
        assert!(valid.validate(MAX_TORRENT_UPLOAD_BYTES).is_ok());
        assert!(TorrentUpload {
            name: "x".to_string(),
            bytes: Vec::new()
        }
        .validate(MAX_TORRENT_UPLOAD_BYTES)
        .is_err());
    }

    #[test]
    fn validates_bulk_hashes_and_locations() {
        assert!(valid_non_empty_hashes(&[
            "0123456789abcdef0123456789abcdef01234567".to_string()
        ]));
        assert!(!valid_non_empty_hashes(&[]));
        assert!(!valid_non_empty_hashes(&["all".to_string()]));
        assert!(!valid_non_empty_hashes(&[" ".to_string()]));
        assert!(valid_location("/downloads/tv"));
        assert!(!valid_location("   "));
        assert!(!valid_location("bad\0path"));
    }

    #[test]
    fn normalizes_peer_lists() {
        assert_eq!(
            normalize_peer_list("1.2.3.4:6881\n5.6.7.8:51413"),
            Some("1.2.3.4:6881|5.6.7.8:51413".to_string())
        );
        assert_eq!(
            normalize_peer_list("[2001:db8::1]:6881"),
            Some("[2001:db8::1]:6881".to_string())
        );
        assert_eq!(normalize_peer_list("1.2.3.4"), None);
        assert_eq!(normalize_peer_list("1.2.3.4:99999"), None);
    }

    #[test]
    fn validates_rename_inputs() {
        assert!(valid_torrent_rename("New Name"));
        assert!(!valid_torrent_rename(""));
        assert!(!valid_torrent_rename("bad/name"));
        assert!(valid_path_leaf("Episode 01.mkv"));
        assert!(!valid_path_leaf("Season/01"));
    }

    #[test]
    fn rebuilds_paths_with_new_leaf() {
        assert_eq!(
            rename_path_with_leaf("folder/file.mkv", "renamed.mkv"),
            Some("folder/renamed.mkv".to_string())
        );
        assert_eq!(
            rename_path_with_leaf("folder\\sub\\file.mkv", "new.mkv"),
            Some("folder\\sub\\new.mkv".to_string())
        );
        assert_eq!(
            rename_path_with_leaf("file.mkv", "new.mkv"),
            Some("new.mkv".to_string())
        );
        assert_eq!(rename_path_with_leaf("folder/file.mkv", "bad/name"), None);
    }

    #[test]
    fn cross_seed_validates_same_instance() {
        assert!(!valid_cross_seed_targets("abc", "abc"));
        assert!(valid_cross_seed_targets("abc", "def"));
        assert!(!valid_cross_seed_targets("", "def"));
    }
}
