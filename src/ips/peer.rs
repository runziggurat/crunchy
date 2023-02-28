use std::{net::IpAddr, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{ips::ips_algorithm::ERR_PARSE_IP, Node};

/// Peer list structure containing peer list for each node
#[derive(Clone, Serialize, Deserialize)]
pub struct Peer {
    /// IP address of the node
    pub ip: IpAddr,
    /// List of peers for the node
    pub list: Vec<IpAddr>,
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
            ip: IpAddr::from_str(node.ip.as_str()).expect(ERR_PARSE_IP),
            list: Vec::with_capacity(node.connections.len()),
        };

        for peer in &node.connections {
            if *peer >= nodes.len() {
                continue;
            }

            peer_list_entry
                .list
                .push(IpAddr::from_str(nodes[*peer].ip.as_str()).expect(ERR_PARSE_IP));
        }

        peer_list_entry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_peerlist_for_node_test() {
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

        let peer = Peer::generate_peerlist(nodes.get(0).unwrap(), &nodes);
        assert_eq!(peer.list.len(), 2);
        assert!(peer
            .list
            .contains(&IpAddr::from_str(&nodes.get(1).unwrap().ip).unwrap()));
        assert!(peer
            .list
            .contains(&IpAddr::from_str(&nodes.get(2).unwrap().ip).unwrap()));
    }
}
