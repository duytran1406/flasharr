//! Health Check API
//!
//! Provides comprehensive health status for all system components

use crate::api::kpi;
use crate::AppState;
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
    NotConfigured,
}

#[derive(Serialize)]
pub struct ServiceHealth {
    pub status: HealthStatus,
    pub message: Option<String>,
    pub response_time_ms: Option<u64>,
}

#[derive(Serialize)]
pub struct HealthCheckResponse {
    pub overall_status: HealthStatus,
    pub websocket: ServiceHealth,
    pub webhook: ServiceHealth,
    pub sonarr: Option<ServiceHealth>,
    pub radarr: Option<ServiceHealth>,
    pub fshare: ServiceHealth,
    pub fshare_ping: ServiceHealth,
    pub internet_speed: ServiceHealth,
    pub database: ServiceHealth,
}

/// GET /api/health/status - Comprehensive health check
pub async fn health_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HealthCheckResponse>, StatusCode> {
    kpi::inc_health_check();

    let websocket = check_websocket(&state).await;
    let webhook = check_webhook(&state).await;
    let sonarr = check_arr_service(&state, "sonarr").await;
    let radarr = check_arr_service(&state, "radarr").await;
    let fshare = check_fshare(&state).await;
    let fshare_ping = check_fshare_ping(&state).await;
    let internet_speed = check_internet_speed(&state).await;
    let database = check_database(&state).await;

    let overall_status = determine_overall_status(&[
        &websocket.status,
        &webhook.status,
        &sonarr
            .as_ref()
            .map(|s| &s.status)
            .unwrap_or(&HealthStatus::NotConfigured),
        &radarr
            .as_ref()
            .map(|s| &s.status)
            .unwrap_or(&HealthStatus::NotConfigured),
        &fshare.status,
        &fshare_ping.status,
        &database.status,
        &internet_speed.status,
    ]);

    Ok(Json(HealthCheckResponse {
        overall_status,
        websocket,
        webhook,
        sonarr,
        radarr,
        fshare,
        fshare_ping,
        internet_speed,
        database,
    }))
}

async fn check_arr_service(state: &AppState, service_type: &str) -> Option<ServiceHealth> {
    let db = &state.db;

    let fallback = match service_type {
        "sonarr" => state.config.sonarr.clone(),
        "radarr" => state.config.radarr.clone(),
        _ => return None,
    };
    let Some(config) = crate::api::settings::effective_arr_config(db, service_type, fallback)
    else {
        return Some(ServiceHealth {
            status: HealthStatus::NotConfigured,
            message: Some("Disabled or not configured".to_string()),
            response_time_ms: None,
        });
    };
    let url = config.url;
    let api_key = config.api_key;
    if crate::api::settings::validate_api_key(&api_key).is_err() {
        return Some(ServiceHealth {
            status: HealthStatus::NotConfigured,
            message: Some("API key is invalid".to_string()),
            response_time_ms: None,
        });
    }

    let start = std::time::Instant::now();
    let client = reqwest::Client::new();
    let test_url = format!("{}/api/v3/system/status", url.trim_end_matches('/'));

    match client
        .get(&test_url)
        .header("X-Api-Key", api_key)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            let response_time = start.elapsed().as_millis() as u64;
            if resp.status().is_success() {
                Some(ServiceHealth {
                    status: HealthStatus::Healthy,
                    message: Some(format!("Connected ({}ms)", response_time)),
                    response_time_ms: Some(response_time),
                })
            } else {
                Some(ServiceHealth {
                    status: HealthStatus::Unhealthy,
                    message: Some(format!("HTTP {}", resp.status())),
                    response_time_ms: Some(response_time),
                })
            }
        }
        Err(e) => Some(ServiceHealth {
            status: HealthStatus::Unhealthy,
            message: Some(format!("Connection failed: {}", e)),
            response_time_ms: None,
        }),
    }
}

async fn check_fshare(state: &AppState) -> ServiceHealth {
    if crate::providers::fshare::is_available(state) {
        ServiceHealth {
            status: HealthStatus::Healthy,
            message: Some("Handler available".to_string()),
            response_time_ms: Some(0),
        }
    } else {
        ServiceHealth {
            status: HealthStatus::Degraded,
            message: Some("Handler not found".to_string()),
            response_time_ms: Some(0),
        }
    }
}

async fn check_database(state: &AppState) -> ServiceHealth {
    match state.db.get_folder_cache_count() {
        Ok(_) => ServiceHealth {
            status: HealthStatus::Healthy,
            message: Some("Connected".to_string()),
            response_time_ms: Some(0),
        },
        Err(e) => ServiceHealth {
            status: HealthStatus::Unhealthy,
            message: Some(format!("DB check failed: {}", e)),
            response_time_ms: None,
        },
    }
}

fn determine_overall_status(statuses: &[&HealthStatus]) -> HealthStatus {
    if statuses
        .iter()
        .any(|s| matches!(s, HealthStatus::Unhealthy))
    {
        HealthStatus::Unhealthy
    } else if statuses.iter().any(|s| matches!(s, HealthStatus::Degraded)) {
        HealthStatus::Degraded
    } else if statuses
        .iter()
        .any(|s| matches!(s, HealthStatus::Unknown | HealthStatus::NotConfigured))
    {
        HealthStatus::Unknown
    } else {
        HealthStatus::Healthy
    }
}

async fn check_websocket(state: &AppState) -> ServiceHealth {
    let receiver_count = state.tx_broadcast.receiver_count();

    ServiceHealth {
        status: HealthStatus::Healthy,
        message: Some(if receiver_count > 0 {
            format!("{} subscribers", receiver_count)
        } else {
            "Ready".to_string()
        }),
        response_time_ms: Some(0),
    }
}

async fn check_webhook(_state: &AppState) -> ServiceHealth {
    webhook_ready_health()
}

fn webhook_ready_health() -> ServiceHealth {
    ServiceHealth {
        status: HealthStatus::Healthy,
        message: Some("SABnzbd-compatible API ready at /sabnzbd/api".to_string()),
        response_time_ms: Some(0),
    }
}

async fn check_fshare_ping(_state: &AppState) -> ServiceHealth {
    const ATTEMPTS: usize = 3;

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("Flasharr health check")
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            return ServiceHealth {
                status: HealthStatus::Degraded,
                message: Some(format!("Failed to create ping client: {}", e)),
                response_time_ms: None,
            }
        }
    };

    let mut last_message = "No response".to_string();
    let mut last_response_time = None;

    for attempt in 1..=ATTEMPTS {
        let start = std::time::Instant::now();
        match client.get("https://www.fshare.vn/").send().await {
            Ok(resp) => {
                let response_time = start.elapsed().as_millis() as u64;
                let status = resp.status();
                last_response_time = Some(response_time);

                if status.is_success() || status.is_redirection() {
                    return ServiceHealth {
                        status: HealthStatus::Healthy,
                        message: Some(format!("{}ms", response_time)),
                        response_time_ms: Some(response_time),
                    };
                }

                last_message = format!("HTTP {}", status);
                if is_transient_fshare_ping_status(status.as_u16()) && attempt < ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_millis(200 * attempt as u64))
                        .await;
                    continue;
                }

                return ServiceHealth {
                    status: HealthStatus::Degraded,
                    message: Some(format_fshare_ping_failure(&last_message, attempt)),
                    response_time_ms: last_response_time,
                };
            }
            Err(e) => {
                last_message = format!("Failed: {}", e);
                last_response_time = None;
                if attempt < ATTEMPTS {
                    tokio::time::sleep(std::time::Duration::from_millis(200 * attempt as u64))
                        .await;
                    continue;
                }
            }
        }
    }

    ServiceHealth {
        status: HealthStatus::Degraded,
        message: Some(format_fshare_ping_failure(&last_message, ATTEMPTS)),
        response_time_ms: last_response_time,
    }
}

fn is_transient_fshare_ping_status(status: u16) -> bool {
    status == 429 || (500..=599).contains(&status)
}

fn format_fshare_ping_failure(message: &str, attempts: usize) -> String {
    if attempts > 1 {
        format!("{} after {} attempts", message, attempts)
    } else {
        message.to_string()
    }
}

const DEFAULT_SPEEDTEST_TRACKER_URL: &str = "http://192.168.1.109:8765";

#[derive(Deserialize)]
struct SpeedtestLatestResponse {
    message: Option<String>,
    data: Option<SpeedtestLatestData>,
}

#[derive(Deserialize)]
struct SpeedtestLatestData {
    ping: Option<serde_json::Value>,
    download: Option<serde_json::Value>,
    upload: Option<serde_json::Value>,
    failed: Option<bool>,
}

async fn check_internet_speed(state: &AppState) -> ServiceHealth {
    let base_url = state
        .db
        .get_setting("speedtest_tracker_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_SPEEDTEST_TRACKER_URL.to_string());

    let base_url = base_url.trim();
    if base_url.is_empty() || base_url.eq_ignore_ascii_case("disabled") {
        return ServiceHealth {
            status: HealthStatus::NotConfigured,
            message: Some("Speedtest Tracker disabled".to_string()),
            response_time_ms: None,
        };
    }

    let latest_url = speedtest_latest_url(base_url);
    let start = std::time::Instant::now();

    match state
        .http_client
        .get(&latest_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => {
            let response_time = start.elapsed().as_millis() as u64;
            let status = resp.status();
            if !status.is_success() {
                return ServiceHealth {
                    status: HealthStatus::Degraded,
                    message: Some(format!("Speedtest Tracker HTTP {}", status)),
                    response_time_ms: Some(response_time),
                };
            }

            match resp.json::<SpeedtestLatestResponse>().await {
                Ok(payload) => classify_speedtest_latest(payload, response_time),
                Err(e) => ServiceHealth {
                    status: HealthStatus::Degraded,
                    message: Some(format!("Speedtest Tracker parse failed: {}", e)),
                    response_time_ms: Some(response_time),
                },
            }
        }
        Err(e) => ServiceHealth {
            status: HealthStatus::Degraded,
            message: Some(format!("Speedtest Tracker unreachable: {}", e)),
            response_time_ms: None,
        },
    }
}

fn speedtest_latest_url(base_url: &str) -> String {
    format!("{}/api/speedtest/latest", base_url.trim_end_matches('/'))
}

fn classify_speedtest_latest(
    payload: SpeedtestLatestResponse,
    response_time_ms: u64,
) -> ServiceHealth {
    if !payload
        .message
        .as_deref()
        .is_some_and(|message| message.eq_ignore_ascii_case("ok"))
    {
        return ServiceHealth {
            status: HealthStatus::Degraded,
            message: Some("Speedtest Tracker returned non-ok response".to_string()),
            response_time_ms: Some(response_time_ms),
        };
    }

    let Some(data) = payload.data else {
        return ServiceHealth {
            status: HealthStatus::Degraded,
            message: Some("Speedtest Tracker returned no latest result".to_string()),
            response_time_ms: Some(response_time_ms),
        };
    };

    if data.failed.unwrap_or(false) {
        return ServiceHealth {
            status: HealthStatus::Degraded,
            message: Some("Latest speed test failed".to_string()),
            response_time_ms: Some(response_time_ms),
        };
    }

    let download = parse_speedtest_metric(data.download.as_ref());
    let upload = parse_speedtest_metric(data.upload.as_ref());
    let ping = parse_speedtest_metric(data.ping.as_ref());

    match (download, upload, ping) {
        (Some(download), Some(upload), Some(ping)) => ServiceHealth {
            status: HealthStatus::Healthy,
            message: Some(format_speedtest_message(download, upload, ping)),
            response_time_ms: Some(response_time_ms),
        },
        _ => ServiceHealth {
            status: HealthStatus::Degraded,
            message: Some("Speedtest Tracker latest result missing metrics".to_string()),
            response_time_ms: Some(response_time_ms),
        },
    }
}

fn parse_speedtest_metric(value: Option<&serde_json::Value>) -> Option<f64> {
    match value? {
        serde_json::Value::Number(n) => n.as_f64(),
        serde_json::Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

fn format_speedtest_message(download: f64, upload: f64, ping: f64) -> String {
    format!(
        "↓ {:.2} Mbps ↑ {:.2} Mbps • {:.2} ms",
        download, upload, ping
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn webhook_ready_health_does_not_require_sabnzbd_config() {
        let health = webhook_ready_health();

        assert!(matches!(health.status, HealthStatus::Healthy));
        assert_eq!(
            health.message.as_deref(),
            Some("SABnzbd-compatible API ready at /sabnzbd/api")
        );
        assert_eq!(health.response_time_ms, Some(0));
    }

    #[test]
    fn fshare_ping_transient_statuses_are_retryable() {
        assert!(is_transient_fshare_ping_status(429));
        assert!(is_transient_fshare_ping_status(500));
        assert!(is_transient_fshare_ping_status(502));
        assert!(is_transient_fshare_ping_status(599));
        assert!(!is_transient_fshare_ping_status(400));
        assert!(!is_transient_fshare_ping_status(404));
    }

    #[test]
    fn fshare_ping_failure_message_includes_attempts() {
        assert_eq!(
            format_fshare_ping_failure("HTTP 502 Bad Gateway", 3),
            "HTTP 502 Bad Gateway after 3 attempts"
        );
        assert_eq!(
            format_fshare_ping_failure("HTTP 404 Not Found", 1),
            "HTTP 404 Not Found"
        );
    }

    #[test]
    fn speedtest_latest_url_trims_trailing_slash() {
        assert_eq!(
            speedtest_latest_url("http://192.168.1.109:8765/"),
            "http://192.168.1.109:8765/api/speedtest/latest"
        );
    }

    #[test]
    fn speedtest_metric_parser_accepts_numbers_and_strings() {
        assert_eq!(
            parse_speedtest_metric(Some(&serde_json::json!(631.95))),
            Some(631.95)
        );
        assert_eq!(
            parse_speedtest_metric(Some(&serde_json::json!("387.62"))),
            Some(387.62)
        );
        assert_eq!(
            parse_speedtest_metric(Some(&serde_json::json!("bad"))),
            None
        );
    }

    #[test]
    fn speedtest_latest_success_formats_healthy_message() {
        let health = classify_speedtest_latest(
            SpeedtestLatestResponse {
                message: Some("ok".to_string()),
                data: Some(SpeedtestLatestData {
                    ping: Some(serde_json::json!(3.021)),
                    download: Some(serde_json::json!(631.95)),
                    upload: Some(serde_json::json!("387.62")),
                    failed: Some(false),
                }),
            },
            42,
        );

        assert!(matches!(health.status, HealthStatus::Healthy));
        assert_eq!(
            health.message.as_deref(),
            Some("↓ 631.95 Mbps ↑ 387.62 Mbps • 3.02 ms")
        );
        assert_eq!(health.response_time_ms, Some(42));
    }

    #[test]
    fn speedtest_latest_failed_result_is_degraded() {
        let health = classify_speedtest_latest(
            SpeedtestLatestResponse {
                message: Some("ok".to_string()),
                data: Some(SpeedtestLatestData {
                    ping: Some(serde_json::json!(3.021)),
                    download: Some(serde_json::json!(631.95)),
                    upload: Some(serde_json::json!(387.62)),
                    failed: Some(true),
                }),
            },
            42,
        );

        assert!(matches!(health.status, HealthStatus::Degraded));
        assert_eq!(health.message.as_deref(), Some("Latest speed test failed"));
    }

    #[test]
    fn overall_status_precedence_is_explicit() {
        assert!(matches!(
            determine_overall_status(&[&HealthStatus::Healthy, &HealthStatus::Degraded]),
            HealthStatus::Degraded
        ));
        assert!(matches!(
            determine_overall_status(&[&HealthStatus::Degraded, &HealthStatus::Unhealthy]),
            HealthStatus::Unhealthy
        ));
        assert!(matches!(
            determine_overall_status(&[&HealthStatus::Healthy, &HealthStatus::NotConfigured]),
            HealthStatus::Unknown
        ));
        assert!(matches!(
            determine_overall_status(&[&HealthStatus::Healthy]),
            HealthStatus::Healthy
        ));
    }
}
