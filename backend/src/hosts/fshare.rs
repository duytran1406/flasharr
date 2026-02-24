//! FshareHandler - Handler for Fshare.vn
//!
//! Supports three authentication modes with automatic fallback:
//! 1. API login via download.fsharegroup.site (primary, currently down)
//! 2. API login via api.fshare.vn with app_key (secondary, fast & reliable)
//! 3. Web form login via www.fshare.vn/site/login (tertiary, slowest but always works)

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use reqwest::Client;
use std::sync::Arc;
use tokio::sync::Mutex;
use super::base::{HostHandler, FileInfo, ResolvedUrl, AccountStatus};
use crate::config::FshareConfig;
use crate::db::Db;

/// FShare app_key for api.fshare.vn (discovered from open-source projects)
/// Source: https://github.com/tudoanh/get_fshare
const FSHARE_APP_KEY: &str = "L2S7R6ZMagggC5wWkQhX2+aDi467PPuftWUMRFSn";


/// Fshare API Response
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct FshareApiResponse {
    code: i32,
    msg: String,
    session_id: Option<String>,
    token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SessionData {
    session_id: String,
    token: String,
    created_at: DateTime<Utc>,
    last_validated: DateTime<Utc>,
}

/// Web-based session data (cookie + CSRF token)
#[derive(Clone, Debug, Serialize, Deserialize)]
struct WebSessionData {
    /// Serialised cookie header value, e.g. "fshare_app=abc123; session-id=xyz789"
    cookies: String,
    /// CSRF token extracted from the logged-in page
    csrf_token: String,
    created_at: DateTime<Utc>,
    last_validated: DateTime<Utc>,
}

impl WebSessionData {
    const VALIDATION_INTERVAL_MINUTES: i64 = 10;

    fn needs_validation(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.last_validated);
        age > chrono::Duration::minutes(Self::VALIDATION_INTERVAL_MINUTES)
    }
}

impl SessionData {
    /// How often to validate session (5 minutes)
    const VALIDATION_INTERVAL_MINUTES: i64 = 5;
    
    /// Check if session needs validation
    fn needs_validation(&self) -> bool {
        let age = Utc::now().signed_duration_since(self.last_validated);
        age > chrono::Duration::minutes(Self::VALIDATION_INTERVAL_MINUTES)
    }
    
    /// Get time since last validation in seconds
    fn seconds_since_validation(&self) -> i64 {
        Utc::now().signed_duration_since(self.last_validated).num_seconds()
    }
}

/// Login rate limiter to prevent ban
struct LoginRateLimiter {
    last_login_attempt: Mutex<Option<DateTime<Utc>>>,
    consecutive_failures: Mutex<u32>,
}

impl LoginRateLimiter {
    fn new() -> Self {
        Self {
            last_login_attempt: Mutex::new(None),
            consecutive_failures: Mutex::new(0),
        }
    }
    
    /// Check if we can login now, or how long to wait
    async fn can_login(&self) -> Result<(), std::time::Duration> {
        let last_attempt = self.last_login_attempt.lock().await;
        let failures = *self.consecutive_failures.lock().await;
        
        if let Some(last) = *last_attempt {
            let elapsed = Utc::now().signed_duration_since(last);
            
            // Exponential backoff based on consecutive failures
            let required_wait_secs = match failures {
                0 => 0,
                1 => 5,
                2 => 10,
                3 => 30,
                _ => 60,
            };
            
            let required_wait = chrono::Duration::seconds(required_wait_secs);
            
            if elapsed < required_wait {
                let remaining = required_wait - elapsed;
                return Err(std::time::Duration::from_secs(remaining.num_seconds() as u64));
            }
        }
        
        Ok(())
    }
    
    /// Record login attempt
    async fn record_attempt(&self) {
        let mut last_attempt = self.last_login_attempt.lock().await;
        *last_attempt = Some(Utc::now());
    }
    
    /// Record login success (reset failure count)
    async fn record_success(&self) {
        let mut failures = self.consecutive_failures.lock().await;
        *failures = 0;
    }
    
    /// Record login failure (increment failure count)
    async fn record_failure(&self) {
        let mut failures = self.consecutive_failures.lock().await;
        *failures += 1;
        tracing::warn!("[FSHARE] Login failed, consecutive failures: {}", *failures);
    }
}

/// Fshare host handler
pub struct FshareHandler {
    config: FshareConfig,
    client: Arc<Client>,
    /// Dedicated client for api.fshare.vn (mobile API with okhttp UA)
    api2_client: Arc<Client>,
    /// A **non-redirect-following** client for the web login flow.
    web_client: Arc<Client>,
    session_data: Arc<Mutex<Option<SessionData>>>,
    web_session: Arc<Mutex<Option<WebSessionData>>>,
    db: Option<Arc<Db>>,
    login_mutex: Arc<Mutex<()>>,
    rate_limiter: Arc<LoginRateLimiter>,
    circuit_breaker: Arc<super::circuit_breaker::CircuitBreaker>,
}

impl FshareHandler {
    /// Create new FshareHandler with shared HTTP client for connection pooling
    pub fn new(config: FshareConfig, client: Arc<Client>) -> Self {
        // Dedicated client for api.fshare.vn - mobile API that requires okhttp UA
        // The shared client has Chrome UA via .user_agent() which can't be overridden
        let api2_client = Arc::new(
            Client::builder()
                .user_agent("okhttp/3.6.0")
                .redirect(reqwest::redirect::Policy::none())
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
        );
        // Build a non-redirect-following client for web login (we need to inspect 302s)
        // NOTE: cookie_store(true) MUST NOT be used here — it causes reqwest's internal
        // cookie jar to inject its own Cookie header on top of our manually-set Cookie
        // headers, producing duplicate/malformed cookies that Fshare rejects with 400.
        let web_client = Arc::new(
            Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .cookie_store(false)
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .unwrap_or_else(|_| Client::new()),
        );
        Self {
            config,
            client,
            api2_client,
            web_client,
            session_data: Arc::new(Mutex::new(None)),
            web_session: Arc::new(Mutex::new(None)),
            db: None,
            login_mutex: Arc::new(Mutex::new(())),
            rate_limiter: Arc::new(LoginRateLimiter::new()),
            circuit_breaker: Arc::new(super::circuit_breaker::CircuitBreaker::new()),
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

    /// Check if we have a valid session (for health checks)
    pub async fn has_valid_session(&self) -> bool {
        let session = self.session_data.lock().await;
        session.as_ref().map(|s| !s.needs_validation()).unwrap_or(false)
    }


    /// Main entry point: Ensure we have a valid session
    /// CORE PRINCIPLE: NEVER LOGIN IF SESSION STILL GOOD
    async fn ensure_valid_session(&self) -> anyhow::Result<SessionData> {
        // Step 1: Check in-memory cache for valid session
        {
            let session = self.session_data.lock().await;
            if let Some(ref data) = *session {
                if !data.needs_validation() {
                    tracing::debug!(
                        "[FSHARE] Using cached session (validated {}s ago)",
                        data.seconds_since_validation()
                    );
                    return Ok(data.clone());
                }
                
                tracing::debug!(
                    "[FSHARE] Session needs validation (last validated {}s ago)",
                    data.seconds_since_validation()
                );
            }
        }
        
        // Step 2: If no session or needs validation, enter protected flow
        // perform_login_protected handles the mutex, DB checks, and actual login
        self.perform_login_protected().await
    }
    
    /// Validate session using /api/user/get
    async fn validate_session(&self, session: &SessionData) -> anyhow::Result<()> {
        tracing::debug!("[FSHARE] Validating session...");
        
        // Try api.fshare.vn first (faster and more reliable)
        let api2_result = self.api2_client
            .get("https://api.fshare.vn/api/user/get")
            .header("Cookie", format!("session_id={}", session.session_id))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
        
        match api2_result {
            Ok(resp) if resp.status() == 200 => {
                tracing::debug!("[FSHARE-API2] Session validation successful");
                return Ok(());
            }
            Ok(resp) => {
                tracing::debug!("[FSHARE-API2] Session invalid (status: {}), trying primary API", resp.status());
            }
            Err(e) => {
                tracing::debug!("[FSHARE-API2] Validation failed ({}), trying primary API", e);
            }
        }
        
        // Fallback to primary API
        let resp = self.client
            .post("https://download.fsharegroup.site/api/user/get")
            .json(&serde_json::json!({
                "session_id": session.session_id,
            }))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Session validation request failed: {}", e))?;
        
        let status = resp.status();
        let data: Value = resp.json().await?;
        
        if status != 200 || data["code"] != 200 {
            return Err(anyhow::anyhow!("Session invalid (status: {}, code: {})", status, data["code"]));
        }
        
        tracing::debug!("[FSHARE] Session validation successful");
        Ok(())
    }
    
    /// Perform login with concurrent protection and rate limiting
    async fn perform_login_protected(&self) -> anyhow::Result<SessionData> {
        // Step 1: Check rate limiter
        match self.rate_limiter.can_login().await {
            Ok(_) => {}
            Err(wait_duration) => {
                tracing::warn!("[FSHARE] Rate limited, waiting {}s before login...", wait_duration.as_secs());
                tokio::time::sleep(wait_duration).await;
            }
        }
        
        // Step 2: Acquire login mutex
        let _guard = self.login_mutex.lock().await;
        
        // Step 3: Double-check - another thread may have logged in or validated while we waited
        {
            let session = self.session_data.lock().await;
            if let Some(ref data) = *session {
                if !data.needs_validation() {
                    tracing::info!("[FSHARE] Another thread successfully validated or established session");
                    return Ok(data.clone());
                }
                
                // If it needs validation, let's try it once here within the lock
                tracing::info!("[FSHARE] Double-checking session validity inside lock...");
                let session_to_validate = data.clone();
                drop(session);
                
                if self.validate_session(&session_to_validate).await.is_ok() {
                    tracing::info!("[FSHARE] Session is still valid (checked inside lock), reusing");
                    let mut session = self.session_data.lock().await;
                    if let Some(ref mut s) = *session {
                        s.last_validated = Utc::now();
                    }
                    return Ok(session_to_validate);
                }
                
                // Validation failed, clear it
                let mut session = self.session_data.lock().await;
                *session = None;
            }
        }

        // Step 4: Try loading from database if memory was empty or invalid
        if let Some(ref db) = self.db {
            if let Ok(Some(saved_session)) = self.load_session_from_db(db).await {
                tracing::info!("[FSHARE] Validating session from database inside lock...");
                match self.validate_session(&saved_session).await {
                    Ok(_) => {
                        let mut session = self.session_data.lock().await;
                        let mut validated_session = saved_session.clone();
                        validated_session.last_validated = Utc::now();
                        *session = Some(validated_session.clone());
                        tracing::info!("[FSHARE] Successfully restored and validated session from database");
                        return Ok(validated_session);
                    }
                    Err(e) => {
                        tracing::warn!("[FSHARE] Database session invalid: {}, cleaning up", e);
                        let _ = self.delete_session_from_db(db).await;
                    }
                }
            }
        }
        
        // Step 5: Perform actual login
        tracing::info!("[FSHARE] No valid session found in memory or database, performing fresh login");
        self.perform_login().await
    }
    
    /// Perform actual login — tries API first, falls back to web form login
    async fn perform_login(&self) -> anyhow::Result<SessionData> {
        let (email, password) = self.get_credentials();
        
        if email.is_empty() || password.is_empty() {
            return Err(anyhow::anyhow!("FShare credentials not configured"));
        }
        
        tracing::info!("[FSHARE] Logging into Fshare for {}", email);
        
        self.rate_limiter.record_attempt().await;
        
        // Check if we should skip primary API and go straight to api.fshare.vn
        if !self.config.prefer_api2 {
            // Tier 1: Try primary API (download.fsharegroup.site)
            match self.perform_api_login(&email, &password).await {
                Ok(session) => return Ok(session),
                Err(e) => {
                    let err_str = format!("{}", e);
                    // Only fall back on connectivity errors, not auth failures
                    if err_str.contains("timed out") || err_str.contains("ECONNREFUSED") 
                        || err_str.contains("connect") || err_str.contains("dns")
                        || err_str.contains("request failed") {
                        tracing::warn!("[FSHARE] Primary API unreachable ({}), trying api.fshare.vn", err_str);
                    } else {
                        // Auth failure or other API error — don't try fallbacks
                        return Err(e);
                    }
                }
            }
        } else {
            tracing::info!("[FSHARE] Skipping primary API (prefer_api2=true), using api.fshare.vn");
        }
        
        // Tier 2: Try secondary API (api.fshare.vn with app_key)
        match self.perform_api2_login(&email, &password).await {
            Ok(session) => return Ok(session),
            Err(e) => {
                // Always fall through to web login - api2 may reject certain accounts
                tracing::warn!("[FSHARE] Secondary API login failed ({}), falling back to web login", e);
            }
        }
        
        // Tier 3: Fallback to web form login
        match self.perform_web_login(&email, &password).await {
            Ok(web_session) => {
                // Store web session
                {
                    let mut ws = self.web_session.lock().await;
                    *ws = Some(web_session.clone());
                }
                if let Some(ref db) = self.db {
                    let json = serde_json::to_string(&web_session).unwrap_or_default();
                    let _ = db.save_setting("fshare_web_session", &json);
                }
                
                // Create a synthetic SessionData so callers can proceed
                let now = Utc::now();
                let synthetic = SessionData {
                    session_id: "web-session".to_string(),
                    token: "web-token".to_string(),
                    created_at: now,
                    last_validated: now,
                };
                let mut session = self.session_data.lock().await;
                *session = Some(synthetic.clone());
                self.rate_limiter.record_success().await;
                Ok(synthetic)
            }
            Err(web_err) => {
                self.rate_limiter.record_failure().await;
                Err(anyhow::anyhow!("All login methods failed. Primary API: unreachable, Secondary API: unreachable, Web: {}", web_err))
            }
        }
    }
    
    /// API login via download.fsharegroup.site
    async fn perform_api_login(&self, email: &str, password: &str) -> anyhow::Result<SessionData> {
        let resp = self.client
            .post("https://download.fsharegroup.site/api/user/login")
            .json(&serde_json::json!({
                "user_email": email,
                "password": password,
            }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("Login request failed: {}", e))?;
        
        let data: Value = resp.json().await?;
        
        if data["code"] != 200 {
            self.rate_limiter.record_failure().await;
            return Err(anyhow::anyhow!("Fshare login failed: {}", data["msg"]));
        }
        
        let session_id = data["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No session_id in response"))?
            .to_string();
        let token = data["token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No token in response"))?
            .to_string();
        
        let now = Utc::now();
        let new_session = SessionData {
            session_id,
            token,
            created_at: now,
            last_validated: now,
        };
        
        self.rate_limiter.record_success().await;
        
        // Store in memory
        {
            let mut session = self.session_data.lock().await;
            *session = Some(new_session.clone());
        }
        
        // Store in database
        if let Some(ref db) = self.db {
            if let Err(e) = self.save_session_to_db(db, &new_session).await {
                tracing::warn!("[FSHARE] Failed to save session to database: {}", e);
            } else {
                tracing::debug!("[FSHARE] Session saved to database");
            }
            // Cache VIP rank from login response so list_accounts is instant
            let account_type = data["account_type"].as_str().unwrap_or("free");
            let rank = if account_type.to_lowercase().contains("vip")
                || account_type.to_lowercase().contains("premium") {
                "VIP"
            } else {
                "FREE"
            };
            
            let valid_until = data["expire_vip"]
                .as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .or_else(|| data["expire_vip"].as_u64())
                .unwrap_or(0);
                
            let _ = db.save_setting("fshare_rank", rank);
            let _ = db.save_setting("fshare_valid_until", &valid_until.to_string());
            tracing::debug!("[FSHARE] Cached rank '{}' and valid_until '{}' to DB", rank, valid_until);
        }
        
        tracing::info!("[FSHARE] API login successful");
        Ok(new_session)
    }
    
    /// API login via api.fshare.vn (secondary fallback)
    /// Uses app_key authentication (tested and working)
    async fn perform_api2_login(&self, email: &str, password: &str) -> anyhow::Result<SessionData> {
        tracing::info!("[FSHARE-API2] Attempting login via api.fshare.vn/api/user/login");
        
        let resp = self.api2_client
            .post("https://api.fshare.vn/api/user/login")
            .json(&serde_json::json!({
                "app_key": FSHARE_APP_KEY,
                "user_email": email,
                "password": password,
            }))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("API2 login request failed: {}", e))?;
        
        let status = resp.status();
        tracing::info!("[FSHARE-API2] Login response status: {}", status);
        
        // Handle redirect: FShare CDN sometimes redirects — follow manually with POST
        let resp = if status == reqwest::StatusCode::MOVED_PERMANENTLY || status == reqwest::StatusCode::FOUND || status == reqwest::StatusCode::TEMPORARY_REDIRECT {
            if let Some(location) = resp.headers().get("location").and_then(|v| v.to_str().ok()) {
                tracing::info!("[FSHARE-API2] Following redirect to: {}", location);
                self.api2_client
                    .post(location)
                    .json(&serde_json::json!({
                        "app_key": FSHARE_APP_KEY,
                        "user_email": email,
                        "password": password,
                    }))
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("API2 login redirect failed: {}", e))?
            } else {
                tracing::warn!("[FSHARE-API2] Redirect without Location header");
                resp
            }
        } else {
            resp
        };
        
        let status = resp.status();
        tracing::info!("[FSHARE-API2] Final response status: {}", status);
        
        let body_text = resp.text().await
            .map_err(|e| anyhow::anyhow!("API2 login: failed to read response body: {}", e))?;
        
        tracing::info!("[FSHARE-API2] Login response body: {}", &body_text[..body_text.len().min(500)]);
        
        let data: Value = serde_json::from_str(&body_text)
            .map_err(|e| anyhow::anyhow!("API2 login: invalid JSON response ({}): {}", e, &body_text[..body_text.len().min(200)]))?;
        
        if data["code"] != 200 {
            self.rate_limiter.record_failure().await;
            return Err(anyhow::anyhow!("FShare API2 login failed: {}", data["msg"]));
        }
        
        let session_id = data["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No session_id in API2 response"))?
            .to_string();
        let token = data["token"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No token in API2 response"))?
            .to_string();
        
        let now = Utc::now();
        let new_session = SessionData {
            session_id,
            token,
            created_at: now,
            last_validated: now,
        };
        
        self.rate_limiter.record_success().await;
        
        // Store in memory
        {
            let mut session = self.session_data.lock().await;
            *session = Some(new_session.clone());
        }
        
        // Store in database
        if let Some(ref db) = self.db {
            if let Err(e) = self.save_session_to_db(db, &new_session).await {
                tracing::warn!("[FSHARE-API2] Failed to save session to database: {}", e);
            } else {
                tracing::debug!("[FSHARE-API2] Session saved to database");
            }
            
            // Fetch and cache rank immediately since API2 login response doesn't include account type
            if let Ok(status) = self.fetch_account_status_for_session(&new_session).await {
                let rank = if status.premium { "VIP" } else { "FREE" };
                let _ = db.save_setting("fshare_rank", rank);
                let valid_until = status.valid_until.unwrap_or(0);
                let _ = db.save_setting("fshare_valid_until", &valid_until.to_string());
                tracing::info!("[FSHARE-API2] Verified account rank: {}, valid_until: {}", rank, valid_until);
            }
        }
        
        tracing::info!("[FSHARE-API2] Login successful via api.fshare.vn");
        Ok(new_session)
    }
    
    
    // ── Web Form Login Fallback ──────────────────────────────────────────────
    
    /// Extract CSRF token from HTML: <meta name="csrf-token" content="...">
    fn extract_csrf_token(html: &str) -> Option<String> {
        // Look for <meta name="csrf-token" content="VALUE">
        let marker = "name=\"csrf-token\" content=\"";
        if let Some(pos) = html.find(marker) {
            let start = pos + marker.len();
            if let Some(end) = html[start..].find('"') {
                return Some(html[start..start + end].to_string());
            }
        }
        None
    }
    
    /// Extract Set-Cookie values from response headers into a cookie string
    fn extract_cookies(resp: &reqwest::Response) -> String {
        resp.headers()
            .get_all(reqwest::header::SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .filter_map(|s| s.split(';').next().map(|c| c.trim().to_string()))
            .collect::<Vec<_>>()
            .join("; ")
    }
    
    /// Merge new cookies into existing cookie string (overwrite duplicates)
    fn merge_cookies(existing: &str, new_cookies: &str) -> String {
        let mut map = std::collections::HashMap::new();
        for pair in existing.split(';').chain(new_cookies.split(';')) {
            let pair = pair.trim();
            if let Some(eq) = pair.find('=') {
                let key = pair[..eq].trim().to_string();
                map.insert(key, pair.to_string());
            }
        }
        map.values().cloned().collect::<Vec<_>>().join("; ")
    }
    
    /// Attempt web form login via www.fshare.vn/site/login
    async fn perform_web_login(&self, email: &str, password: &str) -> anyhow::Result<WebSessionData> {
        tracing::info!("[FSHARE-WEB] Starting web form login for {}", email);
        
        // Step 1: GET login page to obtain CSRF token and initial cookies
        let login_page_resp = self.web_client
            .get("https://www.fshare.vn/site/login")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("[FSHARE-WEB] Failed to load login page: {}", e))?;
        
        let initial_cookies = Self::extract_cookies(&login_page_resp);
        let html = login_page_resp.text().await?;
        
        let csrf_token = Self::extract_csrf_token(&html)
            .ok_or_else(|| anyhow::anyhow!("[FSHARE-WEB] Could not extract CSRF token from login page"))?;
        
        tracing::debug!("[FSHARE-WEB] Got CSRF token: {}...", &csrf_token[..20.min(csrf_token.len())]);
        
        // Step 2: POST form login
        let form_body = format!(
            "_csrf-app={}&LoginForm%5Bemail%5D={}&LoginForm%5Bpassword%5D={}&LoginForm%5BrememberMe%5D=1",
            urlencoding::encode(&csrf_token),
            urlencoding::encode(email),
            urlencoding::encode(password),
        );
        
        let login_resp = self.web_client
            .post("https://www.fshare.vn/site/login")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Referer", "https://www.fshare.vn/site/login")
            .header("Cookie", &initial_cookies)
            .body(form_body)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("[FSHARE-WEB] Login POST failed: {}", e))?;
        
        let status = login_resp.status();
        let login_cookies = Self::extract_cookies(&login_resp);
        let all_cookies = Self::merge_cookies(&initial_cookies, &login_cookies);
        
        tracing::info!("[FSHARE-WEB] Login POST status: {} | Cookies: {}", status, 
            if all_cookies.len() > 60 { format!("{}...", &all_cookies[..60]) } else { all_cookies.clone() });
        
        // 302 redirect = success (redirecting to home page)
        // 200 = failure (re-rendered login form with errors)
        if status == reqwest::StatusCode::FOUND || status == reqwest::StatusCode::MOVED_PERMANENTLY {
            tracing::info!("[FSHARE-WEB] Web login successful (302 redirect)");
        } else if status == reqwest::StatusCode::OK {
            // Check if we got redirected back to login (failed) or actually succeeded
            let body = login_resp.text().await.unwrap_or_default();
            if body.contains("LoginForm[email]") || body.contains("form-signup") {
                return Err(anyhow::anyhow!("[FSHARE-WEB] Login failed — invalid credentials"));
            }
            tracing::info!("[FSHARE-WEB] Web login appears successful (200 with no login form)");
        } else {
            return Err(anyhow::anyhow!("[FSHARE-WEB] Unexpected login response status: {}", status));
        }
        
        // Step 3: Validate the session by checking /account/profile
        let profile_resp = self.web_client
            .get("https://www.fshare.vn/account/profile")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Cookie", &all_cookies)
            .send()
            .await;
        
        match profile_resp {
            Ok(resp) => {
                let profile_status = resp.status();
                // Merge any new cookies from profile response
                let profile_cookies = Self::extract_cookies(&resp);
                let final_cookies = Self::merge_cookies(&all_cookies, &profile_cookies);
                
                if profile_status == reqwest::StatusCode::OK {
                    tracing::info!("[FSHARE-WEB] Session confirmed via /account/profile (200)");
                    
                    // Try to extract a fresh CSRF token from the profile page
                    let profile_html = resp.text().await.unwrap_or_default();
                    let final_csrf = Self::extract_csrf_token(&profile_html)
                        .unwrap_or(csrf_token);
                    
                    let now = Utc::now();
                    return Ok(WebSessionData {
                        cookies: final_cookies,
                        csrf_token: final_csrf,
                        created_at: now,
                        last_validated: now,
                    });
                } else if profile_status == reqwest::StatusCode::FOUND {
                    return Err(anyhow::anyhow!("[FSHARE-WEB] Session invalid — /account/profile redirected to login"));
                }
            }
            Err(e) => {
                tracing::warn!("[FSHARE-WEB] Could not validate session via /account/profile: {}", e);
            }
        }
        
        // If profile check didn't confirm, still return the session (optimistic)
        let now = Utc::now();
        Ok(WebSessionData {
            cookies: all_cookies,
            csrf_token,
            created_at: now,
            last_validated: now,
        })
    }
    
    /// Validate an existing web session
    async fn validate_web_session(&self, session: &WebSessionData) -> anyhow::Result<()> {
        let resp = self.web_client
            .get("https://www.fshare.vn/account/profile")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .header("Cookie", &session.cookies)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("[FSHARE-WEB] Session validation failed: {}", e))?;
        
        if resp.status() == reqwest::StatusCode::OK {
            Ok(())
        } else {
            Err(anyhow::anyhow!("[FSHARE-WEB] Session expired (status: {})", resp.status()))
        }
    }
    
    /// Ensure we have a valid web session
    async fn ensure_web_session(&self) -> anyhow::Result<WebSessionData> {
        // Check in-memory cache
        {
            let ws = self.web_session.lock().await;
            if let Some(ref data) = *ws {
                if !data.needs_validation() {
                    return Ok(data.clone());
                }
                
                // Validate
                let to_validate = data.clone();
                drop(ws);
                if self.validate_web_session(&to_validate).await.is_ok() {
                    let mut ws = self.web_session.lock().await;
                    if let Some(ref mut s) = *ws {
                        s.last_validated = Utc::now();
                    }
                    return Ok(to_validate);
                }
            }
        }
        
        // Try loading from database
        if let Some(ref db) = self.db {
            if let Ok(Some(json)) = db.get_setting("fshare_web_session") {
                if let Ok(saved) = serde_json::from_str::<WebSessionData>(&json) {
                    if self.validate_web_session(&saved).await.is_ok() {
                        let mut validated = saved.clone();
                        validated.last_validated = Utc::now();
                        let mut ws = self.web_session.lock().await;
                        *ws = Some(validated.clone());
                        tracing::info!("[FSHARE-WEB] Restored web session from database");
                        return Ok(validated);
                    }
                }
            }
        }
        
        // Perform fresh web login
        let (email, password) = self.get_credentials();
        let new_session = self.perform_web_login(&email, &password).await?;
        {
            let mut ws = self.web_session.lock().await;
            *ws = Some(new_session.clone());
        }
        if let Some(ref db) = self.db {
            let json = serde_json::to_string(&new_session).unwrap_or_default();
            let _ = db.save_setting("fshare_web_session", &json);
        }
        Ok(new_session)
    }
    
    /// Resolve download URL via the web interface.
    /// Real FShare form: `<form id="form-download" action="/download/get" method="post">`
    /// Fields: `_csrf-app`, `linkcode`, `ushare`, `withFcode5`, `password`, optionally `des`
    /// Response: 302 redirect to the direct download URL, or JSON with `location` key.
    async fn resolve_download_url_web(&self, url: &str) -> anyhow::Result<ResolvedUrl> {
        tracing::info!("[FSHARE-WEB] Resolving download URL via web for: {}", url);
        
        let web_session = self.ensure_web_session().await?;
        
        // Extract file code from URL (path segment after /file/)
        let path_part = url.split("/file/").last().unwrap_or("");
        let fcode = path_part.split('?').next().unwrap_or("");
        if fcode.is_empty() {
            return Err(anyhow::anyhow!("[FSHARE-WEB] Could not extract file code from URL"));
        }
        
        // Extract `des` token from query string — Fshare includes this as a session-bound
        // download token (e.g. ?des=71f5561d15). It must be included in the form POST.
        let des_token = url.split('?')
            .nth(1)
            .and_then(|qs| qs.split('&').find(|p| p.starts_with("des=")))
            .and_then(|p| p.strip_prefix("des="))
            .unwrap_or("");
        
        tracing::info!("[FSHARE-WEB] File code: {}, des: {}, POSTing form to /download/get", fcode, 
            if des_token.is_empty() { "(none)" } else { des_token });
        
        const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        
        // First, visit the file page to get a fresh CSRF token scoped to this file.
        // This is important — a CSRF token from the login page is NOT accepted by /download/get.
        let file_page_resp = self.web_client
            .get(url)
            .header("User-Agent", UA)
            .header("Cookie", &web_session.cookies)
            .send()
            .await;
        
        let (download_csrf, download_cookies) = match file_page_resp {
            Ok(resp) => {
                let page_cookies = Self::extract_cookies(&resp);
                let merged = Self::merge_cookies(&web_session.cookies, &page_cookies);
                let html = resp.text().await.unwrap_or_default();
                let csrf = Self::extract_csrf_token(&html).unwrap_or_else(|| web_session.csrf_token.clone());
                tracing::debug!("[FSHARE-WEB] Got file-page CSRF: {}...", &csrf[..20.min(csrf.len())]);
                (csrf, merged)
            }
            Err(e) => {
                tracing::warn!("[FSHARE-WEB] Could not fetch file page for fresh CSRF ({}), using session CSRF", e);
                (web_session.csrf_token.clone(), web_session.cookies.clone())
            }
        };
        
        // Build form-encoded POST body matching Fshare's actual HTML form.
        // `des` is a session/link token Fshare embeds in shared URLs — required to authorise
        // the download. `password` field must be present (empty string for public files).
        let mut form_body = format!(
            "_csrf-app={}&linkcode={}&ushare=&withFcode5=0&password=",
            urlencoding::encode(&download_csrf),
            urlencoding::encode(fcode),
        );
        if !des_token.is_empty() {
            form_body.push_str(&format!("&des={}", urlencoding::encode(des_token)));
        }
        
        tracing::debug!("[FSHARE-WEB] Form body (sanitised): linkcode={}, des={}", fcode, des_token);
        
        // POST to /download/get with no-redirect client so we can inspect Location header.
        // X-Requested-With marks this as an AJAX request — Fshare's backend now requires it
        // to return a JSON response rather than an HTML error page.
        let resp = self.web_client
            .post("https://www.fshare.vn/download/get")
            .header("User-Agent", UA)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Cookie", &download_cookies)
            .header("Referer", url)
            .header("Origin", "https://www.fshare.vn")
            .header("X-Requested-With", "XMLHttpRequest")
            .body(form_body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("[FSHARE-WEB] Download form POST failed: {}", e))?;
        
        let status = resp.status();
        tracing::info!("[FSHARE-WEB] Download POST status: {}", status);
        
        // Case 1: 302 redirect — Location header IS the direct download URL
        if status == reqwest::StatusCode::FOUND
            || status == reqwest::StatusCode::MOVED_PERMANENTLY
            || status == reqwest::StatusCode::TEMPORARY_REDIRECT
        {
            if let Some(location) = resp.headers().get(reqwest::header::LOCATION) {
                let direct_url = location.to_str()
                    .map_err(|_| anyhow::anyhow!("[FSHARE-WEB] Invalid Location header encoding"))?
                    .to_string();
                tracing::info!("[FSHARE-WEB] Got redirect to download URL: {}...", &direct_url[..50.min(direct_url.len())]);
                return Ok(ResolvedUrl {
                    direct_url,
                    headers: std::collections::HashMap::new(),
                    expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(6)),
                });
            }
        }
        
        // Read the body for further parsing
        let resp_body = resp.text().await.unwrap_or_default();
        
        // Case 2: JSON response — newer Fshare API returns { "url": "...", "location": "..." }
        if let Ok(data) = serde_json::from_str::<Value>(&resp_body) {
            tracing::debug!("[FSHARE-WEB] JSON response keys: {:?}", data.as_object().map(|o| o.keys().collect::<Vec<_>>()));
            
            let direct_url = data["url"].as_str()
                .or_else(|| data["location"].as_str())
                .or_else(|| data["download"].as_str())
                .or_else(|| data["link"].as_str());
            
            if let Some(url_str) = direct_url {
                if !url_str.is_empty() {
                    tracing::info!("[FSHARE-WEB] Got download URL from JSON: {}...", &url_str[..50.min(url_str.len())]);
                    return Ok(ResolvedUrl {
                        direct_url: url_str.to_string(),
                        headers: std::collections::HashMap::new(),
                        expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(6)),
                    });
                }
            }
            
            // Log the full JSON if it doesn't contain what we expect
            tracing::warn!("[FSHARE-WEB] JSON response but no download URL: {}", &resp_body[..resp_body.len().min(300)]);
        }
        
        // Case 3: Scan HTML for a cdn download link
        if let Some(pos) = resp_body.find("https://download") {
            if let Some(end) = resp_body[pos..].find('"').or_else(|| resp_body[pos..].find('\'')) {
                let direct_url = resp_body[pos..pos + end].to_string();
                tracing::info!("[FSHARE-WEB] Extracted download URL from HTML: {}...", &direct_url[..50.min(direct_url.len())]);
                return Ok(ResolvedUrl {
                    direct_url,
                    headers: std::collections::HashMap::new(),
                    expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(6)),
                });
            }
        }
        
        tracing::error!("[FSHARE-WEB] Could not extract download URL. Status: {}, Body preview: {}", 
            status, &resp_body[..500.min(resp_body.len())]);
        Err(anyhow::anyhow!("[FSHARE-WEB] No download URL found in /download/get response (status {})", status))
    }
    
    /// Check if we are currently in web-session mode (API is down)
    async fn is_web_session_mode(&self) -> bool {
        let session = self.session_data.lock().await;
        match session.as_ref() {
            Some(s) => s.session_id == "web-session",
            None => {
                let ws = self.web_session.lock().await;
                ws.is_some()
            }
        }
    }
    
    /// Save session to database
    async fn save_session_to_db(&self, db: &Arc<Db>, session: &SessionData) -> anyhow::Result<()> {
        let session_json = serde_json::to_string(session)?;
        db.save_setting("fshare_session", &session_json)?;
        Ok(())
    }
    
    /// Load session from database
    async fn load_session_from_db(&self, db: &Arc<Db>) -> anyhow::Result<Option<SessionData>> {
        match db.get_setting("fshare_session")? {
            Some(json) => {
                let session: SessionData = serde_json::from_str(&json)?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }
    
    /// Delete session from database
    async fn delete_session_from_db(&self, db: &Arc<Db>) -> anyhow::Result<()> {
        db.delete_setting("fshare_session")?;
        Ok(())
    }

    /// Internal helper to fetch account status using a known session
    async fn fetch_account_status_for_session(&self, session: &SessionData) -> anyhow::Result<AccountStatus> {
        let (email, _) = self.get_credentials();
        
        // Try api.fshare.vn first (faster and more reliable)
        let api2_result = self.api2_client
            .get("https://api.fshare.vn/api/user/get")
            .header("Cookie", format!("session_id={}", session.session_id))
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await;
        
        let data: Value = match api2_result {
            Ok(resp) if resp.status() == 200 => {
                match resp.json().await {
                    Ok(d) => d,
                    Err(_) => Value::Null,
                }
            }
            _ => Value::Null,
        };
        
        // If api2 failed, try primary API
        let data = if data.is_null() {
            let resp = self.client.post("https://download.fsharegroup.site/api/user/get")
                .json(&serde_json::json!({
                    "session_id": session.session_id,
                }))
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await?;
            resp.json().await.unwrap_or(Value::Null)
        } else {
            data
        };
        
        let email = data["email"].as_str().unwrap_or(&email).to_string();
        let account_type = data["account_type"].as_str().unwrap_or("Free");
        let level = match &data["level"] {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => "".to_string(),
        };

        // Level 3 IS VIP in Fshare API. Also check string for "vip" or "premium".
        let is_premium = account_type.to_lowercase().contains("vip") 
            || account_type.to_lowercase().contains("premium")
            || level == "3";
        
        // Parse valid_until (expire_vip from API is usually a string or number timestamp)
        let valid_until = data["expire_vip"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .or_else(|| data["expire_vip"].as_u64());

        tracing::info!("[FSHARE] Raw account data: {}", data);
        tracing::info!("[FSHARE] Account status verified: email='{}', type='{}', level='{}', is_premium={}, valid_until={:?}", email, account_type, level, is_premium, valid_until);
        
        Ok(AccountStatus {
            can_download: true,
            reason: None,
            account_email: email,
            premium: is_premium,
            valid_until,
            traffic_left: Some(data["traffic_used"].to_string()),
        })
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
        let session = self.ensure_valid_session().await?;
        self.fetch_account_status_for_session(&session).await
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
        
        // Standard FShare URL resolution
        
        // Check if we're in web session mode (API was previously unreachable)
        if self.is_web_session_mode().await {
            tracing::info!("[FSHARE] In web session mode, using web download resolution");
            return self.resolve_download_url_web(url).await;
        }
        
        // Check circuit breaker before making API call
        if let Err(e) = self.circuit_breaker.is_request_allowed().await {
            // Circuit breaker is open — try web fallback
            tracing::warn!("[FSHARE] Circuit breaker open ({}), trying web fallback", e);
            return self.resolve_download_url_web(url).await;
        }
        
        let session = self.ensure_valid_session().await?;
        tracing::info!("[FSHARE] Session ready, session_id: {}...", &session.session_id[..20.min(session.session_id.len())]);

        // If session_id is synthetic (web-session), use web resolution
        if session.session_id == "web-session" {
            return self.resolve_download_url_web(url).await;
        }

        // Build request payload
        let request_payload = serde_json::json!({
            "session_id": session.session_id,
            "token": session.token,
            "url": url,
            "password": "",
        });
        tracing::info!("[FSHARE] Request payload (sanitized): url={}, password=''", url);

        // Determine which API endpoint to use based on session source
        // api.fshare.vn uses /api/session/download with {token, url}
        // download.fsharegroup.site uses /int/get with {session_id, token, url, password}
        
        // Try api.fshare.vn first (it's faster and more reliable)
        let api2_payload = serde_json::json!({
            "token": session.token,
            "url": url,
        });
        
        let api2_result = self.api2_client
            .post("https://api.fshare.vn/api/session/download")
            .header("Cookie", format!("session_id={}", session.session_id))
            .json(&api2_payload)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await;
        
        match api2_result {
            Ok(resp) => {
                let status = resp.status();
                tracing::info!("[FSHARE-API2] Download resolution status: {}", status);
                
                // Handle redirect: FShare CDN sometimes redirects — follow manually with POST
                let resp = if status == reqwest::StatusCode::MOVED_PERMANENTLY || status == reqwest::StatusCode::FOUND || status == reqwest::StatusCode::TEMPORARY_REDIRECT {
                    if let Some(location) = resp.headers().get("location").and_then(|v| v.to_str().ok()) {
                        tracing::info!("[FSHARE-API2] Following download redirect to: {}", location);
                        match self.api2_client
                            .post(location)
                            .header("Cookie", format!("session_id={}", session.session_id))
                            .json(&api2_payload)
                            .timeout(std::time::Duration::from_secs(15))
                            .send()
                            .await {
                            Ok(r) => r,
                            Err(e) => {
                                tracing::warn!("[FSHARE-API2] Download redirect failed: {}", e);
                                return Err(anyhow::anyhow!("Download redirect failed: {}", e));
                            }
                        }
                    } else {
                        resp
                    }
                } else {
                    resp
                };
                
                let status = resp.status();
                
                if status.is_success() {
                    // Read body as text first to avoid json decode errors crashing the fallback
                    let body_text = resp.text().await.unwrap_or_default();
                    tracing::info!("[FSHARE-API2] Response body: {}", &body_text[..body_text.len().min(500)]);
                    
                    match serde_json::from_str::<Value>(&body_text) {
                        Ok(data) => {
                            if let Some(location) = data["location"].as_str() {
                                if !location.is_empty() {
                                    self.circuit_breaker.record_success().await;
                                    tracing::info!("[FSHARE-API2] Successfully resolved download URL");
                                    return Ok(ResolvedUrl {
                                        direct_url: location.to_string(),
                                        headers: std::collections::HashMap::new(),
                                        expires_at: Some(chrono::Utc::now() + chrono::Duration::hours(6)),
                                    });
                                }
                            }
                            tracing::warn!("[FSHARE-API2] No location in response: {}", body_text);
                        }
                        Err(e) => {
                            tracing::warn!("[FSHARE-API2] Failed to parse JSON ({}), body: {}", e, &body_text[..body_text.len().min(200)]);
                        }
                    }
                }
                
                // api.fshare.vn failed, try fallback to old API
                tracing::warn!("[FSHARE-API2] Failed (status {}), trying primary API", status);
            }
            Err(e) => {
                let err_str = format!("{}", e);
                if err_str.contains("timed out") || err_str.contains("connect") {
                    tracing::warn!("[FSHARE-API2] Unreachable ({}), trying primary API", err_str);
                } else {
                    tracing::warn!("[FSHARE-API2] Error ({}), trying primary API", err_str);
                }
            }
        }
        
        // Fallback to primary API (download.fsharegroup.site)
        let api_result = self.client.post("https://download.fsharegroup.site/int/get")
            .json(&request_payload)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await;
        
        // If primary API is unreachable, fall back to web
        let resp = match api_result {
            Ok(r) => r,
            Err(e) => {
                let err_str = format!("{}", e);
                if err_str.contains("timed out") || err_str.contains("connect") {
                    tracing::warn!("[FSHARE] Primary API /int/get unreachable ({}), falling back to web", err_str);
                    self.circuit_breaker.record_failure().await;
                    return self.resolve_download_url_web(url).await;
                }
                self.circuit_breaker.record_failure().await;
                return Err(anyhow::anyhow!("Fshare API request failed: {}", e));
            }
        };
        
        let resp_status = resp.status();
        tracing::info!("[FSHARE] Primary API response status: {}", resp_status);

        let data: Value = resp.json().await?;
        
        // Debug: Log the full response
        tracing::info!("[FSHARE] Full API response: {}", serde_json::to_string(&data).unwrap_or_else(|_| format!("{:?}", data)));
        
        // Check for errors in response
        if let Some(error) = data["error"].as_str() {
            tracing::error!("[FSHARE] Error in response: {}", error);
            self.circuit_breaker.record_failure().await;
            return Err(anyhow::anyhow!("Fshare error: {}", error));
        }
        
        // Check for error code
        if let Some(code) = data["code"].as_i64() {
            if code != 200 {
                let msg = data["msg"].as_str().unwrap_or("Unknown error");
                tracing::error!("[FSHARE] Error code {} in response: {}", code, msg);
                self.circuit_breaker.record_failure().await;
                return Err(anyhow::anyhow!("Fshare error (code {}): {}", code, msg));
            }
        }

        let direct_url = data["location"].as_str()
            .or_else(|| data["url"].as_str())
            .ok_or_else(|| {
                tracing::error!("[FSHARE] No download URL found in response keys. Available keys: {:?}", 
                    data.as_object().map(|o| o.keys().collect::<Vec<_>>()));
                let circuit_breaker = self.circuit_breaker.clone();
                tokio::spawn(async move {
                    circuit_breaker.record_failure().await;
                });
                anyhow::anyhow!("No download URL in response. Response: {}", serde_json::to_string(&data).unwrap_or_default())
            })?
            .to_string();

        tracing::info!("[FSHARE] Resolved direct URL: {}...", &direct_url[..50.min(direct_url.len())]);
        
        // Success! Record in circuit breaker
        self.circuit_breaker.record_success().await;

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
