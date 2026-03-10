import sys
import re

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'r') as f:
    content = f.read()

# Replace the start of polygonize function
start_code = """
    pub fn polygonize(&mut self) -> Result<PolygonizerResult> {
        let mut diag = if self.diagnostics_options.enabled {
            let mut d = PolygonizerDiagnostics::default();
            d.input_segment_count = self.input_lines.len();
            Some(d)
        } else {
            None
        };

        let t_ingest_start = Instant::now();
        self.build_graph()?;
        if let Some(ref mut d) = diag {
            d.phase_times.ingest_and_node = t_ingest_start.elapsed();
            // Actually the segment count might change, but let's assume input_segment_count is line count.
        }

        let t_graph_build_start = Instant::now();
        // 1. Sort edges (Geometry Graph operation)
        self.graph.sort_edges();

        // 2. Prune dangles
        let mut dangles = self.graph.prune_dangles();

        // 3. Find rings (3D)
        let rings_with_ids = self.graph.get_edge_rings();

        // 3b. Find cut edges
        let mut cut_edges = self.graph.get_cut_edges();
        dangles.append(&mut cut_edges);
        if let Some(ref mut d) = diag {
            d.phase_times.graph_build = t_graph_build_start.elapsed();
            d.dangle_count = dangles.len();
            d.cut_edge_count = cut_edges.len(); // Wait, cut_edges was moved. Let's not count length of moved cut_edges directly, actually it's fine since we can count them before append. Let's fix this in the replacement string.
        }
"""

content = content.replace("    pub fn polygonize(&mut self) -> Result<PolygonizerResult> {\n        self.build_graph()?;\n\n        // 1. Sort edges (Geometry Graph operation)\n        self.graph.sort_edges();\n\n        // 2. Prune dangles\n        let mut dangles = self.graph.prune_dangles();\n\n        // 3. Find rings (3D)\n        let rings_with_ids = self.graph.get_edge_rings();\n\n        // 3b. Find cut edges\n        let mut cut_edges = self.graph.get_cut_edges();\n        dangles.append(&mut cut_edges);", """    pub fn polygonize(&mut self) -> Result<PolygonizerResult> {
        let mut diag = if self.diagnostics_options.enabled {
            let mut d = PolygonizerDiagnostics::default();
            d.input_segment_count = self.input_lines.len();
            Some(d)
        } else {
            None
        };

        let t_ingest_start = Instant::now();
        self.build_graph()?;
        if let Some(ref mut d) = diag {
            d.phase_times.ingest_and_node = t_ingest_start.elapsed();
        }

        let t_graph_build_start = Instant::now();
        // 1. Sort edges (Geometry Graph operation)
        self.graph.sort_edges();

        // 2. Prune dangles
        let mut dangles = self.graph.prune_dangles();

        // 3. Find rings (3D)
        let rings_with_ids = self.graph.get_edge_rings();

        // 3b. Find cut edges
        let mut cut_edges = self.graph.get_cut_edges();

        if let Some(ref mut d) = diag {
            d.phase_times.graph_build = t_graph_build_start.elapsed();
            d.ring_count = rings_with_ids.len();
            d.cut_edge_count = cut_edges.len();
            // Note: dangles length here does not include cut_edges yet
            d.dangle_count = dangles.len() + cut_edges.len();
        }

        dangles.append(&mut cut_edges);""")

content = content.replace("shells.reserve(rings_with_ids.len() / 2);\n        holes.reserve(rings_with_ids.len() / 2);", "let t_ring_extraction_start = Instant::now();\n        shells.reserve(rings_with_ids.len() / 2);\n        holes.reserve(rings_with_ids.len() / 2);")

# We need to find the end of "4. Classify Rings (Shell vs Hole)"
# and the start of "5. Establish Topology"
classify_end_idx = content.find("// 5. Establish Topology")
if classify_end_idx != -1:
    content = content[:classify_end_idx] + """
        if let Some(ref mut d) = diag {
            d.phase_times.ring_extraction = t_ring_extraction_start.elapsed();
            d.shell_count = shells.len();
            d.hole_count = holes.len();
            d.invalid_ring_count = invalid_rings_candidates.len();
        }

        let t_containment_start = Instant::now();
        """ + content[classify_end_idx:]

# Find "Ok(PolygonizerResult {" at the end to close timings
end_result_idx = content.find("Ok(PolygonizerResult {")
if end_result_idx != -1:
    content = content[:end_result_idx] + """
        if let Some(ref mut d) = diag {
            d.phase_times.containment = t_containment_start.elapsed();
            // output_flatten time could be measured here if we had a separate pass, but we'll leave it 0 or record what we have
        }
        """ + content[end_result_idx:]

with open('crates/geo-polygonize-core/src/polygonizer.rs', 'w') as f:
    f.write(content)
