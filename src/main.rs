mod config;
mod geoip_cache;
use clap::Parser;

use crate::geoip_cache::GeoIPCache;

use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::config::CrunchyConfiguration;
use serde::{Deserialize, Serialize};
use spectre::{graph::AGraph, graph::Graph};
use ziggurat_core_geoip::geoip::GeoInfo;
use ziggurat_core_geoip::providers::ip2loc::Ip2LocationService;
use ziggurat_core_geoip::providers::ipgeoloc::{BackendProvider, IpGeolocateService};

//TODO(asmie): there is some redundancy here as we have ip in the Node structure and one more time
// in the GeoIPInfo structure.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Node {
    ip: String,
    betweenness: f64,
    closeness: f64,
    num_connections: usize,
    geolocation: Option<GeoInfo>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CrunchyState {
    agraph_length: usize,
    elapsed: f64,
    nodes: Vec<Node>,
    min_betweenness: f64,
    max_betweenness: f64,
    min_closeness: f64,
    max_closeness: f64,
}

#[allow(dead_code)]
#[derive(Default, Deserialize)]
pub struct NetworkSummary {
    num_known_nodes: usize,
    num_good_nodes: usize,
    num_known_connections: usize,
    num_versions: usize,
    protocol_versions: HashMap<u32, usize>,
    user_agents: HashMap<String, usize>,
    crawler_runtime: Duration,
    node_ips: Vec<String>,
    agraph: AGraph,
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

//TODO(asmie): this NEED to be refactorized as currently it is method-level smell (too long)
// doing too many things. Especially, when I'd like to add here some other stuff like peer sharing it would
// be too messy. It should be re-designed and divided into smaller functions with appropriate names and
// functionalities (like computing graphs, counting factors, geolocalization etc).
async fn write_state(config: &CrunchyConfiguration) {
    let mut geo_cache = GeoIPCache::new(&config.geoip_config);
    let response = load_response(config.input_file_path.as_ref().unwrap().to_str().unwrap());
    let agraph = response.result.agraph;
    let graph: Graph<usize> = Graph::new();
    let start = Instant::now();
    let (betweenness, closeness) = graph.compute_betweenness_and_closeness(&agraph);
    let elapsed = start.elapsed();
    let mut min_betweenness: f64 = 10000.0;
    let mut max_betweenness: f64 = 0.0;
    let mut min_closeness: f64 = 10000.0;
    let mut max_closeness: f64 = 0.0;

    let res = geo_cache.load().await;
    if res.is_err() {
        println!("No cache file to load! Will be created one.");
    }

    if config.geoip_config.ip2location_enable {
        geo_cache.add_provider(Box::new(Ip2LocationService::new(
            config
                .geoip_config
                .ip2location_db_path
                .as_ref()
                .unwrap()
                .to_str()
                .unwrap(),
        )));
    }

    if config.geoip_config.ipapico_enable {
        geo_cache.add_provider(Box::new(IpGeolocateService::new(
            BackendProvider::IpApiCo,
            config
                .geoip_config
                .ipapico_api_key
                .as_ref()
                .unwrap()
                .as_str(),
        )));
    }

    if config.geoip_config.ipapicom_enable {
        geo_cache.add_provider(Box::new(IpGeolocateService::new(
            BackendProvider::IpApiCom,
            config
                .geoip_config
                .ipapicom_api_key
                .as_ref()
                .unwrap()
                .as_str(),
        )));
    }

    for between in &betweenness {
        if *between < min_betweenness {
            min_betweenness = *between;
        }
        if *between > max_betweenness {
            max_betweenness = *between;
        }
    }
    for close in &closeness {
        if *close < min_closeness {
            min_closeness = *close;
        }
        if *close > max_closeness {
            max_closeness = *close;
        }
    }
    let mut nodes = Vec::with_capacity(agraph.len());
    for i in 0..agraph.len() {
        let node: Node = Node {
            ip: response.result.node_ips[i].clone(),
            betweenness: betweenness[i],
            closeness: closeness[i],
            num_connections: agraph[i].len(),
            geolocation: geo_cache
                .lookup(
                    response.result.node_ips[i]
                        .parse()
                        .expect("malformed IP address"),
                )
                .await,
        };
        nodes.push(node);
    }

    let state = CrunchyState {
        agraph_length: agraph.len(),
        elapsed: elapsed.as_secs_f64(),
        nodes,
        min_betweenness,
        max_betweenness,
        min_closeness,
        max_closeness,
    };

    // Save all changes done to the cache
    // TODO(asmie): better error handling - after refactorization of this function
    geo_cache.save().await.expect("could not save geoip cache");

    let joutput = serde_json::to_string(&state).unwrap();
    fs::write(config.state_file_path.as_ref().unwrap(), joutput).unwrap();
}

#[tokio::main]
async fn main() {
    let arg_conf = ArgConfiguration::parse();
    let mut configuration: CrunchyConfiguration;

    if let Some(config_file_path) = arg_conf.config_file {
        configuration = CrunchyConfiguration::new(&config_file_path)
            .expect("configuration file could not be loaded or parsed");
    } else {
        configuration = CrunchyConfiguration::default();
    }

    // Override configuration with command line arguments if provided
    if let Some(input_sample) = arg_conf.input_sample {
        configuration.input_file_path = Some(PathBuf::from(input_sample));
    }
    if let Some(out_state) = arg_conf.out_state {
        configuration.state_file_path = Some(PathBuf::from(out_state));
    }
    if let Some(geocache_file) = arg_conf.geocache_file {
        configuration.geoip_config.geocache_file_path = PathBuf::from(geocache_file);
    }

    if fs::metadata(configuration.input_file_path.as_ref().unwrap()).is_err() {
        println!(
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
    #[clap(short, long)]
    pub input_sample: Option<String>,
    /// Output file with state of the graph (overrides output from config file)
    #[clap(short, long)]
    pub out_state: Option<String>,
    /// Output file with geolocation cache (overrides cache from config file)
    #[clap(short, long)]
    pub geocache_file: Option<String>,
    /// Configuration file path (if none defaults will be assumed)
    #[clap(short, long)]
    pub config_file: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let size: usize = 2531;
        assert_eq!(state.agraph_length, size);
        assert_eq!(state.nodes.len(), size);
        assert!((state.min_betweenness - 0.0).abs() < f64::EPSILON);
        assert!((state.max_betweenness - 0.0006471174062683313).abs() < f64::EPSILON);
        assert!((state.min_closeness - 1.9965036212494205).abs() < f64::EPSILON);
        assert!((state.max_closeness - 2.9965618988763065).abs() < f64::EPSILON);
        let node = state.nodes[5].clone();
        assert_eq!(node.ip, "38.242.199.182");
        assert_eq!(node.num_connections, 378);
        assert!((node.betweenness - 0.000244483600836513).abs() < f64::EPSILON);
        assert!((node.closeness - 2.0013493455674).abs() < f64::EPSILON);
    }
}
