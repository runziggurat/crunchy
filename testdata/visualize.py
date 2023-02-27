# Simple peers graph visualization using rustworkx and matplotlib (default rendering backend)
# Preqrequisites:
# pip install rustworkx
# pip install matplotlib

import json
import matplotlib.pyplot as plt
import matplotlib as mpl
import rustworkx as rx
from rustworkx.visualization import mpl_draw
import sys

conn_limit = 20

if len(sys.argv) < 2:
    print("Usage: python main.py peers.json")
    exit(1)

f = open(sys.argv[1])
data = json.load(f)

graph = rx.PyGraph()

ipdict = {}

for node in data:
    idx = graph.add_node(node['ip'])
    ipdict[node['ip']] = idx

for node in data:
    conns = 0
    node_id = ipdict[node['ip']]
    for peer_ip in node['list']:
        graph.add_edge(node_id, ipdict[peer_ip], None)
        conns += 1
        if conns > conn_limit:
            conns = 0
            break

print("Counting betweenness centrality...")

centrality = rx.betweenness_centrality(graph)

colors = []
for node in graph.node_indices():
    colors.append(centrality[node])
plt.rcParams['figure.figsize'] = [60, 50]
ax = plt.gca()
sm = plt.cm.ScalarMappable(norm=plt.Normalize(
    vmin=min(centrality.values()),
    vmax=max(centrality.values())
))

print("Drawing graph...")
plt.colorbar(sm, ax=ax)
plt.title(f"Betweenness Centrality of peer graph (max {conn_limit} connections per node)")

mpl.rcParams['path.simplify'] = True
mpl.rcParams['path.simplify_threshold'] = 1.0
mpl.rcParams['figure.autolayout'] = True
mpl.rcParams['agg.path.chunksize'] = 100000

mpl_draw(graph, node_color=colors, ax=ax)

print("Showing graph...")
plt.show()