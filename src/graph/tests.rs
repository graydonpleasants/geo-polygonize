#[cfg(test)]
mod tests {
    use crate::graph::planar_graph::PlanarGraph;
    use geo_types::{Coord, LineString};
    use crate::utils::pseudo_angle;

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
        // 1. Right (0 degrees) -> dx=10, dy=0 -> pseudo 0.0
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        // 2. Up (90 degrees) -> dx=0, dy=10 -> pseudo 1.0
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (0.0, 10.0)]));
        // 3. Left (180 degrees) -> dx=-10, dy=0 -> pseudo 2.0
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (-10.0, 0.0)]));
        // 4. Down (-90 degrees) -> dx=0, dy=-10 -> pseudo 3.0
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (0.0, -10.0)]));

        graph.sort_edges();

        let center_node_idx = graph.node_map.get(&Coord::from((0.0, 0.0)).into()).unwrap();

        let sorted_angles: Vec<f64> = graph.nodes_outgoing[*center_node_idx].iter().map(|&idx| {
            graph.directed_edges[idx].angle
        }).collect();

        // Should be sorted 0.0, 1.0, 2.0, 3.0
        // But wait, are they sorted or just stored?
        // graph.sort_edges() sorts them.

        assert!(sorted_angles[0] < sorted_angles[1]);
        assert!(sorted_angles[1] < sorted_angles[2]);
        assert!(sorted_angles[2] < sorted_angles[3]);

        // Check values match expectations for pseudo_angle
        // Right
        assert!((sorted_angles[0] - 0.0).abs() < 1e-6, "Expected 0.0, got {}", sorted_angles[0]);
        // Up
        assert!((sorted_angles[1] - 1.0).abs() < 1e-6, "Expected 1.0, got {}", sorted_angles[1]);
        // Left
        assert!((sorted_angles[2] - 2.0).abs() < 1e-6, "Expected 2.0, got {}", sorted_angles[2]);
        // Down
        assert!((sorted_angles[3] - 3.0).abs() < 1e-6, "Expected 3.0, got {}", sorted_angles[3]);
    }

    #[test]
    fn test_dangle_pruning() {
        let mut graph = PlanarGraph::new();
        // Triangle with a dangle
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (0.0, 10.0)]));
        graph.add_line_string(LineString::from(vec![(0.0, 10.0), (0.0, 0.0)]));

        // Dangle at B
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (20.0, 0.0)]));

        graph.sort_edges();

        let dangles = graph.prune_dangles();
        assert_eq!(dangles, 1);

        let b_idx = graph.node_map.get(&Coord::from((10.0, 0.0)).into()).unwrap();
        assert_eq!(graph.nodes_degree[*b_idx], 2);
    }

    #[test]
    fn test_simple_cycle() {
        let mut graph = PlanarGraph::new();
        // Triangle
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (0.0, 10.0)]));
        graph.add_line_string(LineString::from(vec![(0.0, 10.0), (0.0, 0.0)]));

        graph.sort_edges();
        let rings = graph.get_edge_rings();

        assert_eq!(rings.len(), 2);
    }
}
