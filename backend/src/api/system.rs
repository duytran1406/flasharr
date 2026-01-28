//! System API Routes
//!
//! System information and health endpoints.

use axum::{
    routing::get,
    Router,
    Json,
    extract::Query,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::fs;
use std::path::Path;
use crate::AppState;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/version", get(get_version))
        .route("/logs", get(get_logs))
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Serialize)]
struct VersionResponse {
    version: String,
    rust_version: &'static str,
    build_date: Option<String>,
}

#[derive(Serialize)]
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

// ============================================================================
// Request Types
// ============================================================================

#[derive(Deserialize)]
struct LogsQuery {
    #[serde(default = "default_lines")]
    lines: usize,
    #[serde(default)]
    level: Option<String>,
}

fn default_lines() -> usize {
    100
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/system/version - Get version info
async fn get_version() -> Json<VersionResponse> {
    // Try to read VERSION file
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

/// GET /api/system/logs - Get recent log entries
async fn get_logs(
    Query(params): Query<LogsQuery>,
) -> Json<LogsResponse> {
    let lines = params.lines.min(1000); // Cap at 1000 lines
    
    // Try to read log file
    let log_paths = [
        "data/flasharr.log",
        "../data/flasharr.log",
        "flasharr.log",
    ];
    
    let mut log_content = String::new();
    for path in log_paths {
        if Path::new(path).exists() {
            if let Ok(content) = fs::read_to_string(path) {
                log_content = content;
                break;
            }
        }
    }
    
    // Parse log entries (simple line-based parsing)
    let log_lines: Vec<&str> = log_content.lines().rev().take(lines).collect();
    let mut logs: Vec<LogEntry> = Vec::new();
    
    for line in log_lines.into_iter().rev() {
        // Simple parsing: assume format "TIMESTAMP - LEVEL - MESSAGE"
        let parts: Vec<&str> = line.splitn(3, " - ").collect();
        if parts.len() >= 3 {
            let level = parts[1].to_uppercase();
            
            // Filter by level if specified
            if let Some(ref filter_level) = params.level {
                if !level.contains(&filter_level.to_uppercase()) {
                    continue;
                }
            }
            
            logs.push(LogEntry {
                timestamp: parts[0].to_string(),
                level,
                message: parts[2].to_string(),
            });
        } else {
            // Fallback: treat entire line as message
            logs.push(LogEntry {
                timestamp: String::new(),
                level: "INFO".to_string(),
                message: line.to_string(),
            });
        }
    }
    
    let total = logs.len();
    Json(LogsResponse { logs, total })
}
