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
pub struct PeerList {
    /// IP address of the node
    pub peer: IpAddr,
    /// List of peers for the node
    pub peer_list: Vec<IpAddr>,
}

const NORMALIZE_TO_VALUE: f64 = 100.0;
#[derive(Default, Clone)]
struct NormalizationFactors {
    min: f64,
    max: f64,
    value: f64,
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
    pub async fn generate(&mut self, state: &CrunchyState, agraph: &AGraph) -> Vec<PeerList> {
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
        let _islands = self.detect_islands(&state.nodes, agraph);

        // Now take the current params
        let degrees = graph.degree_centrality();
        let degree_delta = graph.degree_centrality_delta();
        let eigenvalues = graph.eigenvalue_centrality();

        // Determine factors used for normalization.
        // Normalization step is needed to make sure that all the factors are in the same range and
        // weights can be applied to them.
        self.determine_degrees_factors(&degrees);
        self.determine_eigenvalue_factors(&eigenvalues);
        self.determine_betweenness_factors(&state.nodes);
        self.determine_closeness_factors(&state.nodes);

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

            let mut peer_list_entry = PeerList {
                peer: node_ip,
                peer_list: Vec::new(),
            };

            // 1 - update ranks by location for specified node
            // This need to be done every time as location ranking will change for differently
            // located nodes.
            if self.config.pref_location {
                self.update_rating_by_location(node, &state.nodes, &mut peer_ratings);
            }

            // Load peerlist with current connections (we don't want to change everything)
            for (peer_idx, rating) in peer_ratings.iter().enumerate().take(agraph[node_idx].len()) {
                let peer = agraph[node_idx][peer_idx];
                peer_list_entry
                    .peer_list
                    .push(IpAddr::from_str(state.nodes[peer].ip.as_str()).unwrap());

                // Remeber current peer ratings
                curr_peer_ratings.push(*rating);
            }

            // 2 - Calculate desired vertex degree
            // In the first iteration we will use avg of degree and degree_delta so all
            // nodes should pursue to degree_delta level. That could be bad if graph's vertexes
            // have very high (or low) degrees and therefore, delta is very high (or low) too. But until
            // we have some better idea this one is the best we can do to keep up with the graph.
            let desired_degree = (degree_delta as u32 + degrees.get(&node_ip).unwrap()) / 2;

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
                        peer_list_entry.peer_list.retain(|x| x != &peer_to_delete.0);
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
                    if peer_list_entry.peer_list.contains(&peer.0) {
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
                    peer_list_entry.peer_list.push(peer.0);
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
                let pref_distance = self.config.pref_location_distance as f64 * 1000.0;

                // Map distance to some levels of rating - now they are taken arbitrarily but
                // they should be somehow related to the distance.
                if self.config.pref_location_closer {
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
            ratings[node_idx].2 += rating * self.config.location_weight;
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

    fn determine_degrees_factors(&mut self, degrees: &HashMap<IpAddr, u32>) {
        let min = *degrees
            .iter()
            .min_by(|a, b| a.1.cmp(b.1))
            .map(|m| m.1)
            .unwrap_or(&0);
        let max = *degrees
            .iter()
            .max_by(|a, b| a.1.cmp(b.1))
            .map(|m| m.1)
            .unwrap_or(&(NORMALIZE_TO_VALUE as u32));

        self.degree_factors = NormalizationFactors {
            min: min as f64,
            max: max as f64,
            value: NORMALIZE_TO_VALUE,
        };
    }

    fn determine_eigenvalue_factors(&mut self, eigenvalues: &HashMap<IpAddr, f64>) {
        let min = *eigenvalues
            .iter()
            .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|m| m.1)
            .unwrap_or(&0.0);
        let max = *eigenvalues
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|m| m.1)
            .unwrap_or(&NORMALIZE_TO_VALUE);

        self.eigenvector_factors = NormalizationFactors {
            min,
            max,
            value: NORMALIZE_TO_VALUE,
        };
    }

    fn determine_betweenness_factors(&mut self, nodes: &[Node]) {
        let min = nodes
            .iter()
            .min_by(|a, b| a.betweenness.partial_cmp(&b.betweenness).unwrap())
            .map(|m| m.betweenness)
            .unwrap_or(0.0);
        let max = nodes
            .iter()
            .max_by(|a, b| a.betweenness.partial_cmp(&b.betweenness).unwrap())
            .map(|m| m.betweenness)
            .unwrap_or(NORMALIZE_TO_VALUE);

        self.betweenness_factors = NormalizationFactors {
            min,
            max,
            value: NORMALIZE_TO_VALUE,
        };
    }

    fn determine_closeness_factors(&mut self, nodes: &[Node]) {
        let min = nodes
            .iter()
            .min_by(|a, b| a.closeness.partial_cmp(&b.closeness).unwrap())
            .map(|m| m.closeness)
            .unwrap_or(0.0);
        let max = nodes
            .iter()
            .max_by(|a, b| a.closeness.partial_cmp(&b.closeness).unwrap())
            .map(|m| m.closeness)
            .unwrap_or(NORMALIZE_TO_VALUE);

        self.closeness_factors = NormalizationFactors {
            min,
            max,
            value: NORMALIZE_TO_VALUE,
        };
    }

    fn rate_node(&self, node: &Node, degree: u32, eigenvalue: f64) -> f64 {
        // Calculate rating for node

        // Rating is a combination of the following factors:
        // 1. Degree
        let mut rating = ((degree as f64 - self.degree_factors.min)
            / (self.degree_factors.max - self.degree_factors.min)
            * self.degree_factors.value)
            * self.config.degree_weight;
        // 2. Betweenness
        rating += ((node.betweenness - self.betweenness_factors.min)
            / (self.betweenness_factors.max - self.betweenness_factors.min)
            * self.betweenness_factors.value)
            * self.config.betweenness_weight;
        // 3. Closeness
        rating += ((node.closeness - self.closeness_factors.min)
            / (self.closeness_factors.max - self.closeness_factors.min)
            * self.closeness_factors.value)
            * self.config.closeness_weight;
        // 4. Eigenvector
        rating += ((eigenvalue - self.eigenvector_factors.min)
            / (self.eigenvector_factors.max - self.eigenvector_factors.min)
            * self.eigenvector_factors.value)
            * self.config.eigenvector_weight;

        rating
    }

    // Very simple algorithm to detect islands.
    // Take first vertex and do BFS to find all connected vertices. If there are any unvisited vertices
    // create new island and do BFS one more time. Repeat until all vertices are visited.
    fn detect_islands(&self, nodes: &[Node], agraph: &AGraph) -> Vec<HashSet<usize>> {
        let mut islands = Vec::new();
        let mut visited = vec![false; nodes.len()];

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
