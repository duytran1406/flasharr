use crate::downloader::DownloadState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub struct TransferMetrics {
    pub downloaded: u64,
    pub previous_downloaded: Option<u64>,
    pub progress: f64,
    pub dlspeed: i64,
    pub upspeed: i64,
    pub amount_left: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QbitClassification {
    pub task_state: DownloadState,
    pub transfer_state: String,
    pub transfer_active: bool,
}

pub fn classify_qbit(
    state: &str,
    metrics: TransferMetrics,
    sync_health: &str,
) -> QbitClassification {
    let s = state.trim().to_ascii_lowercase();
    let healthy = sync_health.eq_ignore_ascii_case("healthy");
    let delta = metrics
        .previous_downloaded
        .map(|v| metrics.downloaded > v)
        .unwrap_or(false);
    let progress_evidence =
        metrics.progress > 0.0 && metrics.progress < 1.0 && metrics.amount_left > 0;
    let active = healthy && (metrics.dlspeed > 0 || delta || progress_evidence);
    let complete = metrics.progress >= 1.0 || metrics.amount_left == 0 && metrics.downloaded > 0;

    let (task_state, transfer_state, transfer_active) = if !healthy {
        (DownloadState::Starting, "STALE", false)
    } else if matches!(s.as_str(), "error" | "missingfiles") {
        (DownloadState::Failed, "BLOCKED", false)
    } else if matches!(s.as_str(), "pauseddl" | "stoppeddl") {
        (DownloadState::Paused, "PAUSED", false)
    } else if complete
        && matches!(
            s.as_str(),
            "completed" | "uploading" | "stalledup" | "pausedup" | "stoppedup" | "forcedup"
        )
    {
        (
            DownloadState::Completed,
            if metrics.upspeed > 0 {
                "SEEDING"
            } else {
                "COMPLETED"
            },
            metrics.upspeed > 0,
        )
    } else if matches!(s.as_str(), "downloading" | "forceddl") && active {
        (DownloadState::Downloading, "DOWNLOADING", true)
    } else if matches!(
        s.as_str(),
        "metadl" | "checkingdl" | "checkingresumedata" | "allocating" | "moving"
    ) {
        (DownloadState::Starting, "STARTING", false)
    } else if matches!(
        s.as_str(),
        "queueddl" | "stalleddl" | "downloading" | "forceddl"
    ) {
        (
            DownloadState::Queued,
            if s == "stalleddl" {
                "STALLED"
            } else {
                "WAITING"
            },
            false,
        )
    } else {
        (DownloadState::Starting, "UNKNOWN", false)
    };
    QbitClassification {
        task_state,
        transfer_state: transfer_state.into(),
        transfer_active,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn m(dlspeed: i64, downloaded: u64, progress: f64, left: i64) -> TransferMetrics {
        TransferMetrics {
            downloaded,
            previous_downloaded: None,
            progress,
            dlspeed,
            upspeed: 0,
            amount_left: left,
        }
    }
    #[test]
    fn taxonomy_matrix_is_fail_safe() {
        for state in [
            "metaDL",
            "checkingDL",
            "checkingResumeData",
            "allocating",
            "moving",
            "queuedDL",
            "stalledDL",
            "unknown",
        ] {
            let c = classify_qbit(state, m(0, 0, 0.0, 100), "healthy");
            assert_ne!(c.task_state, DownloadState::Downloading, "{state}");
            assert!(!c.transfer_active);
        }
        let idle = classify_qbit("downloading", m(0, 0, 0.0, 100), "healthy");
        assert_eq!(idle.task_state, DownloadState::Queued);
        let active = classify_qbit("forcedDL", m(1, 0, 0.0, 100), "healthy");
        assert_eq!(active.task_state, DownloadState::Downloading);
        assert!(active.transfer_active);
        let stale = classify_qbit("downloading", m(10, 20, 0.2, 80), "stale");
        assert!(!stale.transfer_active);
        assert_ne!(stale.task_state, DownloadState::Downloading);
    }

    #[test]
    fn active_requires_current_evidence() {
        let historical = classify_qbit("downloading", m(0, 42, 0.0, 100), "healthy");
        assert_eq!(historical.task_state, DownloadState::Queued);
        assert!(!historical.transfer_active);
        let progress = classify_qbit("downloading", m(0, 42, 0.5, 50), "healthy");
        let delta = classify_qbit(
            "downloading",
            TransferMetrics {
                previous_downloaded: Some(41),
                ..m(0, 42, 0.0, 100)
            },
            "healthy",
        );
        assert!(progress.transfer_active && delta.transfer_active);
    }
}
