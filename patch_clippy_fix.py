import re

with open("crates/geo-polygonize-core/src/graph/planar_graph.rs", "r") as f:
    content = f.read()

content = re.sub(r'    /// Sorts all outgoing edges of all nodes by angle\.\n+(\s+)pub fn add_line', r'    /// Sorts all outgoing edges of all nodes by angle.\n\1pub fn add_line', content)

with open("crates/geo-polygonize-core/src/graph/planar_graph.rs", "w") as f:
    f.write(content)
