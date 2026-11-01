//! TMDB Service
//!
//! Centralized service for all TMDB API interactions.
//! Eliminates duplicated TMDB API calls across the codebase.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::warn;

use crate::constants::tmdb_api_key;

const TMDB_API_BASE: &str = "https://api.themoviedb.org/3";

/// Alternative title from TMDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeTitle {
    pub title: String,
    pub iso_3166_1: String,
}

/// Translation from TMDB
#[derive(Debug, Clone)]
pub struct Translation {
    pub name: String,
    pub iso_3166_1: String,
}

/// Media enrichment data from TMDB
#[derive(Debug, Clone, Default)]
pub struct MediaEnrichment {
    pub alternative_titles: Vec<AlternativeTitle>,
    pub translations: Vec<Translation>,
    pub official_name: Option<String>,
    pub original_name: Option<String>,
    pub year: Option<u32>,
    pub original_language: Option<String>,
    pub poster_path: Option<String>,
    pub collection: Option<CollectionInfo>,
}

#[derive(Debug, Clone)]
pub struct CollectionInfo {
    pub id: i64,
    pub name: String,
    pub parts: Vec<CollectionPart>,
}

#[derive(Debug, Clone)]
pub struct CollectionPart {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExternalIds {
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i32>,
}

#[derive(Clone)]
pub struct TmdbService {
    client: Arc<Client>,
    api_key: Arc<RwLock<String>>,
}

impl TmdbService {
    pub fn new(client: Arc<Client>) -> Self {
        Self::new_with_client_and_key(client, tmdb_api_key())
    }

    pub fn new_with_client_and_key(client: Arc<Client>, api_key: String) -> Self {
        Self {
            client,
            api_key: Arc::new(RwLock::new(api_key)),
        }
    }

    pub fn new_with_default_client() -> Self {
        Self::new_with_api_key(tmdb_api_key())
    }

    pub fn new_with_api_key(api_key: String) -> Self {
        Self::new_with_client_and_key(
            Arc::new(
                Client::builder()
                    .no_proxy()
                    .timeout(Duration::from_secs(8))
                    .pool_max_idle_per_host(10)
                    .pool_idle_timeout(Duration::from_secs(90))
                    .build()
                    .unwrap_or_else(|_| Client::new()),
            ),
            api_key,
        )
    }

    pub async fn set_api_key(&self, api_key: String) {
        *self.api_key.write().await = api_key;
    }

    pub async fn api_key(&self) -> String {
        self.api_key.read().await.clone()
    }

    async fn get(&self, path: &str) -> Option<Value> {
        let url = format!("{}{}", TMDB_API_BASE, path);
        match self
            .client
            .get(&url)
            .query(&[("api_key", self.api_key().await)])
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp.json::<Value>().await.ok(),
            Ok(resp) => {
                warn!("TMDB API returned {}: {}", resp.status(), path);
                None
            }
            Err(error) => {
                warn!(
                    "TMDB API request failed: {} (timeout={})",
                    path,
                    error.is_timeout()
                );
                None
            }
        }
    }

    async fn get_with_params(&self, path: &str, params: &[(&str, &str)]) -> Option<Value> {
        let url = format!("{}{}", TMDB_API_BASE, path);
        match self
            .client
            .get(&url)
            .query(&[("api_key", self.api_key().await)])
            .query(params)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => resp.json::<Value>().await.ok(),
            _ => None,
        }
    }

    pub async fn get_tv_details(&self, tmdb_id: i64) -> Option<Value> {
        self.get(&format!("/tv/{}", tmdb_id)).await
    }

    pub async fn get_tv_alternative_titles(&self, tmdb_id: i64) -> Vec<AlternativeTitle> {
        self.get(&format!("/tv/{}/alternative_titles", tmdb_id))
            .await
            .and_then(|data| data["results"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|title| {
                Some(AlternativeTitle {
                    title: title["title"]
                        .as_str()
                        .or(title["name"].as_str())?
                        .to_string(),
                    iso_3166_1: title["iso_3166_1"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect()
    }

    pub async fn get_tv_translations(&self, tmdb_id: i64) -> Vec<Translation> {
        self.get(&format!("/tv/{}/translations", tmdb_id))
            .await
            .and_then(|data| data["translations"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|translation| {
                Some(Translation {
                    name: translation["data"]["name"]
                        .as_str()
                        .filter(|name| !name.is_empty())?
                        .to_string(),
                    iso_3166_1: translation["iso_3166_1"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect()
    }

    pub async fn get_season_details(&self, tmdb_id: i64, season: i32) -> Option<Value> {
        self.get(&format!("/tv/{}/season/{}", tmdb_id, season))
            .await
    }

    pub async fn get_tv_enrichment(&self, tmdb_id: i64) -> MediaEnrichment {
        let mut enrichment = MediaEnrichment::default();
        let data = self
            .get_with_params(
                &format!("/tv/{}", tmdb_id),
                &[("append_to_response", "alternative_titles,translations")],
            )
            .await;
        if let Some(data) = data {
            enrichment.official_name = data["name"].as_str().map(str::to_string);
            enrichment.original_name = data["original_name"].as_str().map(str::to_string);
            enrichment.original_language = data["original_language"].as_str().map(str::to_string);
            enrichment.poster_path = data["poster_path"].as_str().map(str::to_string);
            enrichment.year = data["first_air_date"]
                .as_str()
                .and_then(|date| date.get(0..4))
                .and_then(|year| year.parse().ok());
            enrichment.alternative_titles = data["alternative_titles"]["results"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|title| {
                    Some(AlternativeTitle {
                        title: title["title"]
                            .as_str()
                            .or(title["name"].as_str())?
                            .to_string(),
                        iso_3166_1: title["iso_3166_1"].as_str().unwrap_or("").to_string(),
                    })
                })
                .collect();
            enrichment.translations = data["translations"]["translations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|translation| {
                    Some(Translation {
                        name: translation["data"]["name"]
                            .as_str()
                            .filter(|name| !name.is_empty())?
                            .to_string(),
                        iso_3166_1: translation["iso_3166_1"].as_str().unwrap_or("").to_string(),
                    })
                })
                .collect();
        }
        enrichment
    }

    pub async fn get_movie_details(&self, tmdb_id: i64) -> Option<Value> {
        self.get_with_params(
            &format!("/movie/{}", tmdb_id),
            &[("append_to_response", "belongs_to_collection")],
        )
        .await
    }

    pub async fn get_movie_alternative_titles(&self, tmdb_id: i64) -> Vec<AlternativeTitle> {
        self.get(&format!("/movie/{}/alternative_titles", tmdb_id))
            .await
            .and_then(|data| data["titles"].as_array().cloned())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|title| {
                Some(AlternativeTitle {
                    title: title["title"].as_str()?.to_string(),
                    iso_3166_1: title["iso_3166_1"].as_str().unwrap_or("").to_string(),
                })
            })
            .collect()
    }

    pub async fn get_collection(&self, collection_id: i64) -> Option<CollectionInfo> {
        let data = self.get(&format!("/collection/{}", collection_id)).await?;
        Some(CollectionInfo {
            id: collection_id,
            name: data["name"].as_str()?.to_string(),
            parts: data["parts"]
                .as_array()
                .map(|parts| {
                    parts
                        .iter()
                        .filter_map(|part| {
                            Some(CollectionPart {
                                id: part["id"].as_i64()?,
                                title: part["title"].as_str()?.to_string(),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    pub async fn get_movie_enrichment(&self, tmdb_id: i64) -> MediaEnrichment {
        let mut enrichment = MediaEnrichment::default();
        let data = self
            .get_with_params(
                &format!("/movie/{}", tmdb_id),
                &[("append_to_response", "alternative_titles")],
            )
            .await;
        if let Some(data) = data {
            enrichment.official_name = data["title"].as_str().map(str::to_string);
            enrichment.original_name = data["original_title"].as_str().map(str::to_string);
            enrichment.original_language = data["original_language"].as_str().map(str::to_string);
            enrichment.poster_path = data["poster_path"].as_str().map(str::to_string);
            enrichment.alternative_titles = data["alternative_titles"]["titles"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|title| {
                    Some(AlternativeTitle {
                        title: title["title"].as_str()?.to_string(),
                        iso_3166_1: title["iso_3166_1"].as_str().unwrap_or("").to_string(),
                    })
                })
                .collect();
            if let Some(collection_id) = data["belongs_to_collection"]["id"].as_i64() {
                enrichment.collection = self.get_collection(collection_id).await;
            }
        }
        enrichment
    }

    pub async fn find_by_external_id(&self, external_id: &str, source: &str) -> Option<Value> {
        self.get_with_params(
            &format!("/find/{}", external_id),
            &[("external_source", source)],
        )
        .await
    }

    pub async fn get_tv_external_ids(&self, tmdb_id: i64) -> ExternalIds {
        self.get(&format!("/tv/{}/external_ids", tmdb_id))
            .await
            .map(|data| ExternalIds {
                imdb_id: data["imdb_id"].as_str().map(str::to_string),
                tvdb_id: data["tvdb_id"].as_i64().map(|value| value as i32),
            })
            .unwrap_or_default()
    }

    pub async fn get_movie_external_ids(&self, tmdb_id: i64) -> ExternalIds {
        let data = self.get(&format!("/movie/{tmdb_id}/external_ids")).await;
        ExternalIds {
            imdb_id: data
                .as_ref()
                .and_then(|value| value["imdb_id"].as_str())
                .map(str::to_string),
            tvdb_id: None,
        }
    }

    pub async fn resolve_tv_title_id(&self, title: &str) -> Option<i64> {
        let results = self.search("tv", title).await?;
        Self::resolve_tv_search_id(&results, title)
    }

    fn resolve_tv_search_id(results: &Value, title: &str) -> Option<i64> {
        let wanted = title.trim().to_lowercase();
        let tokens = wanted.split_whitespace().collect::<Vec<_>>();
        results["results"].as_array()?.iter().find_map(|row| {
            let matches = ["name", "original_name"].into_iter().filter_map(|key| row[key].as_str()).any(|name| {
                let normalized = name.trim().to_lowercase();
                normalized == wanted || tokens.iter().all(|token| normalized.split_whitespace().any(|part| part.trim_end_matches('s') == *token))
            });
            matches.then(|| row["id"].as_i64()).flatten()
        })
    }

    pub async fn search(&self, media_type: &str, query: &str) -> Option<Value> {
        self.search_checked(media_type, query).await.ok().flatten()
    }

    pub async fn search_checked(
        &self,
        media_type: &str,
        query: &str,
    ) -> Result<Option<Value>, String> {
        let api_key = self.api_key().await;
        let response = self
            .client
            .get(format!("{}/search/{}", TMDB_API_BASE, media_type))
            .query(&[("api_key", api_key.as_str()), ("query", query)])
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err(format!("TMDB API returned {}", response.status()));
        }
        response
            .json::<Value>()
            .await
            .map(Some)
            .map_err(|error| error.to_string())
    }

    pub async fn search_collection(&self, query: &str) -> Option<Value> {
        self.get_with_params("/search/collection", &[("query", query)])
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tv_title_resolution_prefers_exact_official_or_original_name() {
        let results = serde_json::json!({"results": [
            {"id": 1, "name": "Shopaholics"},
            {"id": 215072, "name": "A Shop for Killers", "original_name": "킬러들의 쇼핑몰"}
        ]});
        assert_eq!(
            TmdbService::resolve_tv_search_id(&results, "A Shop for Killers"),
            Some(215072)
        );
        assert_eq!(
            TmdbService::resolve_tv_search_id(&results, "Shop for Killer"),
            Some(215072)
        );
    }

    #[tokio::test]
    async fn test_tmdb_service_creation_and_runtime_key_reload() {
        let service = TmdbService::new_with_api_key("first".into());
        assert_eq!(Arc::strong_count(&service.client), 1);
        assert_eq!(service.api_key().await, "first");
        service.set_api_key("second".into()).await;
        assert_eq!(service.api_key().await, "second");
    }

    #[test]
    fn test_alternative_title_struct() {
        let title = AlternativeTitle {
            title: "Spider-Man".into(),
            iso_3166_1: "US".into(),
        };
        assert_eq!(title.title, "Spider-Man");
        assert_eq!(title.iso_3166_1, "US");
    }

    #[test]
    fn test_media_enrichment_default() {
        let enrichment = MediaEnrichment::default();
        assert!(enrichment.alternative_titles.is_empty());
        assert!(enrichment.translations.is_empty());
        assert!(enrichment.official_name.is_none());
        assert!(enrichment.poster_path.is_none());
        assert!(enrichment.collection.is_none());
    }

    #[test]
    fn test_external_ids_default() {
        let ids = ExternalIds::default();
        assert!(ids.imdb_id.is_none());
        assert!(ids.tvdb_id.is_none());
    }
}
