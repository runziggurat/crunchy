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

use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::IpAddr,
    str::FromStr,
};

use crate::{
    config::{GeoLocationMode, IPSConfiguration},
    graph_utils::{construct_graph, find_bridges},
    normalization::NormalizationFactors,
    peer::Peer,
    CrunchyState, Node,
};

/// Intelligent Peer Sharing (IPS) module structure
#[derive(Default, Clone)]
pub struct Ips {
    config: IPSConfiguration,
}

/// State structure containing all the information about the graph and nodes at some point
#[derive(Default, Clone)]
struct IpsState {
    pub nodes: Vec<Node>,
    pub peer_list: Vec<Peer>,
    pub degrees: HashMap<IpAddr, u32>,
    pub eigenvalues: HashMap<IpAddr, f64>,
    pub degree_factors: NormalizationFactors,
    pub betweenness_factors: NormalizationFactors,
    pub closeness_factors: NormalizationFactors,
    pub eigenvector_factors: NormalizationFactors,
}

/// Internal structure for storing peer information
#[derive(Copy, Clone)]
struct PeerEntry {
    /// IP address of the peer
    pub ip: IpAddr,
    /// Index of the peer in the state.nodes
    pub index: usize,
    /// Ranking of the peer
    pub rating: f64,
}

const NORMALIZE_TO_VALUE: f64 = 100.0;
const NORMALIZE_HALF: f64 = NORMALIZE_TO_VALUE / 2.0;
const NORMALIZE_2_3: f64 = NORMALIZE_TO_VALUE * 2.0 / 3.0;
const NORMALIZE_1_3: f64 = NORMALIZE_TO_VALUE * 1.0 / 3.0;

pub const ERR_PARSE_IP: &str = "failed to parse IP address";
const ERR_GET_DEGREE: &str = "failed to get degree";
const ERR_GET_EIGENVECTOR: &str = "failed to get eigenvector";

impl Ips {
    pub fn new(config: IPSConfiguration) -> Ips {
        Ips { config }
    }

    /// Generate peer list - main function with The Algorithm
    pub async fn generate(&mut self, state: &CrunchyState) -> Vec<Peer> {
        // Initial state will be used to compare the results of the computations
        let initial_state = self.generate_state(&state.nodes);

        // This is the working set of factors.
        //TODO(asmie): add .clone() to the initial_state when it will be used and remove creating new vector
        // Now we're creating a new vector because MCDA code operates not on state but on the peerlist
        // and if we left here peerlist from the state, it would be doubled by MCDA.
        let mut working_state = initial_state;
        working_state.peer_list = Vec::new();

        // Phase 1: Security checks
        //TODO(asmie): Detecting islands, bridges and hot nodes. Checking if there are any nodes that upon removal
        // would cause the graph to be disconnected. If there are any, there is a need to create
        // new connections between the possible islands.

        // Detect islands
        // To reconsider if islands should be merged prior to any other computations or not.
        // IMHO, if there are islands they can influence on the results of the computations.
        // TODO(asmie): Merging islands is not implemented yet.
        let _islands = self.detect_islands(&working_state.nodes);

        // Now take the current params
        let degree_avg = self.degree_centrality_avg(&working_state.degrees);

        // Detect possible bridges
        let bridges = find_bridges(
            &working_state.nodes,
            self.config.bridge_threshold_adjustment,
        );

        // Phase 2: Generate peer list using MCDA optimization.

        // Node rating can be split into two parts: constant and variable depending on the node's
        // location. Now we can compute each node's constant rating based on some graph params.
        let const_factors = self.calculate_const_factors(&working_state);

        // Iterate over nodes to generate peerlist entry for each node
        for (node_idx, node) in working_state.nodes.iter().enumerate() {
            let node_ip = IpAddr::from_str(node.ip.as_str()).expect(ERR_PARSE_IP);

            // Clone const factors for each node to be able to modify them
            let mut peer_ratings = const_factors.clone();

            let mut curr_peer_ratings: Vec<PeerEntry> = Vec::new();

            let mut peer_list_entry = Peer {
                ip: node_ip,
                list: Vec::new(),
            };

            // 1 - update ranks by location for specified node
            // This need to be done every time as location ranking will change for differently
            // located nodes.
            if self.config.geolocation != GeoLocationMode::Off {
                self.update_rating_by_location(node, &working_state.nodes, &mut peer_ratings);
            }

            // Load peerlist with current connections (we don't want to change everything)
            for peer in &working_state.nodes[node_idx].connections {
                peer_list_entry.list.push(
                    IpAddr::from_str(working_state.nodes[*peer].ip.as_str()).expect(ERR_PARSE_IP),
                );

                // Remember current peer ratings
                curr_peer_ratings.push(peer_ratings[*peer]);
            }

            // Get current node's degree for further computations
            let degree = *working_state.degrees.get(&node_ip).expect(ERR_GET_DEGREE);

            // 2 - Calculate desired vertex degree
            // In the first iteration we will use degree average so all nodes should pursue to
            // that level. That could be bad if graph's vertexes have very high (or low) degrees
            // and therefore, delta is very high (or low) too. But until we have some better idea
            // this one is the best we can do to keep up with the graph.
            let desired_degree = ((degree_avg + degree as f64) / 2.0).round() as u32;

            // 3 - Calculate how many peers to add or delete from peerlist
            //TODO(asmie): when graph has been visualized it occured that it has many nodes with
            // self connections. This is not good as it takes place in peerlist and gives no
            // benefit.
            let mut peers_to_delete_count = if desired_degree < degree {
                degree.saturating_sub(desired_degree)
            } else {
                // Check if config forces to change peerlist even if we have good degree.
                // This should be always set to at least one to allow for some changes in graph -
                // searching for better potential peers.
                self.config.change_at_least
            };

            // Limit number of changes to config value
            if peers_to_delete_count > self.config.change_no_more {
                peers_to_delete_count = self.config.change_no_more;
            }

            // Calculating how many peers should be added. If we have more peers than desired degree
            // we will add at least config.change_at_least peers.
            let mut peers_to_add_count = if desired_degree > degree {
                desired_degree
                    .saturating_sub(degree)
                    .saturating_add(peers_to_delete_count)
            } else {
                self.config.change_at_least
            };

            // Limit number of changes to config value
            if peers_to_add_count > self.config.change_no_more {
                peers_to_add_count = self.config.change_no_more;
            }

            // Remove node itself to ensure we don't add it to peerlist
            peer_ratings.retain(|x| x.index != node_idx);

            // Sort peers by rating (highest first)
            curr_peer_ratings.sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap());

            // 4 - Choose peers to delete from peerlist (based on ranking)
            while peers_to_delete_count > 0 {
                if let Some(peer) = curr_peer_ratings.pop() {
                    // Check if we're not deleting a bridge
                    if bridges.contains_key(&peer.index) && bridges[&peer.index].contains(&node_idx)
                    {
                        continue;
                    }
                    peer_list_entry.list.retain(|x| x != &peer.ip);
                }
                peers_to_delete_count -= 1;
            }

            // 5 - Find peers to add from selected peers (based on rating)
            if peers_to_add_count > 0 {
                // Sort peers by rating
                peer_ratings.sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap());

                // Remove peers that are already in peerlist
                peer_ratings.retain(|x| !peer_list_entry.list.contains(&x.ip));

                let mut candidates = peer_ratings
                    .iter()
                    .take((peers_to_add_count * 2) as usize) // Take twice as many candidates
                    .copied()
                    .collect::<Vec<_>>();

                // Here we have 2*peers_to_add_count candidates to add sorted by ranking.
                // We need to choose best ones from them - let's choose those with lowest
                // betweenness factor - just to avoid creating "hot" nodes that have very high
                // importance to the network which can be risky if such node goes down.
                candidates.sort_by(|a, b| {
                    working_state.nodes[a.index]
                        .betweenness
                        .partial_cmp(&working_state.nodes[b.index].betweenness)
                        .unwrap()
                });

                for peer in candidates.iter().take(peers_to_add_count as usize) {
                    peer_list_entry.list.push(peer.ip);
                }
            }
            working_state.peer_list.push(peer_list_entry);
        }

        // TODO(asmie): recalculate and compare factors to check if network is going better

        working_state.peer_list
    }

    // Helper functions

    /// Generate state for IPS
    fn generate_state(&self, nodes: &[Node]) -> IpsState {
        let mut ips_state = IpsState {
            nodes: nodes.to_vec(),
            ..Default::default()
        };

        let mut graph = construct_graph(nodes);
        let betweenness = graph.betweenness_centrality();
        let closeness = graph.closeness_centrality();

        // Recalculate factors with new graph
        for node in ips_state.nodes.iter_mut() {
            let ip = IpAddr::from_str(&node.ip).unwrap();
            node.betweenness = *betweenness.get(&ip).expect("can't fetch betweenness");
            node.closeness = *closeness.get(&ip).expect("can't fetch closeness");
        }

        ips_state.degrees = graph.degree_centrality();
        ips_state.eigenvalues = graph.eigenvalue_centrality();

        ips_state.degree_factors = NormalizationFactors::determine(
            &ips_state.degrees.values().cloned().collect::<Vec<u32>>(),
        )
        .expect("can't calculate degree factors");

        ips_state.eigenvector_factors = NormalizationFactors::determine(
            &ips_state
                .eigenvalues
                .values()
                .cloned()
                .collect::<Vec<f64>>(),
        )
        .expect("can't calculate eigenvector factors");

        let betweenness = &nodes.iter().map(|n| n.betweenness).collect::<Vec<f64>>();
        ips_state.betweenness_factors = NormalizationFactors::determine(betweenness)
            .expect("can't calculate betweenness factors");

        let closeness = &nodes.iter().map(|n| n.closeness).collect::<Vec<f64>>();
        ips_state.closeness_factors =
            NormalizationFactors::determine(closeness).expect("can't calculate closeness factors");

        ips_state.peer_list = Peer::generate_all_peerlists(nodes);

        ips_state
    }

    /// Calculates const factors for each node.
    fn calculate_const_factors(&self, state: &IpsState) -> Vec<PeerEntry> {
        let mut const_factors = Vec::with_capacity(state.nodes.len());

        for (idx, node) in state.nodes.iter().enumerate() {
            let ip = IpAddr::from_str(node.ip.as_str()).expect(ERR_PARSE_IP);
            const_factors.push(PeerEntry {
                ip,
                index: idx,
                rating: self.rate_node(node, state),
            });
        }
        const_factors
    }

    /// Update nodes rating based on location
    fn update_rating_by_location(
        &self,
        selected_node: &Node,
        nodes: &[Node],
        ratings: &mut [PeerEntry],
    ) {
        if selected_node.geolocation.is_none() {
            return;
        }

        let selected_location =
            if let Some(coordinates) = selected_node.geolocation.as_ref().unwrap().coordinates {
                coordinates
            } else {
                return;
            };

        for (node_idx, node) in nodes.iter().enumerate() {
            if node.geolocation.is_none() {
                continue;
            }

            let geo_info = node.geolocation.as_ref().unwrap();
            if geo_info.coordinates.is_none() {
                continue;
            }

            let distance = selected_location.distance_to(geo_info.coordinates.unwrap());
            let minmax_distance_m = self.config.geolocation_minmax_distance_km as f64 * 1000.0;

            // Map distance to some levels of rating - now they are taken arbitrarily but
            // they should be somehow related to the distance.
            let rating = if self.config.geolocation == GeoLocationMode::PreferCloser {
                match distance {
                    _ if distance < minmax_distance_m => NORMALIZE_TO_VALUE,
                    _ if distance < 2.0 * minmax_distance_m => NORMALIZE_2_3,
                    _ if distance < 3.0 * minmax_distance_m => NORMALIZE_1_3,
                    _ => 0.0,
                }
            } else {
                match distance {
                    _ if distance < 0.5 * minmax_distance_m => 0.0,
                    _ if distance < minmax_distance_m => NORMALIZE_HALF,
                    _ => NORMALIZE_TO_VALUE,
                }
            };
            ratings[node_idx].rating += rating * self.config.mcda_weights.location;
        }
    }

    fn degree_centrality_avg(&self, degrees: &HashMap<IpAddr, u32>) -> f64 {
        if degrees.is_empty() {
            return 0.0;
        }

        (degrees.iter().fold(0, |acc, (_, &degree)| acc + degree) as f64) / degrees.len() as f64
    }

    fn rate_node(&self, node: &Node, state: &IpsState) -> f64 {
        // Calculate rating for node (if min == max for normalization factors then rating is
        // not increased for that factor as lerp() returns 0.0).
        // Rating is a combination of the following factors:
        let mut rating = 0.0;

        let ip = IpAddr::from_str(node.ip.as_str()).expect(ERR_PARSE_IP);
        let degree = *state.degrees.get(&ip).expect(ERR_GET_DEGREE);
        let eigenvalue = *state.eigenvalues.get(&ip).expect(ERR_GET_EIGENVECTOR);

        // 1. Degree
        rating += state.degree_factors.scale(degree as f64)
            * NORMALIZE_TO_VALUE
            * self.config.mcda_weights.degree;

        // 2. Betweenness
        rating += state.betweenness_factors.scale(node.betweenness)
            * NORMALIZE_TO_VALUE
            * self.config.mcda_weights.betweenness;

        // 3. Closeness
        rating += state.closeness_factors.scale(node.closeness)
            * NORMALIZE_TO_VALUE
            * self.config.mcda_weights.closeness;

        // 4. Eigenvector
        rating += state.eigenvector_factors.scale(eigenvalue)
            * NORMALIZE_TO_VALUE
            * self.config.mcda_weights.eigenvector;

        rating
    }

    // Very simple algorithm to detect islands.
    // Take first vertex and do BFS to find all connected vertices. If there are any unvisited vertices
    // create new island and do BFS one more time. Repeat until all vertices are visited.
    fn detect_islands(&self, nodes: &[Node]) -> Vec<HashSet<usize>> {
        let mut islands = Vec::new();
        let mut visited = vec![false; nodes.len()];

        for i in 0..nodes.len() {
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

                for j in 0..nodes[node_idx].connections.len() {
                    if !visited[nodes[node_idx].connections[j]] {
                        queue.push_back(nodes[node_idx].connections[j]);
                    }
                }
            }
            islands.push(island);
        }
        islands
    }
}

#[cfg(test)]
mod tests {
    use spectre::{edge::Edge, graph::Graph};

    use super::*;

    #[test]
    fn rate_node_test() {
        let ips_config = IPSConfiguration::default();
        let ips = Ips::new(ips_config);

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

        let state = ips.generate_state(&nodes);

        assert_eq!(ips.rate_node(nodes.get(0).unwrap(), &state), 10.0);
    }

    #[test]
    fn degree_centrality_avg_test() {
        let ips_config = IPSConfiguration::default();
        let ips = Ips::new(ips_config);
        let mut degrees = HashMap::new();
        degrees.insert(IpAddr::from_str("0.0.0.0").unwrap(), 1);
        degrees.insert(IpAddr::from_str("1.0.0.0").unwrap(), 2);
        degrees.insert(IpAddr::from_str("2.1.2.1").unwrap(), 3);
        degrees.insert(IpAddr::from_str("1.0.1.0").unwrap(), 4);

        assert!(ips.degree_centrality_avg(&degrees) - 2.5 < 0.0001);
    }

    #[test]
    fn degree_centrality_avg_empty_test() {
        let ips_config = IPSConfiguration::default();
        let ips = Ips::new(ips_config);
        let degrees = HashMap::new();

        assert_eq!(ips.degree_centrality_avg(&degrees), 0.0);
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

            ipaddrs.push(IpAddr::from_str(ip.as_str()).expect(ERR_PARSE_IP));

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
                    IpAddr::from_str(nodes[i].ip.as_str()).expect(ERR_PARSE_IP),
                    IpAddr::from_str(nodes[j].ip.as_str()).expect(ERR_PARSE_IP),
                ));
                nodes[i].connections.push(j);
                nodes[j].connections.push(i);
            }
        }

        let islands = ips.detect_islands(&nodes);

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

            ipaddrs.push(IpAddr::from_str(ip.as_str()).expect(ERR_PARSE_IP));

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
                    IpAddr::from_str(nodes[i].ip.as_str()).expect(ERR_PARSE_IP),
                    IpAddr::from_str(nodes[j].ip.as_str()).expect(ERR_PARSE_IP),
                ));

                nodes[i].connections.push(j);
                nodes[j].connections.push(i);
            }
        }

        let islands = ips.detect_islands(&nodes);

        assert_eq!(islands.len(), nodes.len());
    }
}
