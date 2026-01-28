//! Stats API Routes
//!
//! Engine statistics endpoints.

use axum::{
    routing::get,
    Router,
    Json,
    extract::State,
};
use std::sync::Arc;
use crate::AppState;
use crate::downloader::EngineStats;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(get_stats))
}

/// GET /api/stats - Get engine statistics
async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Json<EngineStats> {
    Json(state.download_orchestrator.task_manager().get_stats())
}
