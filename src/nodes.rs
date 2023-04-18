use std::net::SocketAddr;

use serde::{Deserialize, Serialize};
use spectre::{edge::Edge, graph::Graph};
use ziggurat_core_crawler::summary::{NetworkType, NodesIndices};
use ziggurat_core_geoip::geoip::GeoInfo;

use crate::{constants::NUM_THREADS, geoip_cache::GeoIPCache, histogram::Histogram};

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
    /// the node network type
    pub network_type: NetworkType,
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
            network_type: NetworkType::Unknown,
            betweenness: 0.0,
            closeness: 0.0,
            connections: Vec::new(),
            geolocation: None,
        }
    }
}

pub async fn create_nodes_unfiltered(
    indices: &NodesIndices,
    node_addrs: &[SocketAddr],
    node_network_types: &[NetworkType],
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

    let betweenness = graph.betweenness_centrality(NUM_THREADS, false);
    let closeness = graph.closeness_centrality(NUM_THREADS);
    let mut nodes = Vec::with_capacity(indices.len());

    for i in 0..indices.len() {
        let node: Node = Node {
            addr: node_addrs[i],
            network_type: node_network_types[i],
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

pub async fn create_nodes_filtered(
    network_type_filter: NetworkType,
    indices: &NodesIndices,
    node_addrs: &[SocketAddr],
    node_network_types: &[NetworkType],
    geo_cache: &GeoIPCache,
) -> Vec<Node> {
    let num_nodes = indices.len();

    // Create reindexing map using filter value
    //    a) the nodes we keep get new indexing, 0..N
    //    b) the nodes we don't want keep initial value of -1
    let mut index: i32 = 0;
    let mut index_map: Vec<i32> = vec![-1; num_nodes];
    for (n, network_type) in node_network_types.iter().enumerate() {
        if network_type_filter == *network_type {
            index_map[n] = index;
            index += 1;
        }
    }

    // index is the size of our new node indices object,
    // i.e., the new number of nodes.  Initialize it.
    let mut new_indices: NodesIndices = vec![Vec::<usize>::new(); index as usize];

    // Create new NodesIndices object using
    //   a) original indices
    //   b) the index map
    // We only keep connections where both nodes are in the index map
    let mut graph = Graph::new();
    for (n, node) in indices.iter().enumerate() {
        let n_index: i32 = index_map[n];
        if n_index != -1 {
            node.iter()
                .filter(|&connection| {
                    // For each connection, we only add it once, so we use the connection
                    // where source index is less than target
                    index_map[*connection] != -1 && index_map[*connection] > n_index
                })
                .for_each(|connection| {
                    graph.insert(Edge::new(n_index as usize, index_map[*connection] as usize));
                    new_indices[n_index as usize].push(index_map[*connection] as usize);
                    new_indices[index_map[*connection] as usize].push(n_index as usize);
                });
        }
    }

    // Our newly create node indices struct might have nodes with zero connections
    // To those nodes: we add a connection to self.
    for (n, node) in new_indices.iter().enumerate() {
        if node.is_empty() {
            graph.insert(Edge::new(n, n));
        }
    }

    let betweenness = graph.betweenness_centrality(NUM_THREADS, false);
    let closeness = graph.closeness_centrality(NUM_THREADS);
    let mut nodes = Vec::with_capacity(indices.len());

    // here we use the original indexing, because of the node addrs array
    for i in 0..indices.len() {
        let index = index_map[i];
        if index != -1 {
            let node: Node = Node {
                addr: node_addrs[i],
                network_type: node_network_types[i],
                betweenness: *betweenness
                    .get(&(index as usize))
                    .expect("could not find betweenness value for index}"),
                closeness: *closeness
                    .get(&(index as usize))
                    .expect("could not find closeness value for index"),
                connections: new_indices[index as usize].clone(),
                geolocation: geo_cache.lookup(node_addrs[i].ip()).await,
            };
            nodes.push(node);
        }
    }
    nodes
}

pub async fn create_nodes(
    filter_type: Option<NetworkType>,
    indices: &NodesIndices,
    node_addrs: &[SocketAddr],
    node_network_types: &[NetworkType],
    geo_cache: &GeoIPCache,
) -> Vec<Node> {
    if let Some(filter_type) = filter_type {
        create_nodes_filtered(
            filter_type,
            indices,
            node_addrs,
            node_network_types,
            geo_cache,
        )
        .await
    } else {
        create_nodes_unfiltered(indices, node_addrs, node_network_types, geo_cache).await
    }
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
