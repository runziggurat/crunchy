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
    use super::*;

    #[ignore = "must update data"]
    #[tokio::test]
    async fn test_state_output() {
        let configuration = CrunchyConfiguration::default();
        let _ = fs::remove_file(configuration.state_file_path.as_ref().unwrap());
        write_state(&configuration).await;
        let state = load_state(
            configuration
                .state_file_path
                .as_ref()
                .unwrap()
                .to_str()
                .unwrap(),
        );
        let size: usize = 2472;
        assert_eq!(state.nodes.len(), size);
        let node = state.nodes[5].clone();
        assert_eq!(node.addr.to_string(), "95.216.80.108:16125");
        assert_eq!(node.connections.len(), 372);
        assert!((node.betweenness - 0.00022429039726952488).abs() < f64::EPSILON);
        assert!((node.closeness - 1.998241968994726).abs() < f64::EPSILON);
    }
}
