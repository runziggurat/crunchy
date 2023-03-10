use std::net::SocketAddr;

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
            connections: Vec::new(),
            geolocation: None,
        }
    }
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

    for i in 0..indices.len() {
        let node: Node = Node {
            addr: node_addrs[i],
            betweenness: *betweenness
                .get(&i)
                .expect("could not find betweenness value for index}"),
            closeness: *closeness
                .get(&i)
                .expect("could not find closeness value for index"),
            connections: indices[i].clone(),
            geolocation: geo_cache.lookup(node_addrs[i].ip()).await,
        };
        nodes.push(node);
    }
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
