//! *arr Integration Module
//!
//! Provides bi-directional sync with Sonarr and Radarr.
//! Flasharr uses these to replace Sonarr/Radarr/Seerr UI entirely.

pub mod artifact_manager;
pub mod client;

pub use artifact_manager::{ArrArtifactManager, ArtifactStatus};
pub use client::{
    ArrClient,
    // Shared types
    DiskSpace,
    HealthCheck,
    // Radarr types
    RadarrMovie,
    SonarrCalendarEntry,
    SonarrEpisode,
    // Sonarr types
    SonarrSeries,
};
