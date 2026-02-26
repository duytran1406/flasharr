//! TMDB Service
//!
//! Centralized service for all TMDB API interactions.
//! Eliminates duplicated TMDB API calls across the codebase.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tracing::warn;

use crate::constants::TMDB_API_KEY;

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
    /// Alternative titles with their country codes
    pub alternative_titles: Vec<AlternativeTitle>,
    /// Translations
    pub translations: Vec<Translation>,
    /// Official title/name (TMDB display name, e.g. "Tales of Herding Gods")
    pub official_name: Option<String>,
    /// Original title in content's origin language (e.g. "牧神记" for Chinese shows)
    pub original_name: Option<String>,
    /// BCP-47 language code for the original language (e.g. "zh", "ko", "ja")
    pub original_language: Option<String>,
    /// Poster path
    pub poster_path: Option<String>,
    /// Collection info for movies
    pub collection: Option<CollectionInfo>,
}

/// Collection info for movies
#[derive(Debug, Clone)]
pub struct CollectionInfo {
    pub id: i64,
    pub name: String,
    pub parts: Vec<CollectionPart>,
}

/// Part of a collection
#[derive(Debug, Clone)]
pub struct CollectionPart {
    pub id: i64,
    pub title: String,
}

/// External IDs result
#[derive(Debug, Clone, Default)]
pub struct ExternalIds {
    pub imdb_id: Option<String>,
    pub tvdb_id: Option<i32>,
}

/// TMDB Service for centralized API access
#[derive(Clone)]
pub struct TmdbService {
    client: Arc<Client>,
}

impl TmdbService {
    /// Create a new TmdbService with shared HTTP client
    pub fn new(client: Arc<Client>) -> Self {
        Self { client }
    }

    /// Create with a new HTTP client (configured for performance)
    pub fn new_with_default_client() -> Self {
        Self {
            client: Arc::new(
                Client::builder()
                    .timeout(Duration::from_secs(8))
                    .pool_max_idle_per_host(10)
                    .pool_idle_timeout(Duration::from_secs(90))
                    .build()
                    .unwrap_or_else(|_| Client::new())
            ),
        }
    }

    // =========================================================================
    // Core HTTP Methods
    // =========================================================================

    /// Make a GET request to TMDB API
    async fn get(&self, path: &str) -> Option<Value> {
        let url = format!("{}{}?api_key={}", TMDB_API_BASE, path, TMDB_API_KEY);
        
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<Value>().await.ok()
            }
            Ok(resp) => {
                warn!("TMDB API returned {}: {}", resp.status(), path);
                None
            }
            Err(e) => {
                warn!("TMDB API request failed: {} - {}", path, e);
                None
            }
        }
    }

    /// Make a GET request with additional query parameters
    async fn get_with_params(&self, path: &str, params: &[(&str, &str)]) -> Option<Value> {
        let mut url = format!("{}{}?api_key={}", TMDB_API_BASE, path, TMDB_API_KEY);
        for (key, value) in params {
            url.push_str(&format!("&{}={}", key, value));
        }
        
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                resp.json::<Value>().await.ok()
            }
            _ => None,
        }
    }

    // =========================================================================
    // TV Show Methods
    // =========================================================================

    /// Get TV show details
    pub async fn get_tv_details(&self, tmdb_id: i64) -> Option<Value> {
        self.get(&format!("/tv/{}", tmdb_id)).await
    }

    /// Get TV show alternative titles
    pub async fn get_tv_alternative_titles(&self, tmdb_id: i64) -> Vec<AlternativeTitle> {
        let path = format!("/tv/{}/alternative_titles", tmdb_id);
        
        if let Some(data) = self.get(&path).await {
            if let Some(results) = data["results"].as_array() {
                return results.iter()
                    .filter_map(|t| {
                        let title = t["title"].as_str()
                            .or(t["name"].as_str())?;
                        let iso = t["iso_3166_1"].as_str().unwrap_or("");
                        Some(AlternativeTitle {
                            title: title.to_string(),
                            iso_3166_1: iso.to_string(),
                        })
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    /// Get TV show translations
    pub async fn get_tv_translations(&self, tmdb_id: i64) -> Vec<Translation> {
        let path = format!("/tv/{}/translations", tmdb_id);
        
        if let Some(data) = self.get(&path).await {
            if let Some(translations) = data["translations"].as_array() {
                return translations.iter()
                    .filter_map(|t| {
                        let iso = t["iso_3166_1"].as_str().unwrap_or("");
                        let name = t["data"]["name"].as_str()
                            .filter(|n| !n.is_empty())?;
                        Some(Translation {
                            name: name.to_string(),
                            iso_3166_1: iso.to_string(),
                        })
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    /// Get TV season details
    pub async fn get_season_details(&self, tmdb_id: i64, season: i32) -> Option<Value> {
        self.get(&format!("/tv/{}/season/{}", tmdb_id, season)).await
    }

    /// Get full TV enrichment in ONE API call using append_to_response.
    /// Previously made 3 sequential/parallel calls; now a single request.
    pub async fn get_tv_enrichment(&self, tmdb_id: i64) -> MediaEnrichment {
        let mut enrichment = MediaEnrichment::default();

        // Single API call: details + alternative_titles + translations
        let data = self.get_with_params(
            &format!("/tv/{}", tmdb_id),
            &[("append_to_response", "alternative_titles,translations")],
        ).await;

        if let Some(data) = data {
            // Parse details
            enrichment.official_name = data["name"].as_str().map(|s| s.to_string());
            enrichment.original_name = data["original_name"].as_str().map(|s| s.to_string());
            enrichment.original_language = data["original_language"].as_str().map(|s| s.to_string());
            enrichment.poster_path = data["poster_path"].as_str().map(|s| s.to_string());

            // Parse alternative titles from appended response
            if let Some(results) = data["alternative_titles"]["results"].as_array() {
                enrichment.alternative_titles = results.iter()
                    .filter_map(|t| {
                        let title = t["title"].as_str().or(t["name"].as_str())?;
                        let iso = t["iso_3166_1"].as_str().unwrap_or("");
                        Some(AlternativeTitle {
                            title: title.to_string(),
                            iso_3166_1: iso.to_string(),
                        })
                    })
                    .collect();
            }

            // Parse translations from appended response
            if let Some(translations) = data["translations"]["translations"].as_array() {
                enrichment.translations = translations.iter()
                    .filter_map(|t| {
                        let iso = t["iso_3166_1"].as_str().unwrap_or("");
                        let name = t["data"]["name"].as_str()
                            .filter(|n| !n.is_empty())?;
                        Some(Translation {
                            name: name.to_string(),
                            iso_3166_1: iso.to_string(),
                        })
                    })
                    .collect();
            }
        }

        enrichment
    }

    // =========================================================================
    // Movie Methods
    // =========================================================================

    /// Get movie details
    pub async fn get_movie_details(&self, tmdb_id: i64) -> Option<Value> {
        self.get_with_params(
            &format!("/movie/{}", tmdb_id),
            &[("append_to_response", "belongs_to_collection")],
        ).await
    }

    /// Get movie alternative titles
    pub async fn get_movie_alternative_titles(&self, tmdb_id: i64) -> Vec<AlternativeTitle> {
        let path = format!("/movie/{}/alternative_titles", tmdb_id);
        
        if let Some(data) = self.get(&path).await {
            if let Some(titles) = data["titles"].as_array() {
                return titles.iter()
                    .filter_map(|t| {
                        let title = t["title"].as_str()?;
                        let iso = t["iso_3166_1"].as_str().unwrap_or("");
                        Some(AlternativeTitle {
                            title: title.to_string(),
                            iso_3166_1: iso.to_string(),
                        })
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    /// Get collection details
    pub async fn get_collection(&self, collection_id: i64) -> Option<CollectionInfo> {
        let data = self.get(&format!("/collection/{}", collection_id)).await?;
        
        let name = data["name"].as_str()?.to_string();
        let parts = data["parts"].as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| {
                        Some(CollectionPart {
                            id: p["id"].as_i64()?,
                            title: p["title"].as_str()?.to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Some(CollectionInfo {
            id: collection_id,
            name,
            parts,
        })
    }

    /// Get full movie enrichment in ONE API call using append_to_response.
    /// Previously made 3 sequential calls; now 1 call (+1 for collection if needed).
    pub async fn get_movie_enrichment(&self, tmdb_id: i64) -> MediaEnrichment {
        let mut enrichment = MediaEnrichment::default();

        // Single API call: details + alternative_titles + belongs_to_collection
        let data = self.get_with_params(
            &format!("/movie/{}", tmdb_id),
            &[("append_to_response", "alternative_titles")],
        ).await;

        if let Some(data) = data {
            // Parse details
            enrichment.official_name = data["title"].as_str().map(|s| s.to_string());
            enrichment.original_name = data["original_title"].as_str().map(|s| s.to_string());
            enrichment.original_language = data["original_language"].as_str().map(|s| s.to_string());
            enrichment.poster_path = data["poster_path"].as_str().map(|s| s.to_string());

            // Parse alternative titles from appended response
            if let Some(titles) = data["alternative_titles"]["titles"].as_array() {
                enrichment.alternative_titles = titles.iter()
                    .filter_map(|t| {
                        let title = t["title"].as_str()?;
                        let iso = t["iso_3166_1"].as_str().unwrap_or("");
                        Some(AlternativeTitle {
                            title: title.to_string(),
                            iso_3166_1: iso.to_string(),
                        })
                    })
                    .collect();
            }

            // Check for collection (only extra call if movie belongs to one)
            if let Some(coll_id) = data["belongs_to_collection"]["id"].as_i64() {
                enrichment.collection = self.get_collection(coll_id).await;
            }
        }

        enrichment
    }

    // =========================================================================
    // Lookup Methods
    // =========================================================================

    /// Find by external ID (IMDB, TVDB)
    pub async fn find_by_external_id(&self, external_id: &str, source: &str) -> Option<Value> {
        self.get_with_params(
            &format!("/find/{}", external_id),
            &[("external_source", source)],
        ).await
    }

    /// Get external IDs for a TV show
    pub async fn get_tv_external_ids(&self, tmdb_id: i64) -> ExternalIds {
        let path = format!("/tv/{}/external_ids", tmdb_id);
        
        if let Some(data) = self.get(&path).await {
            return ExternalIds {
                imdb_id: data["imdb_id"].as_str().map(|s| s.to_string()),
                tvdb_id: data["tvdb_id"].as_i64().map(|v| v as i32),
            };
        }
        ExternalIds::default()
    }

    // =========================================================================
    // Search Methods
    // =========================================================================

    /// Search for movies/TV shows
    pub async fn search(&self, media_type: &str, query: &str) -> Option<Value> {
        self.get_with_params(
            &format!("/search/{}", media_type),
            &[("query", query)],
        ).await
    }

    /// Search collections
    pub async fn search_collection(&self, query: &str) -> Option<Value> {
        self.get_with_params("/search/collection", &[("query", query)]).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tmdb_service_creation() {
        let service = TmdbService::new_with_default_client();
        assert!(Arc::strong_count(&service.client) == 1);
    }

    #[test]
    fn test_alternative_title_struct() {
        let title = AlternativeTitle {
            title: "Spider-Man".to_string(),
            iso_3166_1: "US".to_string(),
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
