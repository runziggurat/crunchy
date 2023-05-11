use std::{
    collections::HashMap,
    fs, io,
    net::IpAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use ziggurat_core_geoip::{
    geoip::{GeoIPService, GeoInfo},
    providers::{
        ip2loc::Ip2LocationService,
        ipgeoloc::{BackendProvider, IpGeolocateService},
    },
};

use crate::config::{GeoIPConfiguration, DEFAULT_KEEP_IN_CACHE_DAYS};

#[derive(Clone, Serialize, Deserialize)]
struct CachedIp {
    pub last_updated: SystemTime,
    pub info: GeoInfo,
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct GeoCache {
    pub entries: HashMap<IpAddr, CachedIp>,
}

/// GeoIP cache responsible for getting and caching results.
pub struct GeoIPCache {
    /// Available providers and their configuration.
    providers: Vec<Box<dyn GeoIPService>>,
    /// Path to the cache file.
    cache_file: PathBuf,
    /// Cache entries.
    cache: Arc<RwLock<GeoCache>>,
    /// How many days to keep the cache entries.
    keep_in_cache_days: u16,
}

impl GeoIPCache {
    /// Create a new GeoIP cache.
    pub fn new(config: &GeoIPConfiguration) -> Self {
        Self {
            providers: Vec::new(),
            cache_file: config.geocache_file_path.clone(),
            cache: Arc::new(RwLock::new(GeoCache::default())),
            keep_in_cache_days: config
                .keep_in_cache_days
                .unwrap_or(DEFAULT_KEEP_IN_CACHE_DAYS),
        }
    }

    /// Add a new provider to the list of providers. The providers will be called in the order they
    /// are added.
    pub fn add_provider(&mut self, provider: Box<dyn GeoIPService>) {
        self.providers.push(provider);
    }

    /// Load the cache from the file.
    pub async fn load(&self) -> Result<(), io::Error> {
        let cache_string = fs::read_to_string(&self.cache_file)?;

        let mut cache = self.cache.write().await;
        cache.entries = serde_json::from_str(&cache_string).unwrap();
        Ok(())
    }

    /// Save the cache to the file.
    pub async fn save(&self) -> Result<(), io::Error> {
        let cache = self.cache.read().await;
        let cache_string = serde_json::to_string(&cache.entries).unwrap();
        fs::write(&self.cache_file, cache_string)
    }

    /// Function look in cache and if not found, it will call the providers to fetch new data and
    /// store it into cache.
    pub async fn lookup(&self, ip: IpAddr) -> Option<GeoInfo> {
        if let Some(info) = self.check_cache(ip).await {
            return Some(info);
        }

        for provider in self.providers.iter() {
            let entry = provider.lookup(ip).await;
            if let Ok(ip_geo_info) = entry {
                let mut rw_cache = self.cache.write().await;
                let cache_entry = CachedIp {
                    last_updated: SystemTime::now(),
                    info: ip_geo_info.geo_info,
                };
                rw_cache.entries.insert(ip, cache_entry.clone());
                return Some(cache_entry.info);
            }
        }

        None
    }

    async fn check_cache(&self, ip: IpAddr) -> Option<GeoInfo> {
        let mut remove_entry = false;
        {
            let cache = self.cache.read().await;
            let res = cache.entries.get(&ip);
            if let Some(entry) = res {
                // Check if the entry is not too old.
                if entry.last_updated.elapsed().unwrap()
                    < Duration::from_secs(60 * 60 * 24 * self.keep_in_cache_days as u64)
                {
                    return Some(entry.info.clone());
                }
                remove_entry = true;
            }
        }

        if remove_entry {
            let mut rw_cache = self.cache.write().await;
            rw_cache.entries.remove(&ip);
        }

        None
    }

    /// Configure the providers based on the configuration.
    pub fn configure_providers(&mut self, config: &GeoIPConfiguration) {
        if config.ip2location_enable {
            let ipv6db = config
                .ip2location_ipv6_db_path
                .as_ref()
                .map(|path| path.as_path().display().to_string());

            self.add_provider(Box::new(Ip2LocationService::new(
                config
                    .ip2location_db_path
                    .as_ref()
                    .unwrap()
                    .to_str()
                    .unwrap(),
                ipv6db,
            )));
        }

        if config.ipapico_enable {
            self.add_provider(Box::new(IpGeolocateService::new(
                BackendProvider::IpApiCo,
                config.ipapico_api_key.as_ref().unwrap().as_str(),
            )));
        }

        if config.ipapicom_enable {
            self.add_provider(Box::new(IpGeolocateService::new(
                BackendProvider::IpApiCom,
                config.ipapicom_api_key.as_ref().unwrap().as_str(),
            )));
        }
    }
}
