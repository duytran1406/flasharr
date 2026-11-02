use crate::AppState;
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::sync::Arc;

pub async fn status(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "provider_id": "fshare",
        "available": is_available(&state),
    }))
}

pub fn is_available(state: &AppState) -> bool {
    state.host_registry.get_handler("fshare").is_some()
}
