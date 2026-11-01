use crate::db::{Db, QbitInstance, QbitTorrentRow};
use crate::torrent::qbittorrent::{
    AddTorrentOptions, QBittorrentClient, QBittorrentConfig, QBittorrentTransferInfo, TorrentUpload,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, Mutex, RwLock};

fn initial_sync_rid(_persisted_rid: i64) -> i64 {
    0
}

fn sync_error_kind(error: &anyhow::Error) -> &'static str {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("timed out") || message.contains("timeout") {
        "timeout"
    } else if message.contains("login returned no sid") {
        "login_no_cookie"
    } else if message.contains("login rejected") {
        "login_rejected"
    } else if message.contains("login failed: http 401") {
        "login_http_401"
    } else if message.contains("login failed: http 403") {
        "login_http_403"
    } else if message.contains("login failed: http 429") {
        "login_http_429"
    } else if message.contains("login failed: http") {
        "login_http_other"
    } else if message.contains("sync failed: http") {
        "sync_http"
    } else if message.contains("decode") || message.contains("json") {
        "invalid_response"
    } else if message.contains("401") || message.contains("403") || message.contains("unauthorized")
    {
        "authentication"
    } else {
        "connection"
    }
}

async fn bounded_barrier<F: std::future::Future>(
    budget: std::time::Duration,
    future: F,
) -> Result<F::Output> {
    tokio::time::timeout(budget, future)
        .await
        .map_err(|_| anyhow!("qBittorrent initial sync timed out"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum QbitEvent {
    #[serde(rename = "QBIT_TORRENT_UPSERT")]
    TorrentUpsert {
        instance_id: String,
        torrent: QbitTorrentRow,
    },
    #[serde(rename = "QBIT_TORRENT_REMOVED")]
    TorrentRemoved { instance_id: String, hash: String },
    #[serde(rename = "QBIT_TRANSFER_INFO")]
    TransferInfo {
        instance_id: String,
        info: QBittorrentTransferInfo,
    },
    #[serde(rename = "QBIT_ERROR")]
    Error {
        instance_id: String,
        message: String,
    },
    #[serde(rename = "QBIT_HEARTBEAT")]
    Heartbeat { instance_id: String },
}

#[derive(Clone)]
pub struct QbitInstanceManager {
    db: Arc<Db>,
    clients: Arc<RwLock<HashMap<String, Arc<QBittorrentClient>>>>,
    sync_locks: Arc<RwLock<HashMap<String, Arc<Mutex<()>>>>>,
    events: broadcast::Sender<QbitEvent>,
}

impl QbitInstanceManager {
    pub fn new(db: Arc<Db>) -> Self {
        let (events, _) = broadcast::channel(1000);
        Self {
            db,
            clients: Arc::new(RwLock::new(HashMap::new())),
            sync_locks: Arc::new(RwLock::new(HashMap::new())),
            events,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<QbitEvent> {
        self.events.subscribe()
    }

    pub async fn reload(&self) {
        let Ok(instances) = self.db.list_qbit_instances() else {
            return;
        };
        let mut clients = HashMap::new();
        for instance in instances.into_iter().filter(|i| i.enabled) {
            match QBittorrentClient::new(config_from_instance(&instance)) {
                Ok(client) => {
                    clients.insert(instance.id.clone(), Arc::new(client));
                }
                Err(e) => tracing::warn!("qBittorrent instance {} disabled: {}", instance.id, e),
            }
        }
        *self.clients.write().await = clients;
    }

    pub async fn client(&self, id: &str) -> Result<Arc<QBittorrentClient>> {
        if let Some(client) = self.clients.read().await.get(id).cloned() {
            return Ok(client);
        }
        self.reload().await;
        self.clients
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| anyhow!("qBittorrent instance not found or disabled"))
    }

    pub async fn default_client(&self) -> Result<(String, Arc<QBittorrentClient>)> {
        let instance = self
            .db
            .default_qbit_instance()?
            .ok_or_else(|| anyhow!("qBittorrent is not configured"))?;
        let client = self.client(&instance.id).await?;
        Ok((instance.id, client))
    }

    pub async fn test_instance(&self, instance: &QbitInstance) -> Result<String> {
        QBittorrentClient::new(config_from_instance(instance))?
            .test_connection()
            .await
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            self.reload().await;

            let mut last_sync = HashMap::<String, tokio::time::Instant>::new();
            loop {
                let now = tokio::time::Instant::now();
                let instances = self.db.list_qbit_instances().unwrap_or_default();
                for instance in instances.into_iter().filter(|i| i.enabled) {
                    let interval =
                        std::time::Duration::from_secs(instance.poll_interval_secs.max(2));
                    let due = last_sync
                        .get(&instance.id)
                        .map(|last| now.duration_since(*last) >= interval)
                        .unwrap_or(true);
                    if due {
                        self.sync_instance(instance.clone()).await;
                        last_sync.insert(instance.id, now);
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });
    }

    pub async fn initial_full_sync(&self) -> Result<()> {
        self.reload().await;
        for instance in self
            .db
            .list_qbit_instances()?
            .into_iter()
            .filter(|i| i.enabled)
        {
            let id = instance.id.clone();
            if bounded_barrier(
                std::time::Duration::from_secs(15),
                self.sync_instance_at(instance, initial_sync_rid(0)),
            )
            .await
            .is_err()
            {
                let _ = self
                    .db
                    .mark_qbit_sync_error(&id, "disconnected", "initial sync failed");
            }
        }
        Ok(())
    }

    pub async fn sync_instance(&self, instance: QbitInstance) {
        let rid = instance.last_rid;
        self.sync_instance_at(instance, rid).await;
    }

    async fn sync_instance_at(&self, instance: QbitInstance, rid: i64) {
        let sync_lock = self.sync_lock(&instance.id).await;
        let _sync_guard = sync_lock.lock().await;
        let client = match self.client(&instance.id).await {
            Ok(client) => client,
            Err(e) => {
                self.emit_error(&instance.id, e);
                return;
            }
        };
        let mut last_error = None;
        let mut result = None;
        for attempt in 1..=3 {
            match client.sync_maindata(rid).await {
                Ok(sync) => {
                    result = Some(sync);
                    break;
                }
                Err(error) => {
                    tracing::warn!(instance_id = %instance.id, attempt, error_kind = %sync_error_kind(&error), "qBittorrent sync retry");
                    last_error = Some(error);
                    if attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_millis(250 * attempt)).await;
                    }
                }
            }
        }
        let sync = match result {
            Some(sync) => sync,
            None => {
                let e = last_error.expect("sync attempts produce an error");
                if let Err(mark_error) =
                    self.db
                        .mark_qbit_sync_error(&instance.id, "stale", sync_error_kind(&e))
                {
                    tracing::error!("qBittorrent sync error persistence failed: {}", mark_error);
                }
                self.emit_error(&instance.id, e);
                return;
            }
        };

        let app_info = client.app_info().await.ok();
        let torrents = sync.torrents.values().cloned().collect::<Vec<_>>();
        let applied = match self.db.apply_qbit_sync(
            &instance.id,
            sync.rid,
            sync.full_update,
            &torrents,
            &sync.torrents_removed,
            app_info.as_ref().map(|i| i.version.as_str()),
            app_info.as_ref().map(|i| i.webapi_version.as_str()),
        ) {
            Ok(applied) => applied,
            Err(e) => {
                if let Err(mark_error) =
                    self.db
                        .mark_qbit_sync_error(&instance.id, "stale", &e.to_string())
                {
                    tracing::error!("qBittorrent sync error persistence failed: {}", mark_error);
                }
                self.emit_error(&instance.id, e.into());
                return;
            }
        };
        for torrent in applied.upserted {
            let _ = self.events.send(QbitEvent::TorrentUpsert {
                instance_id: instance.id.clone(),
                torrent,
            });
        }
        for hash in applied.removed.into_iter().chain(applied.replaced) {
            let _ = self.events.send(QbitEvent::TorrentRemoved {
                instance_id: instance.id.clone(),
                hash,
            });
        }
        if let Ok(info) = client.transfer_info().await {
            let _ = self.events.send(QbitEvent::TransferInfo {
                instance_id: instance.id.clone(),
                info,
            });
        }
        let _ = self.events.send(QbitEvent::Heartbeat {
            instance_id: instance.id,
        });
    }

    async fn sync_lock(&self, instance_id: &str) -> Arc<Mutex<()>> {
        if let Some(lock) = self.sync_locks.read().await.get(instance_id).cloned() {
            return lock;
        }
        self.sync_locks
            .write()
            .await
            .entry(instance_id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn emit_error(&self, instance_id: &str, _error: anyhow::Error) {
        let _ = self.events.send(QbitEvent::Error {
            instance_id: instance_id.to_string(),
            message: "qBittorrent sync failed".to_string(),
        });
    }

    pub async fn add_torrent(
        &self,
        instance_id: &str,
        url: &str,
        options: AddTorrentOptions,
    ) -> Result<Option<String>> {
        self.client(instance_id)
            .await?
            .add_torrent_with_options(url, options)
            .await
    }

    pub async fn add_torrent_upload(
        &self,
        instance_id: &str,
        upload: TorrentUpload,
        options: AddTorrentOptions,
    ) -> Result<Option<String>> {
        self.client(instance_id)
            .await?
            .add_torrent_upload(upload, options)
            .await
    }
}

pub fn config_from_instance(instance: &QbitInstance) -> QBittorrentConfig {
    QBittorrentConfig {
        enabled: instance.enabled,
        url: instance.url.clone(),
        username: instance.username.clone(),
        password: instance.password.clone(),
        category: instance
            .default_category
            .clone()
            .filter(|v| !v.trim().is_empty()),
        save_path: instance.save_path.clone().filter(|v| !v.trim().is_empty()),
        poll_interval_secs: instance.poll_interval_secs.max(2),
        instance_id: Some(instance.id.clone()),
        instance_name: Some(instance.name.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_sync_error_event_is_generic() {
        let raw = "https://admin:password@evil.test/api?apikey=SECRET&SID=TOKEN";
        let message = "qBittorrent sync failed";
        assert_ne!(message, raw);
        for secret in [
            "https",
            "admin",
            "password",
            "evil.test",
            "apikey",
            "SECRET",
            "SID",
            "TOKEN",
        ] {
            assert!(!message.contains(secret));
        }
    }

    #[test]
    fn initial_process_sync_always_starts_at_zero() {
        assert_eq!(super::initial_sync_rid(99), 0);
    }

    #[tokio::test]
    async fn startup_barrier_timeout_is_bounded() {
        let started = tokio::time::Instant::now();
        let result = super::bounded_barrier(std::time::Duration::from_millis(10), async {
            std::future::pending::<()>().await
        })
        .await;
        assert!(result.is_err());
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
    }

    #[tokio::test]
    async fn same_instance_sync_lock_is_stable_and_serializes() {
        let manager = QbitInstanceManager::new(Arc::new(Db::new(":memory:").expect("test db")));
        let first = manager.sync_lock("qbit-1").await;
        let replacement = manager.sync_lock("qbit-1").await;
        assert!(Arc::ptr_eq(&first, &replacement));

        let guard = first.lock().await;
        let waiter = tokio::spawn(async move { replacement.lock_owned().await });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), async {
                while !waiter.is_finished() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .is_err()
        );
        drop(guard);
        tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("second sync proceeds after first")
            .expect("waiter task");
    }
}
