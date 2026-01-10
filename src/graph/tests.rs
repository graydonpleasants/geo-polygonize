#[cfg(test)]
mod tests {
    use crate::graph::planar_graph::PlanarGraph;
    use geo_types::{Coord, LineString};

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
        // 1. Right (0 degrees) -> dx=10, dy=0
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        // 2. Up (90 degrees) -> dx=0, dy=10
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (0.0, 10.0)]));
        // 3. Left (180 degrees) -> dx=-10, dy=0
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (-10.0, 0.0)]));
        // 4. Down (-90 degrees) -> dx=0, dy=-10
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (0.0, -10.0)]));

        graph.sort_edges();

        let center_node_idx = graph.node_map.get(&Coord::from((0.0, 0.0)).into()).unwrap();

        let edges = &graph.nodes_outgoing[*center_node_idx];
        assert_eq!(edges.len(), 4);

        // We expect the sort order to be CCW starting from +X axis.
        // Right, Up, Left, Down
        // Check destination coordinates to verify.
        let get_dst = |idx: usize| -> (f64, f64) {
            let dst_node_idx = graph.directed_edges[idx].dst;
            (graph.nodes_x[dst_node_idx], graph.nodes_y[dst_node_idx])
        };

        let dst0 = get_dst(edges[0]);
        let dst1 = get_dst(edges[1]);
        let dst2 = get_dst(edges[2]);
        let dst3 = get_dst(edges[3]);

        // Right
        assert!(dst0.0 > 0.0 && dst0.1.abs() < 1e-6, "Expected Right (10, 0), got {:?}", dst0);
        // Up
        assert!(dst1.0.abs() < 1e-6 && dst1.1 > 0.0, "Expected Up (0, 10), got {:?}", dst1);
        // Left
        assert!(dst2.0 < 0.0 && dst2.1.abs() < 1e-6, "Expected Left (-10, 0), got {:?}", dst2);
        // Down
        assert!(dst3.0.abs() < 1e-6 && dst3.1 < 0.0, "Expected Down (0, -10), got {:?}", dst3);
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
