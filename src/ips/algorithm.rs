// Intelligent Peer Sharing (IPS) module
// The main algorithm is divided into two main parts:
// Security part - it is responsible for the selection of the peer that when removed, can cause
// the biggest damage to the network. There is also bridge detection algorithm.
// Second part is the optimization one. It is responsible for the selection of the peers that
// can improve the overall network statistics and improve connectivity between the peers.
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
    fs::File,
    io,
    io::Write,
    net::SocketAddr,
};

use ziggurat_core_crawler::summary::NetworkType;

use crate::{
    config::GeoLocationMode,
    constants::NUM_THREADS,
    ips::{
        config::IPSConfiguration,
        graph_utils::{
            construct_graph, filter_network, find_bridges, find_lowest_betweenness, remove_node,
        },
        normalization::NormalizationFactors,
        peer::Peer,
        statistics::{
            degree_centrality_avg, generate_statistics, print_statistics, print_statistics_delta,
        },
    },
    CrunchyState, Node,
};

/// Intelligent Peer Sharing (IPS) module structure
#[derive(Default, Clone)]
pub struct Ips {
    config: IPSConfiguration,
}

/// State structure containing all the information about the graph and nodes at some point
#[derive(Default, Clone)]
pub struct IpsState {
    /// Nodes present in the network
    pub nodes: Vec<Node>,
    /// Peer list for each node in the network
    pub peer_list: Vec<Peer>,
    /// Degrees of each node in the network
    pub degrees: HashMap<SocketAddr, u32>,
    /// Betweenness of each node in the network
    pub eigenvalues: HashMap<SocketAddr, f64>,
    /// Degree factors used for normalization
    pub degree_factors: NormalizationFactors,
    /// Betweenness factors used for normalization
    pub betweenness_factors: NormalizationFactors,
    /// Closeness factors used for normalization
    pub closeness_factors: NormalizationFactors,
    /// Eigenvector factors used for normalization
    pub eigenvector_factors: NormalizationFactors,
}

/// Internal structure for storing peer information
#[derive(PartialEq, Copy, Clone)]
struct PeerEntry {
    /// IP address of the peer
    pub addr: SocketAddr,
    /// Index of the peer in the state.nodes
    pub index: usize,
    /// Rating of the peer
    pub rating: f64,
}

const NORMALIZE_TO_VALUE: f64 = 100.0;
const NORMALIZE_HALF: f64 = NORMALIZE_TO_VALUE / 2.0;
const NORMALIZE_2_3: f64 = NORMALIZE_TO_VALUE * 2.0 / 3.0;
const NORMALIZE_1_3: f64 = NORMALIZE_TO_VALUE * 1.0 / 3.0;

const ERR_GET_DEGREE: &str = "failed to get degree";
const ERR_GET_EIGENVECTOR: &str = "failed to get eigenvector";

const MASSIVE_ISLAND_PERCENTAGE: f64 = 0.1;
const NODES_TO_BE_REMOVED_PERCENTAGE: f64 = 0.1;

impl Ips {
    pub fn new(config: IPSConfiguration) -> Ips {
        Ips { config }
    }

    /// Generate peer list - main function with The Algorithm
    pub async fn generate(&mut self, state: &CrunchyState, network: NetworkType) -> Vec<Peer> {
        // Set up logging
        let output = match self.config.log_path {
            Some(ref path) => File::create(path).map(|f| Box::new(f) as Box<dyn Write>),
            None => Ok(Box::new(io::stdout()) as Box<dyn Write>),
        };

        let mut o = output.unwrap_or_else(|e| {
            println!("Failed to open the log file: {e}");
            Box::new(io::stdout()) as Box<dyn Write>
        });

        // Sanity check that each node is really connected to its peers and the peers also
        // have the node in their connections.
        writeln!(o, "IPS algorithm started...").unwrap();
        let start_time = std::time::Instant::now();

        writeln!(o, "Checking for nodes connected to themselves...").unwrap();
        for (idx, node) in state.nodes.iter().enumerate() {
            if node.connections.contains(&idx) {
                writeln!(o, "{} is connected to itself.", node.addr).unwrap();
            }

            for peer in &node.connections {
                if !state.nodes[*peer].connections.contains(&idx) {
                    writeln!(
                        o,
                        "{} is not connected to {} but {} have a connection to it",
                        node.addr, state.nodes[*peer].addr, node.addr
                    )
                    .unwrap();
                }
            }
        }

        let network_nodes = filter_network(&state.nodes, network);

        writeln!(
            o,
            "Network contains {} nodes and {} connections",
            network_nodes.len(),
            network_nodes
                .iter()
                .fold(0, |acc, n| acc + n.connections.len())
        )
        .unwrap();

        writeln!(o, "Generating initial network state and its statistics... ").unwrap();

        // This is the working set of factors.
        let mut working_state = self.generate_state(&network_nodes, true);
        let mut final_state = working_state.clone();

        let initial_statistics = generate_statistics(&working_state);

        writeln!(o, "Statistics for the initial network:").unwrap();
        print_statistics(&mut o, &initial_statistics);

        writeln!(
            o,
            "Generated initial state and statistics in {} s",
            start_time.elapsed().as_secs()
        )
        .unwrap();

        // Phase 1: Security checks

        // Detect islands
        let islands = self.detect_islands(&working_state.nodes);
        if islands.len() > 1 {
            // Check if we're talking about massive islands or just a few nodes
            let mut massive_islands_count = 0;
            for island in &islands {
                // Check if any island is more than some % of the network
                if island.len()
                    > (working_state.nodes.len() as f64 * MASSIVE_ISLAND_PERCENTAGE).round()
                        as usize
                {
                    massive_islands_count += 1;
                }
            }

            if massive_islands_count > 1 {
                // We need to break here. Merging big islands can be a very complex task especially
                // when they started to live their lives and created their own blockchain history
                // after separation.
                panic!("There are more than one massive island in the network. It is not possible to merge them automatically.");
            }

            writeln!(
                o,
                "IPS detected no massive islands. However, there are some disconnected nodes."
            )
            .unwrap();
        } else {
            // There are no islands
            writeln!(o, "IPS detected no islands").unwrap();
        }

        if !self.check_and_fix_integrity_upon_removal(&mut working_state) {
            writeln!(o, "There were hot nodes that can be dangerous for the network! Recalculating graph...").unwrap();
            working_state = self.generate_state(&working_state.nodes, true);
        } else {
            // There are no hot nodes
            writeln!(o, "IPS detected no fragmentation possibility even when top nodes would be disconnected").unwrap();
        }

        // Now take the current params
        let degree_avg = degree_centrality_avg(&working_state.degrees);

        // Detect possible bridges
        let bridges = find_bridges(
            &working_state.nodes,
            self.config.bridge_threshold_adjustment,
        );

        // Phase 2: Generate peer list using MCDA optimization.

        writeln!(o, "The MCDA procedure is starting...").unwrap();

        // Node rating can be split into two parts: constant and variable depending on the node's
        // location. Now we can compute each node's constant rating based on some graph params.
        let const_factors = self.calculate_const_factors(&working_state);

        // Iterate over nodes to generate peerlist entry for each node
        for (node_idx, node) in working_state.nodes.iter().enumerate() {
            let node_addr = node.addr;

            // Clone const factors for each node to be able to modify them
            let mut peer_ratings = const_factors.clone();

            let mut curr_peer_ratings: Vec<PeerEntry> = Vec::new();

            // 1 - update ranks by location for specified node
            // This need to be done every time as location ranking will change for differently
            // located nodes.
            if self.config.geolocation != GeoLocationMode::Off {
                self.update_rating_by_location(node, &working_state.nodes, &mut peer_ratings);
            }

            // Load peerlist with current connections (we don't want to change everything)
            for peer in &final_state.nodes[node_idx].connections {
                // Remember current peer ratings
                curr_peer_ratings.push(peer_ratings[*peer]);
            }

            // Get current node's degree for further computations
            let degree = *working_state.degrees.get(&node_addr).expect(ERR_GET_DEGREE);

            // 2 - Calculate desired vertex degree
            // In the first iteration we will use degree average so all nodes should pursue to
            // that level. That could be bad if graph's vertexes have very high (or low) degrees
            // and therefore, delta is very high (or low) too. But until we have some better idea
            // this one is the best we can do to keep up with the graph.
            let desired_degree = degree_avg.round() as u32;

            // 3 - Calculate how many peers to add or delete from peerlist
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

            // Remove potential peers identified to have too high degree and have already
            // been processed by the algorithm
            peer_ratings.retain(|x| {
                final_state.nodes[x.index].connections.len()
                    < working_state.nodes[x.index].connections.len()
            });

            // Remove nodes that reached max conn limit
            peer_ratings.retain(|x| {
                final_state.nodes[x.index]
                    .connections
                    .len()
                    .abs_diff(working_state.nodes[x.index].connections.len())
                    <= self.config.change_no_more as usize
            });

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
                    curr_peer_ratings.retain(|x| x != &peer);
                }
                peers_to_delete_count -= 1;
            }

            // 5 - Find peers to add from selected peers (based on rating)
            if peers_to_add_count > 0 {
                // Sort peers by rating
                peer_ratings.sort_by(|a, b| b.rating.partial_cmp(&a.rating).unwrap());

                let mut candidates = peer_ratings
                    .iter()
                    .filter(|x| {
                        // Check if we're not adding a node that is already connected to us
                        if final_state.nodes[x.index].connections.contains(&node_idx) {
                            return false;
                        }

                        // Check if we're not adding a node that is already connected to us
                        if final_state.nodes[node_idx].connections.contains(&x.index) {
                            return false;
                        }

                        true
                    })
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
                    curr_peer_ratings.push(*peer);
                    final_state.nodes[peer.index].connections.push(node_idx);
                }

                // Write new node set
                final_state.nodes[node_idx].connections = curr_peer_ratings
                    .iter()
                    .map(|x| x.index)
                    .collect::<Vec<usize>>()
                    .to_vec();

                // Eliminate duplicates, the node itself and shrink vector
                final_state.nodes[node_idx].connections.sort();
                final_state.nodes[node_idx].connections.dedup();
                final_state.nodes[node_idx]
                    .connections
                    .retain(|x| *x != node_idx);
                final_state.nodes[node_idx].connections.shrink_to_fit();
            }
        }

        writeln!(
            o,
            "All IPS computations done in {} s from IPS start",
            start_time.elapsed().as_secs()
        )
        .unwrap();

        final_state = self.generate_state(&final_state.nodes, true);

        let final_statistics = generate_statistics(&final_state);
        writeln!(o, "Statistics for the final network:").unwrap();
        print_statistics(&mut o, &final_statistics);

        writeln!(
            o,
            "Comparing if network parameters got changed on plus or minus:"
        )
        .unwrap();
        print_statistics_delta(&mut o, &final_statistics, &initial_statistics);

        writeln!(
            o,
            "IPS has been working for {} seconds",
            start_time.elapsed().as_secs()
        )
        .unwrap();

        final_state.peer_list
    }

    // Helper functions

    /// Check integrity of the network after removing some percent of the nodes with highest
    /// betweenness factor.
    /// Return true if integrity is preserved, false otherwise. If false is returned the caller
    /// should try to regenerate the network.
    fn check_and_fix_integrity_upon_removal(&self, state: &mut IpsState) -> bool {
        let mut high_betweenness = state
            .nodes
            .iter()
            .map(|x| x.betweenness)
            .collect::<Vec<f64>>();

        high_betweenness.sort_by(|a, b| b.partial_cmp(a).unwrap());

        let mut test_state = state.clone();
        let mut removed_idx = Vec::new();

        // Take some % of nodes with highest betweenness
        let nodes_to_remove =
            (high_betweenness.len() as f64 * NODES_TO_BE_REMOVED_PERCENTAGE).round() as usize;
        for b in high_betweenness.iter().take(nodes_to_remove) {
            let idx = test_state
                .nodes
                .iter()
                .position(|x| x.betweenness == *b)
                .unwrap();
            remove_node(&mut test_state.nodes, idx);
            removed_idx.push(idx);
        }

        let islands = self.detect_islands(&test_state.nodes);
        let mut massive_island = 0;
        if islands.len() > 1 {
            // Consider network as not integral if there are more than 1 islands with at least
            // some % of nodes. Don't consider islands with less than some % of nodes as they would
            // probably have no meaning for the network itself.
            for island in islands.iter() {
                if island.len()
                    > (test_state.nodes.len() as f64 * MASSIVE_ISLAND_PERCENTAGE).round() as usize
                {
                    massive_island += 1;
                }
            }
        }

        if massive_island > 1 {
            // If we're able to fragment the network into more than 1 massive islands then try to fix it
            // by adding new connections between highest betweenness node's neighbors.
            for node_idx in removed_idx {
                let mut conns = state.nodes[node_idx].connections.clone();
                let node_a_idx = find_lowest_betweenness(&conns, state);
                // Remove node_a_idx from conns
                conns.retain(|x| *x != node_a_idx);
                let node_b_idx = find_lowest_betweenness(&conns, state);

                state.nodes[node_a_idx].connections.push(node_b_idx);
                state.nodes[node_b_idx].connections.push(node_a_idx);
            }
            return false;
        }

        true
    }

    /// Generate state for IPS
    /// If generate_full is true, then it will generate full state for IPS. If false then
    /// it will not re-run betweenness and closeness centrality calculations.
    fn generate_state(&self, nodes: &[Node], generate_full: bool) -> IpsState {
        let mut ips_state = IpsState {
            nodes: nodes.to_vec(),
            ..Default::default()
        };

        let mut graph = construct_graph(nodes);

        if generate_full {
            let betweenness = graph.betweenness_centrality(NUM_THREADS, false);
            let closeness = graph.closeness_centrality(NUM_THREADS);

            // Recalculate factors with new graph
            for node in ips_state.nodes.iter_mut() {
                let addr = node.addr;
                node.betweenness = *betweenness.get(&addr).expect("can't fetch betweenness");
                node.closeness = *closeness.get(&addr).expect("can't fetch closeness");
            }
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

        for (index, node) in state.nodes.iter().enumerate() {
            let addr = node.addr;
            const_factors.push(PeerEntry {
                addr,
                index,
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

    fn rate_node(&self, node: &Node, state: &IpsState) -> f64 {
        // Calculate rating for node (if min == max for normalization factors then rating is
        // not increased for that factor as lerp() returns 0.0).
        // Rating is a combination of the following factors:
        let mut rating = 0.0;

        let addr = node.addr;
        let degree = *state.degrees.get(&addr).expect(ERR_GET_DEGREE);
        let eigenvalue = *state.eigenvalues.get(&addr).expect(ERR_GET_EIGENVECTOR);

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
    use std::{
        net::{IpAddr, Ipv4Addr, SocketAddr},
        str::FromStr,
    };

    use spectre::{edge::Edge, graph::Graph};

    use super::*;

    pub const ERR_PARSE_IP: &str = "failed to parse IP address";

    #[test]
    fn rate_node_test() {
        let ips_config = IPSConfiguration::default();
        let ips = Ips::new(ips_config);

        let nodes = vec![
            Node {
                addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 1234),
                connections: vec![1, 2],
                ..Default::default()
            },
            Node {
                addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 0, 0, 0)), 1234),
                connections: vec![0, 2],
                ..Default::default()
            },
            Node {
                addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(2, 0, 0, 0)), 1234),
                connections: vec![0, 1],
                ..Default::default()
            },
        ];

        let state = ips.generate_state(&nodes, true);

        assert_eq!(ips.rate_node(nodes.get(0).unwrap(), &state), 10.0);
    }

    #[tokio::test]
    async fn detect_islands_test_no_islands() {
        let mut graph = Graph::new();
        let mut nodes = Vec::new();
        let mut addrs = Vec::new();
        let ips_config = IPSConfiguration::default();
        let ips = Ips::new(ips_config);

        for i in 0..10 {
            let addr = SocketAddr::new(
                IpAddr::from_str(format!("192.169.0.{i}").as_str()).expect(ERR_PARSE_IP),
                1234,
            );

            addrs.push(addr);

            let node = Node {
                addr,
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
                graph.insert(Edge::new(nodes[i].addr, nodes[j].addr));
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
        let mut addrs = Vec::new();
        let ips_config = IPSConfiguration::default();
        let ips = Ips::new(ips_config);

        for i in 0..10 {
            let addr = SocketAddr::new(
                IpAddr::from_str(format!("192.169.0.{i}").as_str()).expect(ERR_PARSE_IP),
                1234,
            );

            addrs.push(addr);

            let node = Node {
                addr,
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
                graph.insert(Edge::new(nodes[i].addr, nodes[j].addr));

                nodes[i].connections.push(j);
                nodes[j].connections.push(i);
            }
        }

        let islands = ips.detect_islands(&nodes);

        assert_eq!(islands.len(), nodes.len());
    }
}
