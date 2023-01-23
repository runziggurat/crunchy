use std::{
    collections::HashMap,
    env, fs,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use spectre::{graph::AGraph, graph::Graph};

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Node {
    ip: String,
    betweenness: f64,
    closeness: f64,
    num_connections: usize,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct CrunchyState {
    agraph_length: usize,
    elapsed: f64,
    nodes: Vec<Node>,
    min_betweenness: f64,
    max_betweenness: f64,
    min_closeness: f64,
    max_closeness: f64,
}

#[allow(dead_code)]
#[derive(Default, Deserialize)]
pub struct NetworkSummary {
    num_known_nodes: usize,
    num_good_nodes: usize,
    num_known_connections: usize,
    num_versions: usize,
    protocol_versions: HashMap<u32, usize>,
    user_agents: HashMap<String, usize>,
    crawler_runtime: Duration,
    node_ips: Vec<String>,
    agraph: AGraph,
}

#[allow(dead_code)]
#[derive(Default, Deserialize)]
pub struct JsonRpcResponse {
    jsonrpc: String,
    result: NetworkSummary,
    id: usize,
}

pub fn load_response(filepath: &str) -> JsonRpcResponse {
    let jstring = fs::read_to_string(filepath).expect("could not open response file");
    serde_json::from_str(&jstring).unwrap()
}

pub fn load_state(filepath: &str) -> CrunchyState {
    let jstring = fs::read_to_string(filepath).expect("could not open state file");
    serde_json::from_str(&jstring).unwrap()
}

fn write_state(infile: &str, outfile: &str) {
    let response = load_response(infile);
    let agraph = response.result.agraph;
    let graph: Graph<usize> = Graph::new();
    let start = Instant::now();
    let (betweenness, closeness) = graph.compute_betweenness_and_closeness(&agraph);
    let elapsed = start.elapsed();
    let mut min_betweenness: f64 = 10000.0;
    let mut max_betweenness: f64 = 0.0;
    let mut min_closeness: f64 = 10000.0;
    let mut max_closeness: f64 = 0.0;

    for between in &betweenness {
        if *between < min_betweenness {
            min_betweenness = *between;
        }
        if *between > max_betweenness {
            max_betweenness = *between;
        }
    }
    for close in &closeness {
        if *close < min_closeness {
            min_closeness = *close;
        }
        if *close > max_closeness {
            max_closeness = *close;
        }
    }
    let mut nodes = Vec::with_capacity(agraph.len());
    for i in 0..agraph.len() {
        let node: Node = Node {
            ip: response.result.node_ips[i].clone(),
            betweenness: betweenness[i],
            closeness: closeness[i],
            num_connections: agraph[i].len(),
        };
        nodes.push(node);
    }

    let state = CrunchyState {
        agraph_length: agraph.len(),
        elapsed: elapsed.as_secs_f64(),
        nodes,
        min_betweenness,
        max_betweenness,
        min_closeness,
        max_closeness,
    };
    let joutput = serde_json::to_string(&state).unwrap();
    fs::write(outfile, joutput).unwrap();
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        println!("\n\nUsage is: cargo run <in-sample.json> <out-state.json> ");
        println!("E.g.:     \x1b[93mcargo run --release testdata/sample.json testdata/state.json\x1b[0m\n");
        return;
    }
    if !fs::metadata(&args[1]).is_ok() {
        println!("{}: No such file or directory", &args[1]);
        return
    }
    write_state(&args[1], &args[2]);
}

#[test]
fn test_state_output() {
    let infile = "testdata/sample.json";
    let outfile = "testdata/state.json";
    let _ = fs::remove_file(outfile);
    write_state(infile, outfile);
    let state = load_state(outfile);
    let size: usize = 2531;
    assert_eq!(state.agraph_length, size);
    assert_eq!(state.nodes.len(), size);
    assert!((state.min_betweenness - 0.0).abs() < f64::EPSILON);
    assert!((state.max_betweenness - 0.0006471174062683313).abs() < f64::EPSILON);
    assert!((state.min_closeness - 1.9965036212494205).abs() < f64::EPSILON);
    assert!((state.max_closeness - 2.9965618988763065).abs() < f64::EPSILON);
    let node = state.nodes[5].clone();
    assert_eq!(node.ip, "38.242.199.182");
    assert_eq!(node.num_connections, 378);
    assert!((node.betweenness - 0.000244483600836513).abs() < f64::EPSILON);
    assert!((node.closeness - 2.0013493455674).abs() < f64::EPSILON);
}
