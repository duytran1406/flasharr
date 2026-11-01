pub mod fshare;

use crate::AppState;
use axum::{
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list))
        .route("/fshare/status", get(fshare::status))
        .route("/fshare/setup", post(fshare::setup))
        .nest("/fshare/accounts", fshare::canonical_accounts_router())
}

async fn list() -> Json<Value> {
    Json(
        json!({"providers": [{"id": "fshare", "capabilities": ["accounts", "folder", "download"]}]}),
    )
}
