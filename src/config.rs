use std::{fs, path::PathBuf};

use anyhow::Result;
use serde::Deserialize;

/// Default number of days to keep each entry in cache
pub const DEFAULT_KEEP_IN_CACHE_DAYS: u16 = 14;

/// Main configuration structure
#[derive(Debug, Clone, Deserialize)]
pub struct CrunchyConfiguration {
    /// Path to input file
    pub input_file_path: Option<PathBuf>,
    /// Path where state JSON file will be written
    pub state_file_path: Option<PathBuf>,
    /// Configuration for GeoIP module
    pub geoip_config: GeoIPConfiguration,
    /// Configuration for Intelligent Peer Sharing module
    pub ips_config: IPSConfiguration,
}

/// Configuration for GeoIP module
#[derive(Debug, Clone, Deserialize)]
pub struct GeoIPConfiguration {
    /// Path to the GeoIP cache
    pub geocache_file_path: PathBuf,
    /// Number of days to keep each entry in cache
    pub keep_in_cache_days: Option<u16>,
    /// Enable IP2Location database
    pub ip2location_enable: bool,
    /// Path to the IP2Location database
    pub ip2location_db_path: Option<PathBuf>,
    /// Enable ipapi.co provider
    pub ipapico_enable: bool,
    /// API key for ipapi.co provider
    pub ipapico_api_key: Option<String>,
    /// Enable ipapi.com provider
    pub ipapicom_enable: bool,
    /// API key for ipapi.com provider
    pub ipapicom_api_key: Option<String>,
}

/// Configuration for Intelligent Peer Sharing module
#[derive(Debug, Clone, Deserialize)]
pub struct IPSConfiguration {
    /// Path where peer list file will be written
    pub peer_file_path: Option<PathBuf>,
    /// Indicates if configuration should be taken into account
    pub use_geolocation: bool,
    /// True means we should prefer closer peers, false means we should prefer farther peers
    pub use_closer_geolocation: bool,
    /// If pref_location_closer is true, this is the maximum distance in kilometers we should prefer
    /// closer peers. If pref_location_closer is false, this is the minimum distance in kilometers we
    /// should prefer farther peers.
    pub geolocation_minmax_distance_km: u32,
    /// Indicates how many peers must be changed for each node
    pub change_at_least: u32,
    /// Indicates maximum peers should be changed for each node
    pub change_no_more: u32,
    /// Weight (importance) of the location factor (used in multi-criteria analysis)
    pub location_weight: f64,
    /// Weight (importance) of the degree factor (used in multi-criteria analysis)
    pub degree_weight: f64,
    /// Weight (importance) of the eigenvector factor (used in multi-criteria analysis)
    pub eigenvector_weight: f64,
    /// Weight (importance) of the betweenness factor (used in multi-criteria analysis)
    pub betweenness_weight: f64,
    /// Weight (importance) of the closeness factor (used in multi-criteria analysis)
    pub closeness_weight: f64,
}

impl CrunchyConfiguration {
    pub fn new(conf_path: &str) -> Result<CrunchyConfiguration> {
        let config_string = fs::read_to_string(conf_path)?;
        let crunchy_config: CrunchyConfiguration = toml::from_str(&config_string)?;
        Ok(crunchy_config)
    }
}

impl Default for CrunchyConfiguration {
    fn default() -> CrunchyConfiguration {
        CrunchyConfiguration {
            input_file_path: Some(PathBuf::from("testdata/sample.json")),
            state_file_path: Some(PathBuf::from("testdata/state.json")),
            ips_config: IPSConfiguration::default(),
            geoip_config: GeoIPConfiguration::default(),
        }
    }
}

impl Default for GeoIPConfiguration {
    fn default() -> GeoIPConfiguration {
        GeoIPConfiguration {
            geocache_file_path: PathBuf::from("testdata/geoip-cache.json"),
            keep_in_cache_days: Some(DEFAULT_KEEP_IN_CACHE_DAYS),
            ip2location_enable: true,
            ip2location_db_path: Some(PathBuf::from("IP2LOCATION-LITE-DB11.BIN")),
            ipapico_enable: true,
            ipapico_api_key: Some(String::from("")),
            ipapicom_enable: true,
            ipapicom_api_key: Some(String::from("")),
        }
    }
}

impl Default for IPSConfiguration {
    fn default() -> IPSConfiguration {
        IPSConfiguration {
            peer_file_path: Some(PathBuf::from("testdata/peers.json")),
            use_geolocation: true,
            use_closer_geolocation: true,
            geolocation_minmax_distance_km: 1000,
            change_at_least: 1,
            change_no_more: 2,
            location_weight: 0.1,
            degree_weight: 0.25,
            eigenvector_weight: 0.25,
            betweenness_weight: 0.25,
            closeness_weight: 0.15,
        }
    }
}
