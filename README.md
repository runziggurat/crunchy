[![dependency status](https://deps.rs/repo/github/runziggurat/crunchy/status.svg)](https://deps.rs/repo/github/runziggurat/crunchy)

# crunchy
P2P network crawler data cruncher for graph metrics


# Running

`crunchy` gets two command line parameters: an input file, and an output file. Both are `JSON` files.

The input file is a sample generated by our zcash crawler. Its format (a JSON-RPC response) corresponds to this:


```
{
    jsonrpc: String,
    result: {
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
    id: usize,
}
```

The generated output contains processed data that our renderer can directly use. It looks like this:

```
{
    agraph_length: u32,
    elapsed: f64,
    nodes: [
        ip: String,
        betweenness: f64,
        closeness: f64,
        num_connections: usize
    ],
    betweenness: Vec<f64>,
    closeness: Vec<f64>,
    min_betweenness: f64,
    max_betweenness: f64,
    min_closeness: f64,
    max_closeness: f64
}
```

### Command Line

```
cargo run --release testdata/sample.json testdata/state.json
```

