//! In-process bounded KPI counters.

use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

use crate::AppState;
use std::sync::Arc;

struct Kpi {
    auth_ok: AtomicU64,
    auth_unauthorized: AtomicU64,
    health_checks_total: AtomicU64,
    downloads_queued: AtomicU64,
    downloads_started: AtomicU64,
    downloads_completed: AtomicU64,
    downloads_failed: AtomicU64,
    search_smart: AtomicU64,
    search_indexer: AtomicU64,
    // Phase 2/3: search reliability metrics
    search_budget_queries: AtomicU64,
    search_breaker_opens: AtomicU64,
    search_errors_rate_limit: AtomicU64,
    search_errors_auth: AtomicU64,
}

impl Kpi {
    const fn new() -> Self {
        Self {
            auth_ok: AtomicU64::new(0),
            auth_unauthorized: AtomicU64::new(0),
            health_checks_total: AtomicU64::new(0),
            downloads_queued: AtomicU64::new(0),
            downloads_started: AtomicU64::new(0),
            downloads_completed: AtomicU64::new(0),
            downloads_failed: AtomicU64::new(0),
            search_smart: AtomicU64::new(0),
            search_indexer: AtomicU64::new(0),
            search_budget_queries: AtomicU64::new(0),
            search_breaker_opens: AtomicU64::new(0),
            search_errors_rate_limit: AtomicU64::new(0),
            search_errors_auth: AtomicU64::new(0),
        }
    }
}

static KPI: OnceLock<Kpi> = OnceLock::new();

fn kpi() -> &'static Kpi {
    KPI.get_or_init(Kpi::new)
}

pub fn inc_auth(result: &str) {
    match result {
        "ok" => kpi().auth_ok.fetch_add(1, Ordering::Relaxed),
        _ => kpi().auth_unauthorized.fetch_add(1, Ordering::Relaxed),
    };
}

/// Call from health_status handler on each invocation.
pub fn inc_health_check() {
    kpi().health_checks_total.fetch_add(1, Ordering::Relaxed);
}

/// Call from download task transitions with "queued", "started", "completed", or "failed".
pub fn inc_download(state: &str) {
    match state {
        "queued" => kpi().downloads_queued.fetch_add(1, Ordering::Relaxed),
        "started" => kpi().downloads_started.fetch_add(1, Ordering::Relaxed),
        "completed" => kpi().downloads_completed.fetch_add(1, Ordering::Relaxed),
        _ => kpi().downloads_failed.fetch_add(1, Ordering::Relaxed),
    };
}

/// Call from search handlers with "smart" or "indexer".
pub fn inc_search(kind: &str) {
    match kind {
        "smart" => kpi().search_smart.fetch_add(1, Ordering::Relaxed),
        _ => kpi().search_indexer.fetch_add(1, Ordering::Relaxed),
    };
}

pub fn inc_search_budget_queries(n: u64) {
    kpi().search_budget_queries.fetch_add(n, Ordering::Relaxed);
}

pub fn inc_search_breaker_open() {
    kpi().search_breaker_opens.fetch_add(1, Ordering::Relaxed);
}

pub fn inc_search_error(kind: &str) {
    match kind {
        "rate_limit" => kpi()
            .search_errors_rate_limit
            .fetch_add(1, Ordering::Relaxed),
        "auth" => kpi().search_errors_auth.fetch_add(1, Ordering::Relaxed),
        _ => 0,
    };
}

// ── snapshot endpoint ─────────────────────────────────────────────────────────

#[derive(Serialize)]
struct AuthCounts {
    ok: u64,
    unauthorized: u64,
}

#[derive(Serialize)]
struct DownloadCounts {
    queued: u64,
    started: u64,
    completed: u64,
    failed: u64,
}

#[derive(Serialize)]
struct SearchCounts {
    smart: u64,
    indexer: u64,
}

#[derive(Serialize)]
struct SearchReliabilityCounts {
    budget_queries: u64,
    breaker_opens: u64,
    errors_rate_limit: u64,
    errors_auth: u64,
}

#[derive(Serialize)]
struct KpiSnapshot {
    auth_requests: AuthCounts,
    health_checks_total: u64,
    download_tasks: DownloadCounts,
    search_requests: SearchCounts,
    search_reliability: SearchReliabilityCounts,
}

async fn get_kpi() -> Json<KpiSnapshot> {
    let k = kpi();
    Json(KpiSnapshot {
        auth_requests: AuthCounts {
            ok: k.auth_ok.load(Ordering::Relaxed),
            unauthorized: k.auth_unauthorized.load(Ordering::Relaxed),
        },
        health_checks_total: k.health_checks_total.load(Ordering::Relaxed),
        download_tasks: DownloadCounts {
            queued: k.downloads_queued.load(Ordering::Relaxed),
            started: k.downloads_started.load(Ordering::Relaxed),
            completed: k.downloads_completed.load(Ordering::Relaxed),
            failed: k.downloads_failed.load(Ordering::Relaxed),
        },
        search_requests: SearchCounts {
            smart: k.search_smart.load(Ordering::Relaxed),
            indexer: k.search_indexer.load(Ordering::Relaxed),
        },
        search_reliability: SearchReliabilityCounts {
            budget_queries: k.search_budget_queries.load(Ordering::Relaxed),
            breaker_opens: k.search_breaker_opens.load(Ordering::Relaxed),
            errors_rate_limit: k.search_errors_rate_limit.load(Ordering::Relaxed),
            errors_auth: k.search_errors_auth.load(Ordering::Relaxed),
        },
    })
}

/// Router mounted at /api/kpi (no auth required — aggregates only, no PII).
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(get_kpi))
}
