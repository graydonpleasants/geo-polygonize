#[cfg(test)]
mod tests {
    use crate::graph::planar_graph::PlanarGraph;
    use geo_types::{Coord, LineString};
    use std::f64::consts::PI;

    #[test]
    fn test_graph_construction() {
        let mut graph = PlanarGraph::new();
        let l1 = LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]);
        let l2 = LineString::from(vec![(0.0, 0.0), (0.0, 10.0)]);

        graph.add_line_string(l1);
        graph.add_line_string(l2);

        assert_eq!(graph.nodes_x.len(), 3); // (0,0), (10,0), (0,10)
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.directed_edges.len(), 4);

        // Node at (0,0) should have 2 outgoing edges
        let center_node_idx = graph.node_map.get(&Coord::from((0.0, 0.0)).into()).unwrap();
        assert_eq!(graph.nodes_outgoing[*center_node_idx].len(), 2);
    }

    #[test]
    fn test_edge_sorting() {
        let mut graph = PlanarGraph::new();
        // Add 4 edges radiating from (0,0)
        // 1. Right (0 degrees)
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        // 2. Up (90 degrees / PI/2)
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (0.0, 10.0)]));
        // 3. Left (180 degrees / PI)
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (-10.0, 0.0)]));
        // 4. Down (-90 degrees / -PI/2)
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (0.0, -10.0)]));

        graph.sort_edges();

        let center_node_idx = graph.node_map.get(&Coord::from((0.0, 0.0)).into()).unwrap();

        let sorted_angles: Vec<f64> = graph.nodes_outgoing[*center_node_idx].iter().map(|&idx| {
            graph.directed_edges[idx].angle
        }).collect();

        // Expected order: -PI/2, 0, PI/2, PI (or -PI)
        // atan2 returns range (-PI, PI]
        // (0, -10) -> atan2(-10, 0) = -PI/2 = -1.57
        // (10, 0) -> atan2(0, 10) = 0
        // (0, 10) -> atan2(10, 0) = PI/2 = 1.57
        // (-10, 0) -> atan2(0, -10) = PI = 3.14

        assert!(sorted_angles[0] < sorted_angles[1]);
        assert!(sorted_angles[1] < sorted_angles[2]);
        assert!(sorted_angles[2] < sorted_angles[3]);

        assert!((sorted_angles[0] - (-PI/2.0)).abs() < 1e-6);
        assert!((sorted_angles[1] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_dangle_pruning() {
        let mut graph = PlanarGraph::new();
        // Triangle with a dangle
        // A(0,0) - B(10,0) - C(0,10) - A(0,0)
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (0.0, 10.0)]));
        graph.add_line_string(LineString::from(vec![(0.0, 10.0), (0.0, 0.0)]));

        // Dangle at B
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (20.0, 0.0)]));

        // Before sort
        // B connects to A, C, D. Degree 3.
        // D connects to B. Degree 1.

        graph.sort_edges();

        let dangles = graph.prune_dangles();
        assert_eq!(dangles, 1); // Only the edge B-D (node D) should be removed.

        // B should have degree 2 now
        let b_idx = graph.node_map.get(&Coord::from((10.0, 0.0)).into()).unwrap();
        assert_eq!(graph.nodes_degree[*b_idx], 2);
    }

    #[test]
    fn test_simple_cycle() {
        let mut graph = PlanarGraph::new();
        // Triangle
        // (0,0) -> (10,0) -> (0,10) -> (0,0)
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (0.0, 10.0)]));
        graph.add_line_string(LineString::from(vec![(0.0, 10.0), (0.0, 0.0)]));

        graph.sort_edges();
        let rings = graph.get_edge_rings();

        // Should find 2 rings (Inner CCW, Outer CW)
        assert_eq!(rings.len(), 2);
    }
}
