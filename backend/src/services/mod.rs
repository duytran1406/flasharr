//! Services Layer
//!
//! Business logic abstraction between API handlers and data layer.
//! Services module - Business logic layer
//!
//! Contains service abstractions that sit between API handlers and data layer.

pub mod download_service;
pub mod tmdb_service;

pub use download_service::DownloadService;
pub use tmdb_service::TmdbService;
