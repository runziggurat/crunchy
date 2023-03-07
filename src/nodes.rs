use std::{collections::HashMap, net::SocketAddr};

use serde::{Deserialize, Serialize};
use spectre::{edge::Edge, graph::Graph};
use ziggurat_core_crawler::summary::NodesIndices;
use ziggurat_core_geoip::geoip::GeoInfo;

use crate::{geoip_cache::GeoIPCache, histogram::Histogram};

const HISTOGRAM_COUNTS: usize = 256;

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct HistogramSummary {
    /// Name of the histogram
    pub label: String,
    /// Counts for each slot
    pub counts: Vec<usize>,
    /// Maximum count for a single slot
    pub max_count: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Node {
    /// the ip address with port number
    pub addr: SocketAddr,
    /// the computed betweenness
    pub betweenness: f64,
    /// the computed closeness
    pub closeness: f64,
    /// this corresponds to the z-coordinate in the visualizer
    pub cell_position: u32,
    /// the height of the cell (think, a column)
    pub cell_height: u32,
    /// indices of all connected nodes
    pub connections: Vec<usize>,
    /// used for latitude, longitude, city, country
    pub geolocation: Option<GeoInfo>,
}

// Implemented it just to make it easier to create a default node for testing
impl Default for Node {
    fn default() -> Self {
        Self {
            addr: SocketAddr::new("0.0.0.0".parse().unwrap(), 0),
            betweenness: 0.0,
            closeness: 0.0,
            cell_position: 0,
            cell_height: 0,
            connections: Vec::new(),
            geolocation: None,
        }
    }
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
        if let Some(GeoInfo {
            coordinates: Some(coordinates),
            ..
        }) = &node.geolocation
        {
            let geostr = hash_geo_location(coordinates.latitude, coordinates.longitude);
            if let Some(count) = cell_stats.get(&geostr) {
                node.cell_height = *count;
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
    if let Some(GeoInfo {
        coordinates: Some(coordinates),
        ..
    }) = geolocation
    {
        let geostr = hash_geo_location(coordinates.latitude, coordinates.longitude);

        return *cell_stats
            .entry(geostr)
            .and_modify(|count| *count += 1)
            .or_insert(1);
    }
    0
}

pub async fn create_nodes(
    indices: &NodesIndices,
    node_addrs: &[SocketAddr],
    geo_cache: &GeoIPCache,
) -> Vec<Node> {
    let mut graph = Graph::new();
    for (n, node) in indices.iter().enumerate() {
        node.iter()
            .filter(|&connection| *connection > n)
            .for_each(|connection| {
                graph.insert(Edge::new(n, *connection));
            });
    }

    let betweenness = graph.betweenness_centrality();
    let closeness = graph.closeness_centrality();
    let mut nodes = Vec::with_capacity(indices.len());
    let mut cell_stats: HashMap<String, u32> = HashMap::new();

    for i in 0..indices.len() {
        let geolocation = geo_cache.lookup(node_addrs[i].ip()).await;
        let cell_position = get_cell_position(&mut cell_stats, &geolocation);

        let node: Node = Node {
            addr: node_addrs[i],
            betweenness: *betweenness
                .get(&i)
                .expect("could not find betweenness value for index}"),
            closeness: *closeness
                .get(&i)
                .expect("could not find closeness value for index"),
            cell_position,
            cell_height: 0,
            connections: indices[i].clone(),
            geolocation,
        };
        nodes.push(node);
    }

    // this second pass is necessary, but quite fast
    set_cell_heights(&mut nodes, &mut cell_stats);
    nodes
}

pub async fn create_histograms(nodes: &[Node]) -> Vec<HistogramSummary> {
    // Betweenness
    let mut histogram_b = Histogram {
        ..Histogram::default()
    };

    // Closeness
    let mut histogram_c = Histogram {
        ..Histogram::default()
    };

    // Degree
    let mut histogram_d = Histogram {
        ..Histogram::default()
    };

    for node in nodes.iter() {
        histogram_b.add(node.betweenness);
        histogram_c.add(node.closeness);
        histogram_d.add(node.connections.len() as f64);
    }

    let mut histograms = Vec::new();
    let (counts, max_count) = histogram_b.compute(HISTOGRAM_COUNTS);
    histograms.push(HistogramSummary {
        label: "betweenness".to_owned(),
        counts,
        max_count,
    });

    let (counts, max_count) = histogram_c.compute(HISTOGRAM_COUNTS);
    histograms.push(HistogramSummary {
        label: "closeness".to_owned(),
        counts,
        max_count,
    });

    let (counts, max_count) = histogram_d.compute(HISTOGRAM_COUNTS);
    histograms.push(HistogramSummary {
        label: "degree".to_owned(),
        counts,
        max_count,
    });

    histograms
}
