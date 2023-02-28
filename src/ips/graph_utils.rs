use std::{
    collections::{HashMap, HashSet},
    net::IpAddr,
    str::FromStr,
};

use spectre::{edge::Edge, graph::Graph};

use crate::{
    ips::{algorithm::ERR_PARSE_IP, utils::median},
    Node,
};

/// Find bridges in graph.
/// Bridges are edges that if removed disconnects the graph but here we try to find something
/// similar to bridges - connections that acts like bridges between two inter-connected islands
/// but cutting them do not disconnect the graph (as there can be couple of bridges for each
/// interconnected island). That is why we use betweenness centrality to find such connections
/// instead of some popular bridge finding algorithms (like Tarjan's algorithm or chain
/// decomposition).
///
/// The idea is to find connections that have high betweenness centrality on both ends. The main
/// problem is meaning of high betweenness centrality. This approach uses median of betweenness
/// centrality of all nodes as a base point for threshold. Then, to eliminate some corner cases
/// (eg. when there are only few nodes with high betweenness centrality and most of the nodes have
/// low factor value what could result in finding too many bridges) we adjust the threshold by
/// const factor read from configuration. There could be different approaches like not using
/// the median but taking value from some percentile (eg. 90th percentile) but this could lead to
/// set threshold to find too many bridges in case of eg. balanced graph (if there are many nodes
/// with similar betweenness centrality taking top 20% would result in finding fake bridges).
pub fn find_bridges(nodes: &[Node], threshold_adjustment: f64) -> HashMap<usize, HashSet<usize>> {
    let mut bridges = HashMap::new();

    // If there are less than 2 nodes there is no point in finding bridges.
    if nodes.len() < 2 {
        return bridges;
    }

    let mut betweenness_list = nodes.iter().map(|n| n.betweenness).collect::<Vec<f64>>();

    betweenness_list.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let betweenness_median = median(&betweenness_list).unwrap(); // Safe to uwrap as we checked if there are at least 2 nodes.
    let betweenness_threshold = betweenness_median * threshold_adjustment;

    for (node_idx, node) in nodes.iter().enumerate() {
        if node.betweenness < betweenness_threshold {
            continue;
        }

        for peer_idx in &node.connections {
            if nodes[*peer_idx].betweenness <= betweenness_threshold {
                continue;
            }

            bridges
                .entry(node_idx)
                .and_modify(|peers: &mut HashSet<usize>| {
                    peers.insert(*peer_idx);
                })
                .or_insert(HashSet::new())
                .insert(*peer_idx);

            bridges
                .entry(*peer_idx)
                .and_modify(|peers: &mut HashSet<usize>| {
                    peers.insert(node_idx);
                })
                .or_insert(HashSet::new())
                .insert(node_idx);
        }
    }
    bridges
}

/// Reconstruct graph from nodes and their connection subfield. This step is used to run
/// some graph algorithms on the graph (like betweenness centrality).
pub fn construct_graph(nodes: &[Node]) -> Graph<IpAddr> {
    let mut graph = Graph::new();

    for node in nodes {
        let node_ip = IpAddr::from_str(node.ip.as_str()).expect(ERR_PARSE_IP);
        for i in &node.connections {
            let edge = Edge::new(
                node_ip,
                IpAddr::from_str(nodes[*i].ip.as_str()).expect(ERR_PARSE_IP),
            );
            graph.insert(edge);
        }
    }
    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construct_graph_test() {
        let nodes = vec![
            Node {
                ip: "0.0.0.0".to_string(),
                connections: vec![1, 2],
                ..Default::default()
            },
            Node {
                ip: "1.0.0.0".to_string(),
                connections: vec![0, 2],
                ..Default::default()
            },
            Node {
                ip: "2.0.0.0".to_string(),
                connections: vec![0, 1],
                ..Default::default()
            },
        ];

        let mut graph = construct_graph(&nodes);
        let degrees = graph.degree_centrality();
        assert_eq!(
            degrees.get(&IpAddr::from_str("0.0.0.0").unwrap()).unwrap(),
            &2
        );
        assert_eq!(
            degrees.get(&IpAddr::from_str("1.0.0.0").unwrap()).unwrap(),
            &2
        );
        assert_eq!(
            degrees.get(&IpAddr::from_str("2.0.0.0").unwrap()).unwrap(),
            &2
        );
    }

    #[test]
    fn find_bridges_test() {
        let nodes = vec![
            Node {
                ip: "0.0.0.0".to_string(),
                betweenness: 1.0,
                connections: vec![1, 2],
                ..Default::default()
            },
            Node {
                ip: "0.0.0.0".to_string(),
                betweenness: 1.5,
                connections: vec![0, 2, 3],
                ..Default::default()
            },
            Node {
                ip: "0.0.0.0".to_string(),
                betweenness: 1.3,
                connections: vec![1, 3],
                ..Default::default()
            },
            Node {
                ip: "0.0.0.0".to_string(),
                betweenness: 3.1,
                connections: vec![1, 2, 4],
                ..Default::default()
            },
            Node {
                ip: "0.0.0.0".to_string(),
                betweenness: 3.2,
                connections: vec![3, 5, 7],
                ..Default::default()
            },
            Node {
                ip: "0.0.0.0".to_string(),
                betweenness: 1.0,
                connections: vec![4, 6],
                ..Default::default()
            },
            Node {
                ip: "0.0.0.0".to_string(),
                betweenness: 1.2,
                connections: vec![5, 7],
                ..Default::default()
            },
            Node {
                ip: "0.0.0.0".to_string(),
                betweenness: 1.4,
                connections: vec![4, 6],
                ..Default::default()
            },
        ];

        let bridges = find_bridges(&nodes, 1.25);
        assert!(bridges.contains_key(&3));
        let peers = bridges.get(&3).unwrap();
        assert_eq!(peers.len(), 1);
        assert!(peers.contains(&4));
    }
}
