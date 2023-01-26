use std::{fs, path::{PathBuf}};

use anyhow::Result;
use serde::Deserialize;
use toml;

/// Main configuration structure
#[derive(Default, Debug, Clone, Deserialize)]
pub struct CrunchyConfiguration {
    /// Path to input file
    pub input_file_path: PathBuf,
    /// Path where state JSON file will be written
    pub state_file_path: PathBuf,
    /// Configuration for Intelligent Peer Sharing module
    pub ips_config: IPSConfiguration,
    /// Configuration for GeoIP module
    pub geoip_config: GeoIPConfiguration,
}

/// Configuration for GeoIP module
#[derive(Default, Debug, Clone, Deserialize)]
pub struct GeoIPConfiguration {
    /// Path to the GeoIP cache
    pub geocache_file_path: PathBuf,
    /// Number of days to keep each entry in cache
    pub keep_in_cache_days: u16
}

/// Configuration for Intelligent Peer Sharing module
#[derive(Default, Debug, Clone, Deserialize)]
pub struct IPSConfiguration {

}

impl CrunchyConfiguration {
    pub fn load(conf_path: &'static str) -> Result<CrunchyConfiguration> {
        let config_string = fs::read_to_string(conf_path)?;
        let crunchy_config: CrunchyConfiguration = toml::from_str(&config_string)?;
        Ok(crunchy_config)
    }
}