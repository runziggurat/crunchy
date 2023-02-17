// Intelligent Peer Sharing (IPS) module
// Selection is based on the "beauty" contest of the nodes - each node is evaluated based on its
// degree, betweenness, closeness and eigenvector centrality. Then, if requested ranking is
// updated with location factor. Each factor has its own weight that is used to determine
// factor's importance to the calculation of the final ranking. That gives possibility
// to test different approaches to the selection of the peers, without re-compiling the code.
// Weights are defined in the configuration file.
// When the ranking is calculated, the peers are selected based on the ranking. The number of
// peers can be changed and it is defined in the configuration file. Algorithm is constructed
// as step by step process, so it is easy to add new steps or change the order of the steps.
// Especially, there could be a need to add some modifiers to the ranking.

use crate::config::IPSConfiguration;
use crate::{CrunchyState, Node};
use geoutils::Location;
use serde::{Deserialize, Serialize};
use spectre::edge::Edge;
use spectre::graph::{AGraph, Graph};
use std::collections::HashMap;
use std::collections::{HashSet, VecDeque};
use std::net::IpAddr;
use std::str::FromStr;

/// Intelligent Peer Sharing (IPS) module structure
#[derive(Default, Clone)]
pub struct Ips {
    config: IPSConfiguration,
    degree_factors: NormalizationFactors,
    betweenness_factors: NormalizationFactors,
    closeness_factors: NormalizationFactors,
    eigenvector_factors: NormalizationFactors,
}

/// Peer list structure containing peer list for each node
#[derive(Clone, Serialize, Deserialize)]
pub struct Peer {
    /// IP address of the node
    pub ip: IpAddr,
    /// List of peers for the node
    pub list: Vec<IpAddr>,
}

const NORMALIZE_TO_VALUE: f64 = 100.0;
#[derive(Default, Clone)]
struct NormalizationFactors {
    min: f64,
    max: f64,
}

impl Ips {
    pub fn new(config: IPSConfiguration) -> Ips {
        Ips {
            config,
            ..Default::default()
        }
    }

    /// Generate peer list - main function with The Algorithm
    /// It needs state and agraph to be passed as parameters which need to be correlated with
    /// the crawler's state and agraph (and with each other), so the indexes saved in the
    /// agraph are the same as the positions of the nodes in the state.nodes.
    pub async fn generate(&mut self, state: &CrunchyState, agraph: &AGraph) -> Vec<Peer> {
        let mut peer_list = Vec::new();

        // Reconstruct graph from the agraph - we need to do this because we need all the
        // measures provided by spectre's graph implementation.
        // Using agraph gives us certainity that we are using the same graph as the crawler and
        // there are only good nodes there (this is critical assumption!). Second assumption is
        // that agraph node indexes are the same as in the state.nodes vector.
        let mut graph = self.construct_graph(&state.nodes, agraph);

        // 0 - Detect islands
        // To reconsider if islands should be merged prior to any other computations or not.
        // IMHO, if there are islands they can influence on the results of the computations.
        // TODO(asmie): Merging islands is not implemented yet.
        let _islands = self.detect_islands(agraph);

        // Now take the current params
        let degrees = graph.degree_centrality();
        let degree_avg = self.degree_centrality_avg(&degrees);
        let eigenvalues = graph.eigenvalue_centrality();

        // Determine factors used for normalization.
        // Normalization step is needed to make sure that all the factors are in the same range and
        // weights can be applied to them.
        self.degree_factors =
            NormalizationFactors::determine(&degrees.values().cloned().collect::<Vec<u32>>());

        self.eigenvector_factors =
            NormalizationFactors::determine(&eigenvalues.values().cloned().collect::<Vec<f64>>());

        let betweenness = &state
            .nodes
            .iter()
            .map(|n| n.betweenness)
            .collect::<Vec<f64>>();
        self.betweenness_factors = NormalizationFactors::determine(betweenness);

        let closeness = &state
            .nodes
            .iter()
            .map(|n| n.closeness)
            .collect::<Vec<f64>>();
        self.closeness_factors = NormalizationFactors::determine(closeness);

        // Node rating can be split into two parts: constant and variable depending on the node's
        // location. Now we can compute each node's constant rating based on some graph params.
        // Vector contains IpAddr, node index (from the state.nodes) and rating. We need index just
        // to be able to easily get the node from nodes vector after sorting.
        let mut const_factors = Vec::with_capacity(state.nodes.len());
        for node_idx in 0..state.nodes.len() {
            let ip = IpAddr::from_str(state.nodes[node_idx].ip.as_str()).unwrap();
            const_factors.push((
                ip,
                node_idx,
                self.rate_node(
                    &state.nodes[node_idx],
                    *degrees.get(&ip).unwrap(), // should be safe to unwrap here
                    *eigenvalues.get(&ip).unwrap(),
                ),
            ));
        }

        // Iterate over nodes to generate peerlist entry for each node
        for (node_idx, node) in state.nodes.iter().enumerate() {
            let node_ip = IpAddr::from_str(node.ip.as_str()).unwrap();

            // Clone const factors for each node to be able to modify them
            let mut peer_ratings = const_factors.clone();
            let mut curr_peer_ratings: Vec<(IpAddr, usize, f64)> = Vec::new();

            let mut peer_list_entry = Peer {
                ip: node_ip,
                list: Vec::new(),
            };

            // 1 - update ranks by location for specified node
            // This need to be done every time as location ranking will change for differently
            // located nodes.
            if self.config.use_geolocation {
                self.update_rating_by_location(node, &state.nodes, &mut peer_ratings);
            }

            // Load peerlist with current connections (we don't want to change everything)
            for (peer_idx, rating) in peer_ratings.iter().enumerate().take(agraph[node_idx].len()) {
                let peer = agraph[node_idx][peer_idx];
                peer_list_entry
                    .list
                    .push(IpAddr::from_str(state.nodes[peer].ip.as_str()).unwrap());

                // Remeber current peer ratings
                curr_peer_ratings.push(*rating);
            }

            // 2 - Calculate desired vertex degree
            // In the first iteration we will use avg of degree and degree_delta so all
            // nodes should pursue to degree_delta level. That could be bad if graph's vertexes
            // have very high (or low) degrees and therefore, delta is very high (or low) too. But until
            // we have some better idea this one is the best we can do to keep up with the graph.
            let desired_degree = (degree_avg as u32 + degrees.get(&node_ip).unwrap()) / 2;

            // 3 - Calculate how many peers to add or delete from peerlist
            let peers_to_delete_count = if desired_degree < *degrees.get(&node_ip).unwrap() {
                degrees.get(&node_ip).unwrap() - desired_degree
            } else {
                // Check if config forces to change peerlist even if we have good degree.
                // This should be always set to at least one to allow for some changes in graph -
                // searching for better potential peers.
                self.config.change_at_least
            };

            // Calculating how many peers should be added. If we have more peers than desired degree
            // we will add at least config.change_at_least peers.
            let mut peers_to_add_count = if desired_degree > *degrees.get(&node_ip).unwrap() {
                desired_degree - degrees.get(&node_ip).unwrap() + peers_to_delete_count
            } else {
                self.config.change_at_least
            };

            // Limit number of changes to config value
            if peers_to_add_count > self.config.change_no_more {
                peers_to_add_count = self.config.change_no_more;
            }

            // 4 - Choose peers to delete from peerlist (based on ranking)
            if peers_to_delete_count > 0 {
                // Sort peers by rating (highest first)
                curr_peer_ratings.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

                // Remove peers with lowest rating
                for _ in 0..peers_to_delete_count {
                    let peer_to_delete = curr_peer_ratings.pop();
                    if let Some(peer_to_delete) = peer_to_delete {
                        peer_list_entry.list.retain(|x| x != &peer_to_delete.0);
                    }
                }
            }

            // 5 - Find peers to add from selected peers (based on rating)
            if peers_to_add_count > 0 {
                // Take twice as many candidates as we need to add to peerlist to be able to
                // choose best ones from them.
                let mut candidates_to_search = peers_to_add_count * 2;

                let mut candidates: Vec<(IpAddr, usize, f64)> =
                    Vec::with_capacity(candidates_to_search as usize);

                // Sort peers by rating
                peer_ratings.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

                // Add peers with highest rating as candidates
                for peer in peer_ratings.iter() {
                    // Check if peer is already in peerlist - if so go to next one
                    if peer_list_entry.list.contains(&peer.0) {
                        continue;
                    }

                    candidates.push(*peer);
                    candidates_to_search -= 1;
                    if candidates_to_search == 0 {
                        break;
                    }
                }

                // Here we have 2*peers_to_add_count candidates to add sorted by ranking.
                // We need to choose best ones from them - let's choose those with lowest
                // betweenness factor - just to avoid creating "hot" nodes that have very high
                // importance to the network which can be risky if such node goes down.
                candidates.sort_by(|a, b| {
                    state.nodes[a.1]
                        .betweenness
                        .partial_cmp(&state.nodes[b.1].betweenness)
                        .unwrap()
                });

                for peer in candidates.iter().take(peers_to_add_count as usize) {
                    peer_list_entry.list.push(peer.0);
                }
            }

            // Do not compute factors one more time after every single peerlist addition. At least
            // for now, when computing factors is very expensive (especially betweenness and closeness).
            // Re-calculating it after each node for whole graph would take too long.

            peer_list.push(peer_list_entry);
        }
        peer_list
    }

    // Helper functions

    /// Update nodes rating based on location
    fn update_rating_by_location(
        &self,
        selected_node: &Node,
        nodes: &[Node],
        ratings: &mut [(IpAddr, usize, f64)],
    ) {
        let selected_location = &selected_node
            .geolocation
            .as_ref()
            .map(|geo_info| {
                Location::new(
                    geo_info.latitude.unwrap_or_default(), // TODO(asmie): refactor when zg-core will be updated with Location
                    geo_info.longitude.unwrap_or_default(),
                )
            })
            .unwrap_or(Location::new(0.0, 0.0));

        for (node_idx, node) in nodes.iter().enumerate() {
            let mut rating = 0.0;
            if let Some(geo_info) = &node.geolocation {
                let location = Location::new(
                    geo_info.latitude.unwrap_or_default(),
                    geo_info.longitude.unwrap_or_default(),
                );
                let distance = selected_location.distance_to(&location).unwrap().meters();
                let pref_distance = self.config.geolocation_minmax_distance_km as f64 * 1000.0;

                // Map distance to some levels of rating - now they are taken arbitrarily but
                // they should be somehow related to the distance.
                if self.config.use_closer_geolocation {
                    match distance {
                        _ if distance < pref_distance => rating = NORMALIZE_TO_VALUE,
                        _ if distance < 2.0 * pref_distance => {
                            rating = NORMALIZE_TO_VALUE * 2.0 / 3.0
                        }
                        _ if distance < 3.0 * pref_distance => {
                            rating = NORMALIZE_TO_VALUE * 1.0 / 3.0
                        }
                        _ => rating = 0.0,
                    }
                } else {
                    match distance {
                        _ if distance < 0.5 * pref_distance => rating = 0.0,
                        _ if distance < pref_distance => rating = NORMALIZE_TO_VALUE / 2.0,
                        _ => rating = NORMALIZE_TO_VALUE,
                    }
                }
            }
            ratings[node_idx].2 += rating * self.config.mcda_weights.location;
        }
    }

    fn construct_graph(&self, nodes: &[Node], agraph: &AGraph) -> Graph<IpAddr> {
        let mut graph = Graph::new();

        for i in 0..agraph.len() {
            for j in 0..agraph[i].len() {
                let edge = Edge::new(
                    IpAddr::from_str(nodes[i].ip.as_str()).unwrap(),
                    IpAddr::from_str(nodes[j].ip.as_str()).unwrap(),
                );
                graph.insert(edge);
            }
        }
        graph
    }

    fn degree_centrality_avg(&self, degrees: &HashMap<IpAddr, u32>) -> f64 {
        (degrees.iter().fold(0, |acc, (_, &degree)| acc + degree) as f64) / degrees.len() as f64
    }

    fn rate_node(&self, node: &Node, degree: u32, eigenvalue: f64) -> f64 {
        // Calculate rating for node (if min == max for normalization factors then rating is
        // not increased for that factor as lerp() returns 0.0).
        // Rating is a combination of the following factors:
        let mut rating = 0.0;

        // 1. Degree
        rating += self.degree_factors.lerp(degree as f64)
            * NORMALIZE_TO_VALUE
            * self.config.mcda_weights.degree;

        // 2. Betweenness
        rating += self.betweenness_factors.lerp(node.betweenness)
            * NORMALIZE_TO_VALUE
            * self.config.mcda_weights.betweenness;

        // 3. Closeness
        rating += self.closeness_factors.lerp(node.closeness)
            * NORMALIZE_TO_VALUE
            * self.config.mcda_weights.closeness;

        // 4. Eigenvector
        rating += self.eigenvector_factors.lerp(eigenvalue)
            * NORMALIZE_TO_VALUE
            * self.config.mcda_weights.eigenvector;

        rating
    }

    // Very simple algorithm to detect islands.
    // Take first vertex and do BFS to find all connected vertices. If there are any unvisited vertices
    // create new island and do BFS one more time. Repeat until all vertices are visited.
    fn detect_islands(&self, agraph: &AGraph) -> Vec<HashSet<usize>> {
        let mut islands = Vec::new();
        let mut visited = vec![false; agraph.len()];

        for i in 0..agraph.len() {
            if visited[i] {
                continue;
            }

            let mut island = HashSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(i);

            while let Some(node_idx) = queue.pop_front() {
                if visited[node_idx] {
                    continue;
                }

                island.insert(node_idx);

                visited[node_idx] = true;

                for j in 0..agraph[node_idx].len() {
                    if !visited[agraph[node_idx][j]] {
                        queue.push_back(agraph[node_idx][j]);
                    }
                }
            }
            islands.push(island);
        }
        islands
    }
}

impl NormalizationFactors {
    fn determine<T>(list: &[T]) -> NormalizationFactors
    where
        T: PartialOrd + Into<f64> + Copy,
    {
        let min = list
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let max = list
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        NormalizationFactors {
            min: (*min).into(),
            max: (*max).into(),
        }
    }

    fn lerp(&self, value: f64) -> f64 {
        if self.min == self.max {
            return 0.0;
        }

        (value - self.min) / (self.max - self.min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spectre::edge::Edge;

    #[test]
    fn normalization_factors_determine_test() {
        let list = vec![1, 2, 3, 4, 5];
        let factors = NormalizationFactors::determine(&list);

        assert_eq!(factors.min, 1.0);
        assert_eq!(factors.max, 5.0);
    }

    #[test]
    fn normalization_factors_lerp_test() {
        let factors = NormalizationFactors { min: 1.0, max: 5.0 };
        let value = 3.0;

        assert_eq!(factors.lerp(value), 0.5);
    }

    #[test]
    fn normalization_factors_lerp_divide_zero_test() {
        let factors = NormalizationFactors { min: 2.0, max: 2.0 };
        let value = 3.0;

        assert_eq!(factors.lerp(value), 0.0);
    }

    #[tokio::test]
    async fn detect_islands_test_no_islands() {
        let mut graph = Graph::new();
        let mut nodes = Vec::new();
        let mut ipaddrs = Vec::new();
        let ips_config = IPSConfiguration::default();
        let ips = Ips::new(ips_config);

        for i in 0..10 {
            let ip = format!("192.168.0.{i}");

            ipaddrs.push(IpAddr::from_str(ip.as_str()).unwrap());

            let node = Node {
                ip: ip.clone(),
                ..Default::default()
            };
            nodes.push(node);
        }

        // Case where each node is connected to all other nodes
        for i in 0..10 {
            for j in 0..10 {
                if i == j {
                    continue;
                }
                graph.insert(Edge::new(
                    IpAddr::from_str(nodes[i].ip.as_str()).unwrap(),
                    IpAddr::from_str(nodes[j].ip.as_str()).unwrap(),
                ));
            }
        }

        let agraph = graph.create_agraph(&ipaddrs);
        let islands = ips.detect_islands(&agraph);

        assert_eq!(islands.len(), 1);
    }

    #[tokio::test]
    async fn detect_islands_test() {
        let mut graph = Graph::new();
        let mut nodes = Vec::new();
        let mut ipaddrs = Vec::new();
        let ips_config = IPSConfiguration::default();
        let ips = Ips::new(ips_config);

        for i in 0..10 {
            let ip = format!("192.169.0.{i}");

            ipaddrs.push(IpAddr::from_str(ip.as_str()).unwrap());

            let node = Node {
                ip: ip.clone(),
                ..Default::default()
            };
            nodes.push(node);
        }

        // Each node is connected only to itself - each node is an island
        for i in 0..10 {
            for j in 0..10 {
                if i != j {
                    continue;
                }
                graph.insert(Edge::new(
                    IpAddr::from_str(nodes[i].ip.as_str()).unwrap(),
                    IpAddr::from_str(nodes[j].ip.as_str()).unwrap(),
                ));
            }
        }

        let agraph = graph.create_agraph(&ipaddrs);
        let islands = ips.detect_islands(&agraph);

        assert_eq!(islands.len(), nodes.len());
    }
}
