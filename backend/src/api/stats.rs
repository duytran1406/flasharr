//! Stats API Routes
//!
//! Engine statistics endpoints.

use crate::downloader::EngineStats;
use crate::AppState;
use axum::{extract::State, routing::get, Json, Router};
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_stats))
}

/// GET /api/stats - Get engine statistics
async fn get_stats(State(state): State<Arc<AppState>>) -> Json<EngineStats> {
    Json(state.download_orchestrator.task_manager().get_stats().await)
}
