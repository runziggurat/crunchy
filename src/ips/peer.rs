use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::Node;

/// Peer list structure containing peer list for each node
#[derive(Clone, Serialize, Deserialize)]
pub struct Peer {
    /// IP address of the node
    pub ip: SocketAddr,
    /// List of peers for the node
    pub list: Vec<SocketAddr>,
}

impl Peer {
    /// Generate peerlist for given nodes based on their connections
    pub fn generate_all_peerlists(nodes: &[Node]) -> Vec<Peer> {
        let mut peer_list = Vec::with_capacity(nodes.len());

        for node in nodes {
            peer_list.push(Peer::generate_peerlist(node, nodes));
        }

        peer_list
    }

    /// Generate peerlist for given node based on its connections
    pub fn generate_peerlist(node: &Node, nodes: &[Node]) -> Peer {
        let mut peer_list_entry = Peer {
            ip: node.addr,
            list: Vec::with_capacity(node.connections.len()),
        };

        for peer in &node.connections {
            if *peer >= nodes.len() || nodes[*peer].addr == node.addr {
                continue;
            }

            peer_list_entry.list.push(nodes[*peer].addr);
        }

        peer_list_entry
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use super::*;

    #[test]
    fn generate_peerlist_for_node_test() {
        let nodes = vec![
            Node {
                addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 0, 0, 0)), 1234),
                connections: vec![1, 2],
                ..Default::default()
            },
            Node {
                addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(2, 0, 0, 0)), 1234),
                connections: vec![0, 2],
                ..Default::default()
            },
            Node {
                addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(3, 0, 0, 0)), 1234),
                connections: vec![0, 1],
                ..Default::default()
            },
        ];

        let peer = Peer::generate_peerlist(nodes.get(0).unwrap(), &nodes);
        assert_eq!(peer.list.len(), 2);
        assert!(peer.list.contains(&nodes.get(1).unwrap().addr));
        assert!(peer.list.contains(&nodes.get(2).unwrap().addr));
    }
}
