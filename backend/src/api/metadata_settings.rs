use crate::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const TMDB_TEST_URL: &str = "https://api.themoviedb.org/3/configuration";
const TVDB_TEST_URL: &str = "https://api4.thetvdb.com/v4/login";

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/metadata", get(get_metadata))
        .route("/metadata/:provider", put(update_metadata))
        .route("/metadata/:provider/test", post(test_metadata))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettings {
    provider: &'static str,
    configured: bool,
    source: &'static str,
    masked_key: String,
}

#[derive(Serialize)]
struct MetadataResponse {
    providers: Vec<ProviderSettings>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataKeyRequest {
    api_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetadataTestRequest {
    api_key: Option<String>,
}

#[derive(Serialize)]
struct TestResponse {
    success: bool,
    message: String,
}

fn provider_name(value: &str) -> Result<&'static str, (StatusCode, String)> {
    match value {
        "tmdb" => Ok("tmdb"),
        "tvdb" => Ok("tvdb"),
        _ => Err((StatusCode::NOT_FOUND, "metadata provider not found".into())),
    }
}

fn validate_key(value: &str) -> Result<(), (StatusCode, String)> {
    if value.is_empty()
        || value.len() > 512
        || value != value.trim()
        || !value.is_ascii()
        || value.bytes().any(|byte| byte.is_ascii_control())
    {
        Err((
            StatusCode::BAD_REQUEST,
            "API key must be 1-512 ASCII characters without whitespace padding or controls".into(),
        ))
    } else {
        Ok(())
    }
}

fn setting_key(provider: &str) -> String {
    format!("metadata_{provider}_api_key_encrypted")
}

fn scope(provider: &str) -> String {
    format!("metadata:{provider}:api-key")
}

fn registry() -> Result<crate::stremio::AddonRegistry, (StatusCode, String)> {
    crate::stremio::AddonRegistry::load_or_create().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "metadata secret registry unavailable".into(),
        )
    })
}

fn stored_key(state: &AppState, provider: &str) -> Result<Option<String>, (StatusCode, String)> {
    let Some(encrypted) = state.db.get_setting(&setting_key(provider)).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "metadata settings unavailable".into(),
        )
    })?
    else {
        return Ok(None);
    };
    registry()?
        .decrypt_secret(&scope(provider), &encrypted)
        .map(Some)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "stored metadata key could not be decrypted".into(),
            )
        })
}

fn fallback_key(state: &AppState, provider: &str) -> String {
    match provider {
        "tmdb" => state.config.external.tmdb_api_key.clone(),
        "tvdb" => state.config.external.tvdb_api_key.clone(),
        _ => String::new(),
    }
}

fn effective_key(
    state: &AppState,
    provider: &str,
) -> Result<(String, &'static str), (StatusCode, String)> {
    if let Some(key) = stored_key(state, provider)? {
        return Ok((key, "database"));
    }
    let fallback = fallback_key(state, provider);
    Ok((
        fallback.clone(),
        if fallback.is_empty() {
            "none"
        } else {
            "environment"
        },
    ))
}

fn mask_key(value: &str) -> String {
    let suffix = value.chars().rev().take(4).collect::<String>();
    if suffix.is_empty() {
        String::new()
    } else if value.chars().count() <= 4 {
        "••••".into()
    } else {
        format!("••••••••{}", suffix.chars().rev().collect::<String>())
    }
}

fn provider_settings(
    state: &AppState,
    provider: &'static str,
) -> Result<ProviderSettings, (StatusCode, String)> {
    let (key, source) = effective_key(state, provider)?;
    Ok(ProviderSettings {
        provider,
        configured: !key.is_empty(),
        source,
        masked_key: mask_key(&key),
    })
}

async fn get_metadata(
    State(state): State<Arc<AppState>>,
) -> Result<Json<MetadataResponse>, (StatusCode, String)> {
    Ok(Json(MetadataResponse {
        providers: vec![
            provider_settings(&state, "tmdb")?,
            provider_settings(&state, "tvdb")?,
        ],
    }))
}

async fn update_metadata(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Json(payload): Json<MetadataKeyRequest>,
) -> Result<Json<ProviderSettings>, (StatusCode, String)> {
    let provider = provider_name(&provider)?;
    validate_key(&payload.api_key)?;
    let _update_guard = state.metadata_update_lock.lock().await;
    probe_metadata(&state, provider, &payload.api_key).await?;
    let encrypted = registry()?
        .encrypt_secret(&scope(provider), &payload.api_key)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "metadata key encryption failed".into(),
            )
        })?;
    state
        .db
        .save_setting(&setting_key(provider), &encrypted)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "metadata key persistence failed".into(),
            )
        })?;
    reload_provider(&state, provider, payload.api_key).await;
    state.search_cache.invalidate_all();
    state.tmdb_cache.invalidate_all();
    Ok(Json(provider_settings(&state, provider)?))
}

async fn reload_provider(state: &AppState, provider: &str, key: String) {
    match provider {
        "tmdb" => state.tmdb_service.set_api_key(key).await,
        "tvdb" => state.tvdb_service.set_api_key(key).await,
        _ => unreachable!("validated provider"),
    }
}

async fn test_metadata(
    State(state): State<Arc<AppState>>,
    Path(provider): Path<String>,
    Json(payload): Json<MetadataTestRequest>,
) -> Result<Json<TestResponse>, (StatusCode, String)> {
    let provider = provider_name(&provider)?;
    let key = match payload.api_key {
        Some(key) => {
            validate_key(&key)?;
            key
        }
        None => effective_key(&state, provider)?.0,
    };
    validate_key(&key)?;
    probe_metadata(&state, provider, &key).await?;
    Ok(Json(TestResponse {
        success: true,
        message: format!("{} connection successful", provider.to_uppercase()),
    }))
}

async fn probe_metadata(
    state: &AppState,
    provider: &str,
    key: &str,
) -> Result<(), (StatusCode, String)> {
    let request = match provider {
        "tmdb" => state
            .integration_probe_client
            .get(TMDB_TEST_URL)
            .query(&[("api_key", key)]),
        "tvdb" => state
            .integration_probe_client
            .post(TVDB_TEST_URL)
            .json(&serde_json::json!({"apikey": key})),
        _ => unreachable!("validated provider"),
    };
    let response = request.send().await.map_err(|_| {
        (
            StatusCode::BAD_GATEWAY,
            "metadata provider unavailable".into(),
        )
    })?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err((StatusCode::BAD_GATEWAY, "metadata key rejected".into()))
    }
}

pub fn runtime_key(db: &crate::db::Db, provider: &str) -> String {
    let fallback = match provider {
        "tmdb" => std::env::var("TMDB_API_KEY").unwrap_or_default(),
        "tvdb" => std::env::var("TVDB_API_KEY").unwrap_or_default(),
        _ => String::new(),
    };
    startup_key(db, provider, fallback).unwrap_or_default()
}

pub fn startup_key(db: &crate::db::Db, provider: &str, fallback: String) -> anyhow::Result<String> {
    let Some(encrypted) = db.get_setting(&setting_key(provider))? else {
        return Ok(fallback);
    };
    registry()
        .map_err(|(_, message)| anyhow::anyhow!(message))?
        .decrypt_secret(&scope(provider), &encrypted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_allowlist_and_key_validation_are_strict() {
        assert_eq!(provider_name("tmdb").unwrap(), "tmdb");
        assert_eq!(provider_name("tvdb").unwrap(), "tvdb");
        assert!(provider_name("other").is_err());
        for key in ["", " key", "key ", "line\nfeed"] {
            assert!(validate_key(key).is_err(), "accepted {key:?}");
        }
        assert!(validate_key("valid-API_key.123").is_ok());
    }

    #[test]
    fn masking_never_exposes_full_key() {
        assert_eq!(mask_key(""), "");
        assert_eq!(mask_key("abc"), "••••");
        assert_eq!(mask_key("abcdefghijkl"), "••••••••ijkl");
        assert_eq!(mask_key("aéabc"), "••••••••éabc");
        assert!(!mask_key("abcdefghijkl").contains("abcdefgh"));
    }
}
