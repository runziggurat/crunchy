mod config;
mod constants;
mod geoip_cache;
mod histogram;
mod ips;
mod nodes;

use std::{fs, path::PathBuf, time::Instant};

use clap::Parser;
use serde::{Deserialize, Serialize};
use ziggurat_core_crawler::summary::{NetworkSummary, NetworkType};

use crate::{
    config::CrunchyConfiguration,
    geoip_cache::GeoIPCache,
    ips::algorithm::Ips,
    nodes::{create_histograms, create_nodes, HistogramSummary, Node},
};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CrunchyState {
    elapsed: f64,
    nodes: Vec<Node>,
    histograms: Vec<HistogramSummary>,
}

#[allow(dead_code)]
#[derive(Default, Deserialize)]
pub struct JsonRpcResponse {
    jsonrpc: String,
    result: NetworkSummary,
    id: usize,
}

pub fn load_response(filepath: &str) -> JsonRpcResponse {
    let jstring = fs::read_to_string(filepath).expect("could not open response file");
    serde_json::from_str(&jstring).unwrap()
}

pub fn load_state(filepath: &str) -> CrunchyState {
    let jstring = fs::read_to_string(filepath).expect("could not open state file");
    serde_json::from_str(&jstring).unwrap()
}

/// Perform all the necessary steps to generate the state file and the peer list.
async fn write_state(config: &CrunchyConfiguration) {
    let mut geo_cache = GeoIPCache::new(&config.geoip_config);
    let response = load_response(config.input_file_path.as_ref().unwrap().to_str().unwrap());
    let start = Instant::now();
    let elapsed = start.elapsed();

    let res = geo_cache.load().await;
    if res.is_err() {
        println!("No cache file to load! Will be created one.");
    }

    geo_cache.configure_providers(&config.geoip_config);

    let nodes = create_nodes(
        config.network_type_filter,
        &response.result.nodes_indices,
        &response.result.node_addrs,
        &response.result.node_network_types,
        &geo_cache,
    )
    .await;

    let histograms = create_histograms(&nodes).await;

    let state = CrunchyState {
        elapsed: elapsed.as_secs_f64(),
        nodes,
        histograms,
    };

    // Save all changes done to the cache
    if let Err(res) = geo_cache.save().await {
        println!("Could not save cache file: {}", res);
    }

    let mut ips = Ips::new(config.ips_config.clone());
    let ips_peers = ips.generate(&state, NetworkType::Zcash).await;

    let peerlist = serde_json::to_string(&ips_peers).unwrap();
    fs::write(config.ips_config.peer_file_path.as_ref().unwrap(), peerlist).unwrap();

    let joutput = serde_json::to_string(&state).unwrap();
    fs::write(config.state_file_path.as_ref().unwrap(), joutput).unwrap();
}

#[tokio::main]
async fn main() {
    let arg_conf = ArgConfiguration::parse();
    let mut configuration = arg_conf
        .config_file
        .map(|path| {
            CrunchyConfiguration::new(path.to_str().unwrap())
                .expect("could not load configuration file")
        })
        .unwrap_or_default();

    // Override configuration with command line arguments if provided
    if let Some(input_file) = arg_conf.input_sample {
        configuration.input_file_path = Some(input_file);
    }
    if let Some(state_file) = arg_conf.out_state {
        configuration.state_file_path = Some(state_file);
    }
    if let Some(geocache_file) = arg_conf.geocache_file {
        configuration.geoip_config.geocache_file_path = geocache_file;
    }
    if arg_conf.ips_file.is_some() {
        configuration.ips_config.peer_file_path = arg_conf.ips_file;
    }

    // Check if user error setting optional filter type
    if arg_conf.filter_type.is_some() && arg_conf.filter_type.unwrap() == NetworkType::Invalid {
        panic!("Invalid network type for filter. Check Readme for possible values.")
    }

    configuration.network_type_filter = arg_conf.filter_type;

    if !configuration.input_file_path.as_ref().unwrap().is_file() {
        eprintln!(
            "{}: No such file or directory",
            configuration
                .input_file_path
                .as_ref()
                .unwrap()
                .to_str()
                .unwrap()
        );
        return;
    }
    write_state(&configuration).await;
}

#[derive(Parser, Debug)]
#[clap(author = "Ziggurat Team", version, about, long_about = None)]
pub struct ArgConfiguration {
    /// Input file with sample data to process (overrides input from config file)
    #[clap(short, long, value_parser)]
    pub input_sample: Option<PathBuf>,
    /// Output file with state of the graph (overrides output from config file)
    #[clap(short, long, value_parser)]
    pub out_state: Option<PathBuf>,
    /// Output file with geolocation cache (overrides cache from config file)
    #[clap(short, long, value_parser)]
    pub geocache_file: Option<PathBuf>,
    /// Configuration file path (if none defaults will be assumed)
    #[clap(short, long, value_parser)]
    pub config_file: Option<PathBuf>,
    /// Optional node filtering parameter; consult Readme for possible values
    #[clap(short, long, value_parser)]
    pub filter_type: Option<NetworkType>,
    /// Intelligent Peer Sharing output file path (overrides output from config file)
    #[clap(short = 'p', long, value_parser)]
    pub ips_file: Option<PathBuf>,
}

#[cfg(test)]
mod tests {

    use std::net::SocketAddr;

    use super::*;
    use crate::config::GeoIPConfiguration;

    #[tokio::test]
    async fn create_nodes_unfiltered_test() {
        let response = load_response("testdata/sample.json");

        let config = GeoIPConfiguration::default();
        let mut geo_cache = GeoIPCache::new(&config);
        geo_cache.configure_providers(&config);

        let nodes = create_nodes(
            None,
            &response.result.nodes_indices,
            &response.result.node_addrs,
            &response.result.node_network_types,
            &geo_cache,
        )
        .await;

        assert_eq!(nodes.len(), 6103);
        assert_eq!(nodes[0].connections.len(), 2478);
        assert_eq!(nodes[1].connections.len(), 2216);
        assert_eq!(nodes[2].connections.len(), 1);
        assert_eq!(nodes[3].connections.len(), 2184);
        assert_eq!(nodes[3].connections[2], 609);
    }

    #[tokio::test]
    async fn create_nodes_filtered_test1() {
        //    let response = load_response(config.input_file_path.as_ref().unwrap().to_str().
        let indices = vec![vec![1, 2], vec![0, 2, 3], vec![0, 1, 3], vec![1, 2]];
        let node_addrs = vec![
            SocketAddr::from(([127, 0, 0, 1], 1234)),
            SocketAddr::from(([127, 0, 0, 2], 1234)),
            SocketAddr::from(([127, 0, 0, 3], 1234)),
            SocketAddr::from(([127, 0, 0, 4], 1234)),
        ];
        let node_network_types = vec![
            NetworkType::Unknown,
            NetworkType::Zcash,
            NetworkType::Unknown,
            NetworkType::Zcash,
        ];
        let config = GeoIPConfiguration::default();
        let mut geo_cache = GeoIPCache::new(&config);
        geo_cache.configure_providers(&config);
        let nodes = create_nodes(
            Some(NetworkType::Zcash),
            &indices,
            &node_addrs,
            &node_network_types,
            &geo_cache,
        )
        .await;
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].connections, vec![1]);
        assert_eq!(nodes[1].connections, vec![0]);
    }

    #[tokio::test]
    async fn create_nodes_filtered_test2() {
        let response = load_response("testdata/sample.json");

        let config = GeoIPConfiguration::default();
        let mut geo_cache = GeoIPCache::new(&config);
        geo_cache.configure_providers(&config);

        let nodes = create_nodes(
            Some(NetworkType::Zcash),
            &response.result.nodes_indices,
            &response.result.node_addrs,
            &response.result.node_network_types,
            &geo_cache,
        )
        .await;
        assert_eq!(nodes.len(), 122);
        assert_eq!(nodes[0].connections.len(), 2);
        assert_eq!(nodes[1].connections.len(), 0);
        assert_eq!(nodes[2].connections.len(), 1);
        assert_eq!(nodes[3].connections.len(), 1);
        assert_eq!(nodes[3].connections[0], 56);

        let node = nodes[0].clone();
        assert_eq!(node.addr.to_string(), "3.72.134.66:8233");
        let epsilon: f64 = 0.0000001;
        assert!((node.betweenness - 47.525898078529664).abs() < epsilon);
        assert!((node.closeness - 1.603305785123967).abs() < epsilon);
    }
}
