use crate::geoip_cache::GeoIPCache;
use ziggurat_core_geoip::geoip::GeoInfo;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use spectre::graph::{AGraph, Graph};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Node {
    // the ip address, dotted quad, without port number
    pub ip: String,
    // the computed betweenness
    pub betweenness: f64,
    // the computed closeness
    pub closeness: f64,
    // this corresponds to the z-coordinate in the visualizer
    pub cell_position: u32,
    // the height of the cell (think, a column)
    pub cell_height: u32,
    // indices of all connected nodes
    pub connections: Vec<usize>,
    // used for latitude, longitude, city, country
    pub geolocation: Option<GeoInfo>,
}

fn hash_geo_location(latitude: f64, longitude: f64) -> String {
    // make unique every 0.2 degrees in both axes, so multiply by 5, convert to integer
    const DEGREE_RESOLUTION: f64 = 1.0 / 0.2;
    let latitude = (latitude * DEGREE_RESOLUTION).floor() as i32;
    let longitude = (longitude * DEGREE_RESOLUTION).floor() as i32;
    format!("{latitude}:{longitude}")
}

// second pass: do the lookup again, and set node's corresponding final cell height
// this field is the same for all nodes in a given cell
pub fn set_cell_heights(nodes: &mut Vec<Node>, cell_stats: &mut HashMap<String, u32>) {
    for node in nodes {
        if let Some(geoinfo) = &node.geolocation {
            if let Some(latitude) = geoinfo.latitude {
                let longitude = geoinfo
                    .longitude
                    .expect("a longitude must be set if a latitude is set");
                let geostr = hash_geo_location(latitude, longitude);
                if let Some(count) = cell_stats.get(&geostr) {
                    node.cell_height = *count;
                }
            }
        }
    }
}

// essentially, we sort the nodes into groups at (nearly) same geo-location
// we use 0.2 degrees for epsilon in both axes.
// hash gets created from a string created by two numbers
// increment each time the same location is found
pub fn get_cell_position(
    cell_stats: &mut HashMap<String, u32>,
    geolocation: &Option<GeoInfo>,
) -> u32 {
    if let Some(geoinfo) = geolocation {
        if let Some(latitude) = geoinfo.latitude {
            let longitude = geoinfo
                .longitude
                .expect("a longitude must be set if a latitude is set");
            let geostr = hash_geo_location(latitude, longitude);
            return *cell_stats
                .entry(geostr)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }
    }
    0
}

pub async fn create_nodes(
    agraph: &AGraph,
    node_ips: &[String],
    geo_cache: &GeoIPCache,
) -> Vec<Node> {
    let graph: Graph<usize> = Graph::new();
    // TODO(asmie/kylegranger): make this an associated function for Graph.
    // it does not use the Graph per se
    let (betweenness, closeness) = graph.compute_betweenness_and_closeness(agraph);
    let mut nodes = Vec::with_capacity(agraph.len());
    let mut cell_stats: HashMap<String, u32> = HashMap::new();
    for i in 0..agraph.len() {
        let geolocation = geo_cache
            .lookup(node_ips[i].parse().expect("malformed IP address"))
            .await;
        let cell_position = get_cell_position(&mut cell_stats, &geolocation);
        let node: Node = Node {
            ip: node_ips[i].clone(),
            betweenness: betweenness[i],
            closeness: closeness[i],
            cell_position,
            cell_height: 0,
            connections: agraph[i].clone(),
            geolocation,
        };
        nodes.push(node);
    }
    // this second pass is necessary, but quite fast
    set_cell_heights(&mut nodes, &mut cell_stats);
    nodes
}
