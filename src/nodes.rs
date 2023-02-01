use crate::geoip_cache::GeoIPCache;
use ziggurat_core_geoip::geoip::GeoInfo;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use spectre::graph::{Graph, AGraph};

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

fn hash_geo_location(latitude: f64, longitude: f64) -> String {
    // make unique every 0.2, so multiply by 5, convert to integer
    let ilatitude = (latitude * 5.0).floor() as i32;
    let ilongitude = (longitude * 5.0).floor() as i32;
    format!("{ilatitude}:{ilongitude}")
}

// essentially, we sort the nodes into groups at (nearly) same geo-location
// we use 0.2 degrees for epsilon in both axes.
// hash gets created from a string created by two numbers
// increment each time the same location is found
pub fn set_column_positions(nodes: &mut Vec<Node>) -> HashMap<String, u32> {
    let mut column_stats = HashMap::new();
    for node in nodes {
        if let Some(geoinfo) = &node.geolocation {
            if let Some(latitude) = geoinfo.latitude {
                if let Some(longitude) = geoinfo.longitude {
                    let geostr = hash_geo_location(latitude, longitude);
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

// do the lookup again, and set node's corresponding final column size
// this field is the same for all nodes in a given column
pub fn set_column_sizes(nodes: &mut Vec<Node>, column_stats: &mut HashMap<String, u32>) {
    for node in nodes {
        if let Some(geoinfo) = &node.geolocation {
            if let Some(latitude) = geoinfo.latitude {
                if let Some(longitude) = geoinfo.longitude {
                    let geostr = hash_geo_location(latitude, longitude);
                    if let Some(count) = column_stats.get(&geostr) {
                        node.column_size = *count;
                    }
                }
            }
        }
    }
}

pub async fn create_nodes(
    agraph: &AGraph,
    node_ips: &[String],
    geo_cache: &GeoIPCache,
) -> Vec<Node> {
    let graph: Graph<usize> = Graph::new();
    let (betweenness, closeness) = graph.compute_betweenness_and_closeness(agraph);
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

    let mut column_stats = set_column_positions(&mut nodes);
    set_column_sizes(&mut nodes, &mut column_stats);
    nodes
}
