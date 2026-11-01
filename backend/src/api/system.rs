//! System API Routes
//!
//! System information and activity endpoints.

use crate::{config, db::ActivityEvent, AppState};
use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf, sync::Arc};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/version", get(get_version))
        .route("/logs", get(get_logs))
        .route("/activity", get(get_activity))
}

#[derive(Serialize)]
struct VersionResponse {
    version: String,
    rust_version: &'static str,
    build_date: Option<String>,
}

#[derive(Serialize, Clone, PartialEq, Debug)]
struct LogEntry {
    timestamp: String,
    level: String,
    message: String,
}

#[derive(Serialize)]
struct LogsResponse {
    logs: Vec<LogEntry>,
    total: usize,
}

#[derive(Serialize)]
struct ActivityResponse {
    activities: Vec<ActivityEvent>,
    total: usize,
}

#[derive(Deserialize)]
struct LogsQuery {
    #[serde(default = "default_lines")]
    lines: usize,
    #[serde(default)]
    level: Option<String>,
}

#[derive(Deserialize)]
struct ActivityQuery {
    #[serde(default = "default_activity_limit")]
    limit: usize,
}

fn default_lines() -> usize {
    100
}
fn default_activity_limit() -> usize {
    50
}

/// GET /api/system/version - Get version info
async fn get_version() -> Json<VersionResponse> {
    let version = fs::read_to_string("VERSION")
        .or_else(|_| fs::read_to_string("../VERSION"))
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string())
        .trim()
        .to_string();

    Json(VersionResponse {
        version,
        rust_version: "1.75+",
        build_date: option_env!("BUILD_DATE").map(|s| s.to_string()),
    })
}

/// GET /api/system/logs - Get recent JSON tracing entries.
async fn get_logs(Query(params): Query<LogsQuery>) -> Json<LogsResponse> {
    let logs = read_logs(params.lines.clamp(1, 1000), params.level.as_deref());
    let total = logs.len();
    Json(LogsResponse { logs, total })
}

/// GET /api/system/activity - Get durable user-meaningful download outcomes.
async fn get_activity(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ActivityQuery>,
) -> Json<ActivityResponse> {
    let activities = state
        .db
        .get_activity_events_async(params.limit.clamp(1, 100))
        .await
        .unwrap_or_else(|error| {
            tracing::warn!("Failed to load activity events: {error}");
            Vec::new()
        });
    let total = activities.len();
    Json(ActivityResponse { activities, total })
}

fn read_logs(lines: usize, level: Option<&str>) -> Vec<LogEntry> {
    let mut paths: Vec<PathBuf> = fs::read_dir(config::get_appdata_dir().join("logs"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("flasharr.log"))
        })
        .collect();
    paths.sort();
    paths.reverse();

    let level = level.map(str::to_ascii_uppercase);
    let mut logs = Vec::with_capacity(lines);
    for path in paths {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        for line in content.lines().rev() {
            let Some(entry) = parse_log_line(line) else {
                continue;
            };
            if level.as_ref().is_some_and(|filter| entry.level != *filter) {
                continue;
            }
            logs.push(entry);
            if logs.len() == lines {
                logs.reverse();
                return logs;
            }
        }
    }
    logs.reverse();
    logs
}

fn parse_log_line(line: &str) -> Option<LogEntry> {
    let value: Value = serde_json::from_str(line).ok()?;
    let timestamp = value.get("timestamp")?.as_str()?.to_string();
    let level = value.get("level")?.as_str()?.to_ascii_uppercase();
    let message = value
        .pointer("/fields/message")
        .or_else(|| value.get("message"))?
        .as_str()?
        .to_string();
    Some(LogEntry {
        timestamp,
        level,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_tracing_line() {
        assert_eq!(
            parse_log_line(
                r#"{"timestamp":"2026-07-26T12:00:00Z","level":"INFO","fields":{"message":"Started"}}"#
            ),
            Some(LogEntry {
                timestamp: "2026-07-26T12:00:00Z".into(),
                level: "INFO".into(),
                message: "Started".into()
            })
        );
    }
}
