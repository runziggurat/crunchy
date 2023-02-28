use std::{fs, path::PathBuf};

use anyhow::Result;
use serde::Deserialize;

use crate::ips::config::IPSConfiguration;

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

/// GeoLocationMode enum
#[derive(Debug, PartialEq, Clone, Deserialize)]
pub enum GeoLocationMode {
    Off,
    PreferCloser,
    PreferDistant,
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
