//! TVDB Service
//!
//! TV metadata primary source. API key must come from config/env; never hardcode.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tracing::warn;

const TVDB_API_BASE: &str = "https://api4.thetvdb.com/v4";

#[derive(Debug, Clone, Default)]
pub struct TvdbEnrichment {
    pub official_name: Option<String>,
    pub original_name: Option<String>,
    pub year: Option<u32>,
    pub poster_path: Option<String>,
    pub aliases: Vec<TvdbAlias>,
    pub translations: Vec<TvdbTranslation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TvdbAlias {
    pub title: String,
    pub country: String,
}

#[derive(Debug, Clone)]
pub struct TvdbTranslation {
    pub name: String,
    pub country: String,
}

#[derive(Debug, Clone)]
struct CachedToken {
    token: String,
    expires_at: Instant,
}

#[derive(Clone)]
pub struct TvdbService {
    client: Arc<Client>,
    api_key: Arc<RwLock<Option<String>>>,
    token: Arc<Mutex<Option<CachedToken>>>,
}

#[derive(Serialize)]
struct LoginRequest<'a> {
    apikey: &'a str,
}

impl TvdbService {
    pub fn new_with_default_client(api_key: Option<String>) -> Self {
        Self {
            client: Arc::new(
                Client::builder()
                    .timeout(Duration::from_secs(8))
                    .pool_max_idle_per_host(10)
                    .pool_idle_timeout(Duration::from_secs(90))
                    .build()
                    .unwrap_or_else(|_| Client::new()),
            ),
            api_key: Arc::new(RwLock::new(api_key.filter(|k| !k.trim().is_empty()))),
            token: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn enabled(&self) -> bool {
        self.api_key.read().await.is_some()
    }

    pub async fn set_api_key(&self, api_key: String) {
        *self.api_key.write().await = (!api_key.trim().is_empty()).then_some(api_key);
        *self.token.lock().await = None;
    }

    pub async fn api_key(&self) -> Option<String> {
        self.api_key.read().await.clone()
    }

    async fn bearer_token(&self) -> Option<String> {
        if let Some(cached) = self.token.lock().await.as_ref() {
            if cached.expires_at > Instant::now() {
                return Some(cached.token.clone());
            }
        }

        let api_key = self.api_key().await?;
        let url = format!("{}/login", TVDB_API_BASE);
        let resp = match self
            .client
            .post(url)
            .json(&LoginRequest { apikey: &api_key })
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp,
            Ok(resp) => {
                warn!("TVDB login returned {}", resp.status());
                return None;
            }
            Err(e) => {
                warn!("TVDB login failed: {}", e);
                return None;
            }
        };

        let body: Value = resp.json().await.ok()?;
        let token = body["data"]["token"].as_str()?.to_string();
        *self.token.lock().await = Some(CachedToken {
            token: token.clone(),
            expires_at: Instant::now() + Duration::from_secs(60 * 60 * 12),
        });
        Some(token)
    }

    async fn get(&self, path: &str) -> Option<Value> {
        let token = self.bearer_token().await?;
        let url = format!("{}{}", TVDB_API_BASE, path);
        match self.client.get(url).bearer_auth(token).send().await {
            Ok(resp) if resp.status().is_success() => resp.json::<Value>().await.ok(),
            Ok(resp) => {
                warn!("TVDB API returned {}: {}", resp.status(), path);
                None
            }
            Err(e) => {
                warn!("TVDB API request failed: {} - {}", path, e);
                None
            }
        }
    }

    pub async fn get_series(&self, tvdb_id: i64) -> Option<Value> {
        self.get(&format!("/series/{}", tvdb_id)).await
    }

    pub async fn get_series_extended(&self, tvdb_id: i64) -> Option<Value> {
        self.get(&format!("/series/{}/extended?meta=translations", tvdb_id))
            .await
    }

    pub async fn get_series_episodes_default(&self, tvdb_id: i64) -> Option<Vec<Value>> {
        let mut episodes = Vec::new();
        let mut page = 0;
        loop {
            let value = self
                .get(&format!("/series/{}/episodes/default/{}", tvdb_id, page))
                .await?;
            let data = value.get("data")?;
            if let Some(arr) = data.get("episodes").and_then(|v| v.as_array()) {
                episodes.extend(arr.iter().cloned());
            }
            let next = value
                .get("links")
                .and_then(|v| v.get("next"))
                .and_then(|v| v.as_u64());
            match next {
                Some(n) if n as usize > page => page = n as usize,
                _ => break,
            }
        }
        Some(episodes)
    }

    pub async fn get_tv_enrichment(&self, tvdb_id: i64) -> TvdbEnrichment {
        let mut out = TvdbEnrichment::default();
        let value = match self.get_series_extended(tvdb_id).await {
            Some(v) => Some(v),
            None => self.get_series(tvdb_id).await,
        };
        let data = match value {
            Some(v) => v["data"].clone(),
            None => return out,
        };

        out.official_name = data["name"].as_str().map(str::to_string);
        out.original_name = data["nameTranslations"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_str())
            .map(str::to_string);
        out.poster_path = data["image"].as_str().map(str::to_string);
        out.year = data["year"]
            .as_str()
            .and_then(|y| y.parse::<u32>().ok())
            .or_else(|| {
                data["firstAired"]
                    .as_str()
                    .and_then(|s| s.get(0..4))
                    .and_then(|y| y.parse().ok())
            });

        if let Some(aliases) = data["aliases"].as_array() {
            for a in aliases {
                let title = a["name"].as_str().or_else(|| a.as_str());
                if let Some(title) = title.filter(|s| !s.trim().is_empty()) {
                    out.aliases.push(TvdbAlias {
                        title: title.to_string(),
                        country: a["language"].as_str().unwrap_or("").to_string(),
                    });
                }
            }
        }

        if let Some(translations) = data["translations"].as_object() {
            for (country, t) in translations {
                if let Some(name) = t["name"].as_str().filter(|s| !s.trim().is_empty()) {
                    out.translations.push(TvdbTranslation {
                        name: name.to_string(),
                        country: country.clone(),
                    });
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn service_runtime_key_reload() {
        let service = TvdbService::new_with_default_client(None);
        assert!(!service.enabled().await);
        service.set_api_key("redacted".into()).await;
        assert!(service.enabled().await);
        assert_eq!(service.api_key().await.as_deref(), Some("redacted"));
        service.set_api_key(String::new()).await;
        assert!(!service.enabled().await);
    }
}
