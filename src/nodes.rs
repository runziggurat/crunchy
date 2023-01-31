use crate::geoip_cache::GeoIPCache;
use ziggurat_core_geoip::geoip::GeoInfo;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use spectre::graph::Graph;

//TODO(asmie): there is some redundancy here as we have ip in the Node structure and one more time
// in the GeoIPInfo structure.
#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Node {
    pub ip: String,
    pub betweenness: f64,
    pub closeness: f64,
    pub column_position: u32,
    pub column_size: u32,
    pub connections: Vec<usize>,
    pub geolocation: Option<GeoInfo>,
}

pub fn compute_columns(nodes: &mut Vec<Node>) -> HashMap<String, u32> {
    let mut column_stats: HashMap<String, u32> = HashMap::new();
    for node in nodes {
        if let Some(geoinfo) = &node.geolocation {
            if let Some(latitude) = geoinfo.latitude {
                if let Some(longitude) = geoinfo.longitude {
                    let ilatitude: i32 = (latitude * 5.0).floor() as i32;
                    let ilongitude: i32 = (longitude * 5.0).floor() as i32;
                    let geostr = format!("{}:{}", ilatitude, ilongitude);
                    column_stats
                        .entry(geostr.clone())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                    node.column_position = column_stats[&geostr];
                }
            }
        }
    }
    column_stats
}

pub fn set_column_sizes(nodes: &mut Vec<Node>, column_stats: &mut HashMap<String, u32>) {
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

pub async fn create_nodes(
    agraph: &Vec<Vec<usize>>,
    node_ips: &Vec<String>,
    geo_cache: &GeoIPCache,
) -> Vec<Node> {
    let graph: Graph<usize> = Graph::new();
    let (betweenness, closeness) = graph.compute_betweenness_and_closeness(&agraph);
    let mut nodes = Vec::with_capacity(agraph.len());
    for i in 0..agraph.len() {
        let node: Node = Node {
            ip: node_ips[i].clone(),
            betweenness: betweenness[i],
            closeness: closeness[i],
            column_position: 0,
            column_size: 0,
            connections: agraph[i].clone(),
            geolocation: geo_cache
                .lookup(node_ips[i].parse().expect("malformed IP address"))
                .await,
        };
        nodes.push(node);
    }

    let mut column_stats = compute_columns(&mut nodes);
    set_column_sizes(&mut nodes, &mut column_stats);
    nodes
}
