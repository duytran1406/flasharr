//! FshareHandler - Handler for Fshare.vn

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::Value;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;
use super::base::{HostHandler, FileInfo, ResolvedUrl, AccountStatus};
use crate::config::FshareConfig;
use crate::db::Db;

/// Fshare API Response
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct FshareApiResponse {
    code: i32,
    msg: String,
    session_id: Option<String>,
    token: Option<String>,
}

#[derive(Clone)]
struct SessionData {
    session_id: String,
    token: String,
}

/// Fshare host handler
pub struct FshareHandler {
    config: FshareConfig,
    client: Arc<Client>,
    session_data: Arc<Mutex<Option<SessionData>>>,
    db: Option<Arc<Db>>,
}

impl FshareHandler {
    /// Create new FshareHandler with shared HTTP client for connection pooling
    pub fn new(config: FshareConfig, client: Arc<Client>) -> Self {
        Self {
            config,
            client,
            session_data: Arc::new(Mutex::new(None)),
            db: None,
        }
    }

    /// Set database reference for dynamic credential fetching
    pub fn with_db(mut self, db: Arc<Db>) -> Self {
        self.db = Some(db);
        self
    }

    /// Get current credentials from database or fallback to config
    fn get_credentials(&self) -> (String, String) {
        // Try database first (where setup wizard saves credentials)
        if let Some(ref db) = self.db {
            if let Ok(Some(email)) = db.get_setting("fshare_email") {
                if let Ok(Some(password)) = db.get_setting("fshare_password") {
                    if !email.is_empty() && !password.is_empty() {
                        return (email, password);
                    }
                }
            }
        }
        
        // Fallback to config for backwards compatibility
        (self.config.email.clone(), self.config.password.clone())
    }

    async fn ensure_login(&self) -> anyhow::Result<SessionData> {
        // Check if we already have a session
        {
            let session = self.session_data.lock().await;
            if let Some(ref data) = *session {
                return Ok(data.clone());
            }
        }
        // Session lock dropped here before any network calls

        let (email, password) = self.get_credentials();
        
        if email.is_empty() || password.is_empty() {
            return Err(anyhow::anyhow!("FShare credentials not configured"));
        }

        tracing::info!("Logging into Fshare for {}", email);
        
        let resp = self.client.post("https://download.fsharegroup.site/api/user/login")
            .json(&serde_json::json!({
                "user_email": email,
                "password": password,
            }))
            .send()
            .await?;

        let data: Value = resp.json().await?;
        if data["code"] != 200 {
            return Err(anyhow::anyhow!("Fshare login failed: {}", data["msg"]));
        }

        let session_id = data["session_id"].as_str().ok_or_else(|| anyhow::anyhow!("No session_id in response"))?.to_string();
        let token = data["token"].as_str().ok_or_else(|| anyhow::anyhow!("No token in response"))?.to_string();

        let new_session = SessionData { session_id, token };
        
        // Store the session
        {
            let mut session = self.session_data.lock().await;
            *session = Some(new_session.clone());
        }

        Ok(new_session)
    }
}

#[async_trait]
impl HostHandler for FshareHandler {
    fn get_host_name(&self) -> &str {
        "fshare"
    }
    
    fn can_handle(&self, url: &str) -> bool {
        url.contains("fshare.vn")
    }
    
    async fn get_file_info(&self, url: &str) -> anyhow::Result<FileInfo> {
        // Extract file code from URL
        let fcode = url.split("/file/").last().unwrap_or("").split("?").next().unwrap_or("");
        if fcode.is_empty() {
            return Err(anyhow::anyhow!("Invalid Fshare URL"));
        }

        tracing::info!("[FSHARE] get_file_info using V3 API for fcode: {}", fcode);
        
        // V3 API (internal web API) - doesn't require authentication
        let resp = self.client.get("https://www.fshare.vn/api/v3/files/folder")
            .query(&[("linkcode", fcode)])
            .header("Accept", "application/json, text/plain, */*")
            .send()
            .await?;

        let status = resp.status();
        tracing::info!("[FSHARE] V3 API response status: {}", status);

        if status != reqwest::StatusCode::OK {
            return Err(anyhow::anyhow!("V3 API returned status {}", status));
        }

        let data: Value = resp.json().await?;
        tracing::info!("[FSHARE] V3 API response: {}", serde_json::to_string(&data).unwrap_or_default());
        
        // Check for 404 status in response
        if data["status"].as_i64() == Some(404) {
            return Err(anyhow::anyhow!("File not found (404)"));
        }

        let current = &data["current"];
        let filename = current["name"].as_str().unwrap_or(fcode).to_string();
        let size = current["size"].as_u64()
            .or_else(|| current["size"].as_str().and_then(|s| s.parse().ok()))
            .unwrap_or(0);

        tracing::info!("[FSHARE] V3 file info: name='{}', size={}", filename, size);

        Ok(FileInfo {
            filename,
            size,
            original_url: url.to_string(),
        })
    }
    
    async fn check_account_status(&self) -> anyhow::Result<AccountStatus> {
        let session = self.ensure_login().await?;
        let (email, _) = self.get_credentials();
        
        let resp = self.client.post("https://download.fsharegroup.site/api/user/get")
            .json(&serde_json::json!({
                "session_id": session.session_id,
            }))
            .send()
            .await?;

        let data: Value = resp.json().await?;
        
        let email = data["email"].as_str().unwrap_or(&email).to_string();
        let account_type = data["account_type"].as_str().unwrap_or("Free");
        let is_premium = account_type.to_lowercase() == "vip" || account_type.to_lowercase() == "premium";
        
        Ok(AccountStatus {
            can_download: true,
            reason: None,
            account_email: email,
            premium: is_premium,
            traffic_left: Some(data["traffic_used"].to_string()),
        })
    }
    
    async fn resolve_download_url(&self, url: &str) -> anyhow::Result<ResolvedUrl> {
        tracing::info!("=== [FSHARE] resolve_download_url called ===");
        tracing::info!("[FSHARE] URL to resolve: {}", url);
        
        // Check if this is already a direct download URL (premium link from previous resolution)
        // Direct links look like: https://xxx.fshare.vn/dl/... or contain download tokens
        if url.contains("/dl/") || url.contains("download.fshare") {
            tracing::info!("[FSHARE] URL appears to be a direct download link, validating...");
            
            // Test if the existing link is still valid with HEAD request
            match self.client.head(url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    tracing::info!("[FSHARE] Existing premium link is still valid!");
                    return Ok(ResolvedUrl {
                        direct_url: url.to_string(),
                        headers: std::collections::HashMap::new(),
                        expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)), // Conservative estimate
                    });
                }
                Ok(resp) => {
                    tracing::info!("[FSHARE] Existing premium link expired (status: {}), will re-resolve", resp.status());
                    // Extract original fshare.vn URL from task if possible, or try to fetch fresh
                }
                Err(e) => {
                    tracing::warn!("[FSHARE] Failed to validate existing link: {}", e);
                }
            }
            
            // If the direct link is invalid, we need the original fshare.vn URL to re-resolve
            // For now, return an error - the caller should use original_url for re-resolution
            return Err(anyhow::anyhow!("Premium link expired, use original_url to re-resolve"));
        }
        
        // Standard FShare URL resolution via /int/get
        let session = self.ensure_login().await?;
        tracing::info!("[FSHARE] Login successful, session_id: {}...", &session.session_id[..20.min(session.session_id.len())]);

        // Build request payload
        let request_payload = serde_json::json!({
            "session_id": session.session_id,
            "token": session.token,
            "url": url,
            "password": "",
        });
        tracing::info!("[FSHARE] Request payload (sanitized): url={}, password=''", url);

        let resp = self.client.post("https://download.fsharegroup.site/int/get")
            .json(&request_payload)
            .send()
            .await?;
        
        let resp_status = resp.status();
        tracing::info!("[FSHARE] API response status: {}", resp_status);

        let data: Value = resp.json().await?;
        
        // Debug: Log the full response
        tracing::info!("[FSHARE] Full API response: {}", serde_json::to_string(&data).unwrap_or_else(|_| format!("{:?}", data)));
        
        // Check for errors in response
        if let Some(error) = data["error"].as_str() {
            tracing::error!("[FSHARE] Error in response: {}", error);
            return Err(anyhow::anyhow!("Fshare error: {}", error));
        }
        
        // Check for error code
        if let Some(code) = data["code"].as_i64() {
            if code != 200 {
                let msg = data["msg"].as_str().unwrap_or("Unknown error");
                tracing::error!("[FSHARE] Error code {} in response: {}", code, msg);
                return Err(anyhow::anyhow!("Fshare error (code {}): {}", code, msg));
            }
        }

        let direct_url = data["location"].as_str()
            .or_else(|| data["url"].as_str())
            .ok_or_else(|| {
                tracing::error!("[FSHARE] No download URL found in response keys. Available keys: {:?}", 
                    data.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                anyhow::anyhow!("No download URL in response. Response: {}", serde_json::to_string(&data).unwrap_or_default())
            })?
            .to_string();

        tracing::info!("[FSHARE] Resolved direct URL: {}...", &direct_url[..50.min(direct_url.len())]);

        Ok(ResolvedUrl {
            direct_url,
            headers: std::collections::HashMap::new(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(6)),
        })
    }
    
    async fn validate_download_url(&self, url: &str) -> anyhow::Result<bool> {
        let resp = self.client.head(url).send().await?;
        Ok(resp.status().is_success())
    }
    
    async fn refresh_download_url(&self, original_url: &str) -> anyhow::Result<ResolvedUrl> {
        self.resolve_download_url(original_url).await
    }
    
    fn supports_resume(&self) -> bool {
        true
    }
    
    fn supports_multi_segment(&self) -> bool {
        true
    }
    
    fn get_max_segments(&self) -> u32 {
        8
    }

    async fn logout(&self) -> anyhow::Result<()> {
        let mut session = self.session_data.lock().await;
        *session = None;
        tracing::info!("[FSHARE] Logged out and cleared session data in memory");
        Ok(())
    }
}
