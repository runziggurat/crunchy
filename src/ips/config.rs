use std::path::PathBuf;

use serde::Deserialize;

use crate::config::GeoLocationMode;

/// Multi-criteria analysis weights
#[derive(Debug, Clone, Deserialize)]
pub struct MultiCriteriaAnalysisWeights {
    /// Weight (importance) of the location factor
    pub location: f64,
    /// Weight (importance) of the degree factor
    pub degree: f64,
    /// Weight (importance) of the eigenvector factor
    pub eigenvector: f64,
    /// Weight (importance) of the betweenness factor
    pub betweenness: f64,
    /// Weight (importance) of the closeness factor
    pub closeness: f64,
}

/// Configuration for Intelligent Peer Sharing module
#[derive(Debug, Clone, Deserialize)]
pub struct IPSConfiguration {
    /// Path where peer list file will be written
    pub peer_file_path: Option<PathBuf>,
    /// Path where log file will be written (if none, all logs will be written to stdout)
    pub log_path: Option<PathBuf>,
    /// Indicates if configuration should be taken into account and if so what should be
    /// preferred (closer or distant).
    pub geolocation: GeoLocationMode,
    /// This is the max (or min) distance in km between peers
    pub geolocation_minmax_distance_km: u32,
    /// Indicates how many peers must be changed for each node
    pub change_at_least: u32,
    /// Indicates maximum peers should be changed for each node
    pub change_no_more: u32,
    /// Indicates adjustment factor for bridge detection
    pub bridge_threshold_adjustment: f64,
    /// Multi-criteria analysis weights
    pub mcda_weights: MultiCriteriaAnalysisWeights,
    /// If set, vanilla (original, before IPS) peer list should be generated in the specified file
    pub vanilla_peer_file_path: Option<PathBuf>,
}

impl Default for IPSConfiguration {
    fn default() -> IPSConfiguration {
        IPSConfiguration {
            peer_file_path: Some(PathBuf::from("testdata/peers.json")),
            log_path: None,
            geolocation: GeoLocationMode::PreferCloser,
            geolocation_minmax_distance_km: 1000,
            change_at_least: 1,
            change_no_more: 2,
            mcda_weights: MultiCriteriaAnalysisWeights::default(),
            bridge_threshold_adjustment: 1.25,
            vanilla_peer_file_path: None,
        }
    }
}

impl Default for MultiCriteriaAnalysisWeights {
    fn default() -> MultiCriteriaAnalysisWeights {
        MultiCriteriaAnalysisWeights {
            location: 0.3,
            degree: 0.25,
            eigenvector: 0.1,
            betweenness: 0.25,
            closeness: 0.1,
        }
    }
}
