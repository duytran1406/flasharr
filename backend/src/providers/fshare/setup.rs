use crate::{api::setup::handlers, AppState};
use axum::{extract::State, http::StatusCode, Json};
use std::sync::Arc;

pub use handlers::{FshareCredentials, TestResult};

pub async fn setup(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FshareCredentials>,
) -> Result<Json<TestResult>, StatusCode> {
    handlers::setup_fshare_impl(State(state), Json(payload)).await
}
