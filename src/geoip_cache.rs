use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use std::{collections::HashMap, fs, io, time::Duration};
use tokio::sync::RwLock;

use ziggurat_core_geoip::geoip::{GeoIPInfo, GeoIPService};

#[derive(Clone, Serialize, Deserialize)]
struct CacheEntry {
    pub last_updated: SystemTime,
    pub entry: GeoIPInfo,
}

#[derive(Clone, Serialize, Deserialize)]
struct GeoCache {
    pub entries: HashMap<IpAddr, CacheEntry>,
}

/// GeoIP cache responsible for getting and caching results.
pub struct GeoIPCache {
    /// Available providers and their configuration.
    providers: Vec<Box<dyn GeoIPService>>,
    /// Path to the cache file.
    cache_file: PathBuf,
    /// Cache entries.
    cache: Arc<RwLock<GeoCache>>,
}

impl GeoIPCache {
    /// Create a new GeoIP cache.
    pub fn new(cache_file: &Path) -> Self {
        Self {
            providers: Vec::new(),
            cache_file: cache_file.to_owned(),
            cache: Arc::new(RwLock::new(GeoCache::new())),
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
    pub async fn lookup(&self, ip: IpAddr) -> Option<GeoIPInfo> {
        let mut remove_entry = false;

        {
            let cache = self.cache.read().await;
            let res = cache.entries.get(&ip);
            if let Some(entry) = res {
                // Check if the entry is not too old.
                if entry.last_updated.elapsed().unwrap() < Duration::from_secs(60 * 60 * 24 * 14) {
                    return Some(entry.entry.clone());
                } else {
                    remove_entry = true;
                }
            }
        }

        {
            // Beeing here means that we didn't find the entry in cache.
            let mut rw_cache = self.cache.write().await;

            if remove_entry {
                rw_cache.entries.remove(&ip);
            }

            for provider in self.providers.iter() {
                let entry = provider.lookup(ip).await;
                if let Ok(entry) = entry {
                    rw_cache.entries.insert(
                        ip,
                        CacheEntry {
                            last_updated: SystemTime::now(),
                            entry,
                        },
                    );
                    return Some(rw_cache.entries.get(&ip).unwrap().entry.clone());
                }
            }
        }

        None
    }
}

impl GeoCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }
}
