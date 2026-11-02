use crate::api::kpi;
use crate::AppState;
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderMap, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
};

const SESSION_COOKIE: &str = "flasharr_session";
const SESSION_TTL_SECS: i64 = 60 * 60 * 24 * 14;
const LOGIN_WINDOW_SECS: i64 = 60;
const MAX_LOGIN_ATTEMPTS: u32 = 8;

static LOGIN_ATTEMPTS: OnceLock<Mutex<HashMap<String, (u32, i64)>>> = OnceLock::new();

#[derive(Serialize)]
struct SetupStatusResponse {
    configured: bool,
    authenticated: bool,
    username: Option<String>,
}

#[derive(Serialize)]
struct SessionResponse {
    authenticated: bool,
    username: Option<String>,
}

#[derive(Deserialize)]
struct Credentials {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Serialize)]
struct ActionResponse {
    success: bool,
    message: Option<String>,
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/setup-status", get(setup_status))
        .route("/setup", post(setup))
        .route("/login", post(login))
        .route("/logout", post(logout))
        .route("/change-password", post(change_password))
        .route("/session", get(session))
        .route("/verify", get(verify_key))
}

async fn setup_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<SetupStatusResponse>, StatusCode> {
    let username = session_username(&state, &headers)?;
    Ok(Json(SetupStatusResponse {
        configured: state
            .db
            .has_app_users()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
        authenticated: username.is_some(),
        username,
    }))
}

async fn setup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(credentials): Json<Credentials>,
) -> Result<Response, StatusCode> {
    if state
        .db
        .has_app_users()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::CONFLICT);
    }
    let credentials = validate_credentials(credentials)?;
    let password_hash = hash_password(&credentials.password)?;
    let user_id = uuid::Uuid::new_v4().to_string();
    if !state
        .db
        .create_initial_app_user(&user_id, &credentials.username, &password_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::CONFLICT);
    }
    Ok(login_response(
        &state,
        &headers,
        &user_id,
        &credentials.username,
    )?)
}

async fn login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(credentials): Json<Credentials>,
) -> Result<Response, StatusCode> {
    let client = client_key(&headers);
    if login_rate_limited(&client) {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    let username = credentials.username.trim().to_string();
    let Some((user_id, password_hash)) = state
        .db
        .get_app_user_password_hash(&username)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    else {
        record_failed_login(client);
        return Err(StatusCode::UNAUTHORIZED);
    };
    let parsed_hash =
        PasswordHash::new(&password_hash).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if Argon2::default()
        .verify_password(credentials.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        record_failed_login(client);
        return Err(StatusCode::UNAUTHORIZED);
    }
    clear_login_attempts(&client);
    Ok(login_response(&state, &headers, &user_id, &username)?)
}

async fn logout(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if let Some(token) = session_token(&headers) {
        state
            .db
            .delete_app_session(&token_hash(&token))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    Ok((
        [(header::SET_COOKIE, expired_cookie())],
        Json(ActionResponse {
            success: true,
            message: None,
        }),
    )
        .into_response())
}

async fn session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<SessionResponse>, StatusCode> {
    let username = session_username(&state, &headers)?;
    Ok(Json(SessionResponse {
        authenticated: username.is_some(),
        username,
    }))
}

async fn change_password(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<Response, StatusCode> {
    let username = session_username(&state, &headers)?.ok_or(StatusCode::UNAUTHORIZED)?;
    if payload.new_password.len() < 12 || payload.new_password.len() > 256 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let (user_id, password_hash) = state
        .db
        .get_app_user_password_hash(&username)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    let parsed_hash =
        PasswordHash::new(&password_hash).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if Argon2::default()
        .verify_password(payload.current_password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if payload.current_password == payload.new_password {
        return Err(StatusCode::BAD_REQUEST);
    }
    let new_hash = hash_password(&payload.new_password)?;
    if !state
        .db
        .update_app_user_password(&user_id, &new_hash)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok((
        [(header::SET_COOKIE, expired_cookie())],
        Json(ActionResponse {
            success: true,
            message: Some("Password changed. Sign in again.".to_string()),
        }),
    )
        .into_response())
}

async fn verify_key(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ActionResponse>, StatusCode> {
    if !is_authenticated(&state, &headers)? {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(ActionResponse {
        success: true,
        message: None,
    }))
}

pub async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if is_authenticated(&state, req.headers())? {
        kpi::inc_auth("ok");
        return Ok(next.run(req).await);
    }
    kpi::inc_auth("unauthorized");
    tracing::warn!(path = %req.uri().path(), "Unauthorized API access attempt");
    Err(StatusCode::UNAUTHORIZED)
}

pub fn validate_api_key(state: &AppState, provided: &str) -> bool {
    state
        .db
        .get_setting("indexer_api_key")
        .ok()
        .flatten()
        .is_some_and(|key| !key.is_empty() && !provided.is_empty() && provided == key)
}

fn is_authenticated(state: &AppState, headers: &HeaderMap) -> Result<bool, StatusCode> {
    Ok(session_username(state, headers)?.is_some())
}

pub async fn setup_auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if req.uri().path().ends_with("/status")
        || !state
            .db
            .has_app_users()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        || session_username(&state, req.headers())?.is_some()
    {
        return Ok(next.run(req).await);
    }
    Err(StatusCode::UNAUTHORIZED)
}

pub async fn indexer_auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let api_key = req
        .headers()
        .get("X-Api-Key")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .or_else(|| {
            url::form_urlencoded::parse(req.uri().query().unwrap_or_default().as_bytes())
                .find_map(|(key, value)| (key == "apikey").then(|| value.into_owned()))
        });
    if api_key
        .as_deref()
        .is_some_and(|key| validate_api_key(&state, key))
        || session_username(&state, req.headers())?.is_some()
    {
        return Ok(next.run(req).await);
    }
    Err(StatusCode::UNAUTHORIZED)
}

fn session_username(state: &AppState, headers: &HeaderMap) -> Result<Option<String>, StatusCode> {
    let Some(token) = session_token(headers) else {
        return Ok(None);
    };
    state
        .db
        .validate_app_session(&token_hash(&token))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn validate_credentials(mut credentials: Credentials) -> Result<Credentials, StatusCode> {
    credentials.username = credentials.username.trim().to_string();
    if credentials.username.len() < 3
        || credentials.username.len() > 64
        || !credentials
            .username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
        || credentials.password.len() < 12
        || credentials.password.len() > 256
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(credentials)
}

fn hash_password(password: &str) -> Result<String, StatusCode> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn login_response(
    state: &AppState,
    headers: &HeaderMap,
    user_id: &str,
    username: &str,
) -> Result<Response, StatusCode> {
    let mut token = [0u8; 32];
    OsRng.fill_bytes(&mut token);
    let token = hex(&token);
    let expires_at = chrono::Utc::now().timestamp() + SESSION_TTL_SECS;
    state
        .db
        .create_app_session(&token_hash(&token), user_id, expires_at)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let secure = headers
        .get("x-forwarded-proto")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("https"))
        || std::env::var("FLASHARR_COOKIE_SECURE").ok().as_deref() == Some("true");
    Ok((
        [(header::SET_COOKIE, session_cookie(&token, secure))],
        Json(SessionResponse {
            authenticated: true,
            username: Some(username.to_string()),
        }),
    )
        .into_response())
}

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value.split(';').find_map(|part| {
                let (name, value) = part.trim().split_once('=')?;
                (name == SESSION_COOKIE).then(|| value.to_string())
            })
        })
        .filter(|token| token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()))
}

fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

fn session_cookie(token: &str, secure: bool) -> String {
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={SESSION_TTL_SECS}{}",
        if secure { "; Secure" } else { "" }
    )
}

fn expired_cookie() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}

fn client_key(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .unwrap_or("unknown")
        .trim()
        .to_string()
}

fn login_rate_limited(client: &str) -> bool {
    let now = chrono::Utc::now().timestamp();
    let attempts = LOGIN_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut attempts = attempts.lock().expect("login attempt lock");
    attempts.retain(|_, (_, until)| *until > now);
    attempts
        .get(client)
        .is_some_and(|(count, _)| *count >= MAX_LOGIN_ATTEMPTS)
}

fn record_failed_login(client: String) {
    let now = chrono::Utc::now().timestamp();
    let attempts = LOGIN_ATTEMPTS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut attempts = attempts.lock().expect("login attempt lock");
    let entry = attempts
        .entry(client)
        .or_insert((0, now + LOGIN_WINDOW_SECS));
    if entry.1 <= now {
        *entry = (0, now + LOGIN_WINDOW_SECS);
    }
    entry.0 += 1;
}

fn clear_login_attempts(client: &str) {
    if let Some(attempts) = LOGIN_ATTEMPTS.get() {
        attempts.lock().expect("login attempt lock").remove(client);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_safe_credentials() {
        assert!(validate_credentials(Credentials {
            username: "admin.user".into(),
            password: "correct-horse-battery".into()
        })
        .is_ok());
        assert!(validate_credentials(Credentials {
            username: "x".into(),
            password: "correct-horse-battery".into()
        })
        .is_err());
        assert!(validate_credentials(Credentials {
            username: "admin user".into(),
            password: "correct-horse-battery".into()
        })
        .is_err());
        assert!(validate_credentials(Credentials {
            username: "admin".into(),
            password: "short".into()
        })
        .is_err());
    }

    #[test]
    fn password_update_invalidates_sessions() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = crate::db::Db::new(file.path()).unwrap();
        db.create_initial_app_user("user", "admin", "old-hash")
            .unwrap();
        db.create_app_session("session", "user", chrono::Utc::now().timestamp() + 60)
            .unwrap();

        assert!(db.update_app_user_password("user", "new-hash").unwrap());
        assert_eq!(
            db.get_app_user_password_hash("admin").unwrap(),
            Some(("user".into(), "new-hash".into()))
        );
        assert_eq!(db.validate_app_session("session").unwrap(), None);
    }

    #[test]
    fn session_cookie_is_http_only_lax() {
        let cookie = session_cookie("aabb", true);
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Secure"));
    }

    #[test]
    fn bootstrap_user_is_atomic() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let db = crate::db::Db::new(file.path()).unwrap();
        assert!(db
            .create_initial_app_user("first", "admin", "hash")
            .unwrap());
        assert!(!db
            .create_initial_app_user("second", "other", "hash")
            .unwrap());
    }
}
