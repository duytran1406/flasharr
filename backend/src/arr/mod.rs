//! *arr Integration Module
//!
//! Provides bi-directional sync with Sonarr and Radarr.
//! Enables downloads from Flasharr UI to trigger automatic imports in *arr applications.

pub mod client;

pub use client::ArrClient;
