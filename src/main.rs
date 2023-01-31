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
    column_position: u32,
    column_size: u32,
    connections: Vec<usize>,
    geolocation: Option<GeoInfo>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CrunchyState {
    agraph_length: usize,
    elapsed: f64,
    nodes: Vec<Node>,
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


pub fn compute_columns(nodes: &mut Vec<Node>) -> HashMap<String,u32> {
    let mut column_stats: HashMap<String,u32> = HashMap::new();
    for node in nodes {
        if let Some(geoinfo) = &node.geolocation {
            if let Some(latitude) = geoinfo.latitude {
                if let Some(longitude) = geoinfo.longitude {
                    let ilatitude: i32 = (latitude * 5.0).floor() as i32;
                    let ilongitude: i32 = (longitude * 5.0).floor() as i32;
                    let geostr =  format!("{}:{}", ilatitude, ilongitude);
                    column_stats.entry(geostr.clone()).and_modify(|count| *count += 1).or_insert(1);
                    node.column_position = column_stats[&geostr];
                }
            }
        }
    }
    column_stats
}

pub fn set_column_positions(nodes: &mut Vec<Node>, column_stats: &mut HashMap<String,u32>) {
    for node in nodes {
        if let Some(geoinfo) = &node.geolocation {
            if let Some(latitude) = geoinfo.latitude {
                if let Some(longitude) = geoinfo.longitude {
                    let ilatitude: i32 = (latitude * 5.0).floor() as i32;
                    let ilongitude: i32 = (longitude * 5.0).floor() as i32;
                    let geostr = format!("{}:{}", ilatitude, ilongitude);
                    if let Some(count) = column_stats.get(&geostr) {
                        node.column_size = *count;
                    }
                }
            }
        }
    }
}


async fn create_nodes(network_summary: &NetworkSummary, geo_cache: &GeoIPCache) -> Vec<Node> {
    let graph: Graph<usize> = Graph::new();
    let agraph: &Vec<Vec<usize>> = &network_summary.agraph;
    let (betweenness, closeness) = graph.compute_betweenness_and_closeness(&agraph);
    let mut nodes = Vec::with_capacity(agraph.len());
    for i in 0..agraph.len() {
        let node: Node = Node {
            ip: network_summary.node_ips[i].clone(),
            betweenness: betweenness[i],
            closeness: closeness[i],
            column_position: 0,
            column_size: 0,
            connections: agraph[i].clone(),
            geolocation: geo_cache
                .lookup(
                    network_summary.node_ips[i]
                        .parse()
                        .expect("malformed IP address"),
                )
                .await,
        };
        nodes.push(node);
    }

    let mut column_stats = compute_columns(&mut nodes);
    set_column_positions(&mut nodes, &mut column_stats);
    nodes
}



//TODO(asmie): this NEED to be refactorized as currently it is method-level smell (too long)
// doing too many things. Especially, when I'd like to add here some other stuff like peer sharing it would
// be too messy. It should be re-designed and divided into smaller functions with appropriate names and
// functionalities (like computing graphs, counting factors, geolocalization etc).
async fn write_state(config: &CrunchyConfiguration) {
    let mut geo_cache = GeoIPCache::new(&config.geoip_config);
    let response = load_response(config.input_file_path.as_ref().unwrap().to_str().unwrap());
    let start = Instant::now();
    let elapsed = start.elapsed();

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

    let nodes = create_nodes(&response.result, &geo_cache).await;

    let state = CrunchyState {
        agraph_length: response.result.agraph.len(),
        elapsed: elapsed.as_secs_f64(),
        nodes
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
        // assert!((state.min_betweenness - 0.0).abs() < f64::EPSILON);
        // assert!((state.max_betweenness - 0.0006471174062683313).abs() < f64::EPSILON);
        // assert!((state.min_closeness - 1.9965036212494205).abs() < f64::EPSILON);
        // assert!((state.max_closeness - 2.9965618988763065).abs() < f64::EPSILON);
        let node = state.nodes[5].clone();
        assert_eq!(node.ip, "38.242.199.182");
        // assert_eq!(node.num_onnections, 378);
        assert!((node.betweenness - 0.000244483600836513).abs() < f64::EPSILON);
        assert!((node.closeness - 2.0013493455674).abs() < f64::EPSILON);
    }
}
