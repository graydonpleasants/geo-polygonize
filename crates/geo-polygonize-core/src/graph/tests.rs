#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::graph::planar_graph::PlanarGraph;
    use crate::types::Line3D;
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
    fn test_bulk_load_duplicate_nodes_different_z() {
        use crate::types::Coord3D;

        let mut graph = PlanarGraph::new();
        // Point 1 with different Zs
        let p1_a = Coord3D {
            x: 5.0,
            y: 5.0,
            z: 10.0,
        };
        let p1_b = Coord3D {
            x: 5.0,
            y: 5.0,
            z: 20.0,
        };
        // Point 2 with different Zs
        let p2_a = Coord3D {
            x: 10.0,
            y: 10.0,
            z: 10.0,
        };
        let p2_b = Coord3D {
            x: 10.0,
            y: 10.0,
            z: 30.0,
        };

        let lines = vec![Line3D::new(p1_a, p2_a, 0), Line3D::new(p1_b, p2_b, 1)];

        graph.bulk_load(lines);

        // Nodes with the exact same (x, y) coordinates should be deduplicated.
        // Even though Z coordinates are different, the deduplication in `bulk_load` ignores Z.
        // We expect only 2 nodes (p1, p2) to be added.
        assert_eq!(graph.nodes_x.len(), 2);

        // However, the edges will remain because they aren't duplicates
        // in terms of the list provided, and they don't have zero length.
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.directed_edges.len(), 4);

        // Verify that the edges point to the same two nodes.
        // We sort the src, dst to safely verify the topology.
        let mut n1 = [graph.directed_edges[0].src, graph.directed_edges[0].dst];
        let mut n2 = [graph.directed_edges[2].src, graph.directed_edges[2].dst];
        n1.sort_unstable();
        n2.sort_unstable();

        assert_eq!(n1, n2);
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
        assert!(
            dst0.0 > 0.0 && dst0.1.abs() < 1e-6,
            "Expected Right (10, 0), got {:?}",
            dst0
        );
        // Up
        assert!(
            dst1.0.abs() < 1e-6 && dst1.1 > 0.0,
            "Expected Up (0, 10), got {:?}",
            dst1
        );
        // Left
        assert!(
            dst2.0 < 0.0 && dst2.1.abs() < 1e-6,
            "Expected Left (-10, 0), got {:?}",
            dst2
        );
        // Down
        assert!(
            dst3.0.abs() < 1e-6 && dst3.1 < 0.0,
            "Expected Down (0, -10), got {:?}",
            dst3
        );
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
        assert_eq!(dangles.len(), 1);

        let b_idx = graph
            .node_map
            .get(&Coord::from((10.0, 0.0)).into())
            .unwrap();
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

    #[test]
    fn test_bulk_load() {
        use geo::Line;

        // Define segments: Square with a diagonal + disconnected segment
        let segments = vec![
            Line::new(Coord::from((0.0, 0.0)), Coord::from((10.0, 0.0))),
            Line::new(Coord::from((10.0, 0.0)), Coord::from((10.0, 10.0))),
            Line::new(Coord::from((10.0, 10.0)), Coord::from((0.0, 10.0))),
            Line::new(Coord::from((0.0, 10.0)), Coord::from((0.0, 0.0))),
            Line::new(Coord::from((0.0, 0.0)), Coord::from((10.0, 10.0))), // Diagonal
            Line::new(Coord::from((20.0, 20.0)), Coord::from((30.0, 30.0))), // Disconnected
        ];

        // 1. Incremental graph
        let mut graph_incremental = PlanarGraph::new();
        for segment in &segments {
            graph_incremental.add_line_string(LineString::from(vec![segment.start, segment.end]));
        }

        // 2. Bulk graph
        let mut graph_bulk = PlanarGraph::new();
        let segments_3d: Vec<Line3D> = segments.iter().map(|l| (*l).into()).collect();
        graph_bulk.bulk_load(segments_3d);

        // 3. Comparisons

        // Check counts
        assert_eq!(
            graph_bulk.nodes_x.len(),
            graph_incremental.nodes_x.len(),
            "Node count mismatch"
        );
        assert_eq!(
            graph_bulk.edges.len(),
            graph_incremental.edges.len(),
            "Edge count mismatch"
        );
        // Directed edges count should match edges * 2
        assert_eq!(
            graph_bulk.directed_edges.len(),
            graph_incremental.directed_edges.len(),
            "Directed edge count mismatch"
        );

        // Helper to get sorted neighbors (by coordinate) for a given node coordinate
        let get_neighbors = |graph: &PlanarGraph, coord: Coord<f64>| -> Vec<Coord<f64>> {
            // Try node_map first
            let mut node_idx = graph.node_map.get(&coord.into()).copied();

            // If not found (e.g. bulk loaded graph does not populate node_map), linear scan
            if node_idx.is_none() {
                for (i, (&x, &y)) in graph.nodes_x.iter().zip(graph.nodes_y.iter()).enumerate() {
                    // Use exact equality as in bulk_load logic
                    if x == coord.x && y == coord.y {
                        node_idx = Some(i);
                        break;
                    }
                }
            }

            if let Some(idx) = node_idx {
                let mut neighbors: Vec<Coord<f64>> = graph.nodes_outgoing[idx]
                    .iter()
                    .map(|&de_idx| {
                        let dst_idx = graph.directed_edges[de_idx].dst;
                        Coord {
                            x: graph.nodes_x[dst_idx],
                            y: graph.nodes_y[dst_idx],
                        }
                    })
                    .collect();
                // Sort for stable comparison
                neighbors.sort_by(|a, b| {
                    a.x.partial_cmp(&b.x)
                        .unwrap()
                        .then(a.y.partial_cmp(&b.y).unwrap())
                });
                neighbors
            } else {
                vec![]
            }
        };

        // Collect all unique points from input
        let mut unique_points: Vec<Coord<f64>> = segments
            .iter()
            .flat_map(|line| vec![line.start, line.end])
            .collect();
        unique_points.sort_by(|a, b| {
            a.x.partial_cmp(&b.x)
                .unwrap()
                .then(a.y.partial_cmp(&b.y).unwrap())
        });
        unique_points.dedup();

        for point in unique_points {
            let neighbors_inc = get_neighbors(&graph_incremental, point);
            let neighbors_bulk = get_neighbors(&graph_bulk, point);

            assert_eq!(
                neighbors_inc, neighbors_bulk,
                "Neighbors mismatch for point {:?}",
                point
            );
        }
    }

    #[test]
    fn test_bulk_load_empty() {
        let mut graph = PlanarGraph::new();
        let lines: Vec<Line3D> = vec![];

        graph.bulk_load(lines);

        assert!(graph.nodes_x.is_empty());
        assert!(graph.edges.is_empty());
        assert!(graph.directed_edges.is_empty());
        assert!(graph.nodes_outgoing.is_empty());
        assert!(graph.node_map.is_empty());
    }

    #[test]
    fn test_bulk_load_zero_length_segment() {
        use crate::types::Coord3D;

        let mut graph = PlanarGraph::new();
        let p0 = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        // Exactly zero length
        let p1 = Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        // Almost zero length (differs by < 1e-12)
        let p2 = Coord3D {
            x: 1e-13,
            y: 1e-13,
            z: 0.0,
        };
        // A valid segment
        let p3 = Coord3D {
            x: 10.0,
            y: 10.0,
            z: 0.0,
        };

        let lines = vec![
            Line3D::new(p0, p1, 0),
            Line3D::new(p0, p2, 1),
            Line3D::new(p0, p3, 2),
        ];

        graph.bulk_load(lines);

        // It should skip the first two zero-length / almost zero-length lines
        // leaving only the valid segment (p0 -> p3).
        // The nodes themselves are still added to `self.nodes_x`, etc.
        // from the `entries` deduplication phase.
        // p0, p1, p2, p3 will be collected. p0 and p1 will be deduplicated.
        // p2 differs by < 1e-12 but dedup checks for exact equality, so p2 will NOT be deduplicated with p0.
        // So nodes are p0, p2, p3 (3 nodes).
        assert_eq!(graph.nodes_x.len(), 3);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.directed_edges.len(), 2);
    }

    #[test]
    fn test_get_cut_edges() {
        let mut graph = PlanarGraph::new();
        // Triangle 1
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (0.0, 10.0)]));
        graph.add_line_string(LineString::from(vec![(0.0, 10.0), (0.0, 0.0)]));

        // Bridge (Cut Edge)
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (20.0, 0.0)]));

        // Triangle 2
        graph.add_line_string(LineString::from(vec![(20.0, 0.0), (30.0, 0.0)]));
        graph.add_line_string(LineString::from(vec![(30.0, 0.0), (20.0, 10.0)]));
        graph.add_line_string(LineString::from(vec![(20.0, 10.0), (20.0, 0.0)]));

        graph.sort_edges();

        let dangles = graph.prune_dangles();
        assert_eq!(dangles.len(), 0);

        // Before extracting rings, all edges are unvisited and unmarked.
        let cut_edges_before = graph.get_cut_edges();
        // Total edges: 3 (Tri 1) + 1 (Bridge) + 3 (Tri 2) = 7 edges.
        assert_eq!(cut_edges_before.len(), 7);

        let rings = graph.get_edge_rings();
        assert_eq!(rings.len(), 2);

        // After ring extraction, the bridge is marked as visited due to being traversed
        // forward and back as a degenerate ring in GEOS/JTS.
        // Therefore, it does not show up in get_cut_edges.
        let cut_edges_after = graph.get_cut_edges();
        assert_eq!(cut_edges_after.len(), 0);
    }

    #[test]
    fn test_get_cut_edges_simple() {
        let mut graph = PlanarGraph::new();

        // Add a single line which doesn't form a ring
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));

        // It's unvisited and unmarked, so it's a cut edge.
        let cut_edges = graph.get_cut_edges();
        assert_eq!(cut_edges.len(), 1);

        let edge = &cut_edges[0];
        assert_eq!(edge.len(), 2);

        let c1 = edge[0];
        let c2 = edge[1];

        assert!(
            (c1.x == 0.0 && c1.y == 0.0 && c2.x == 10.0 && c2.y == 0.0)
                || (c1.x == 10.0 && c1.y == 0.0 && c2.x == 0.0 && c2.y == 0.0)
        );

        // If we prune it as a dangle, it will be marked and should not be returned by get_cut_edges.
        graph.prune_dangles();
        let cut_edges_after_prune = graph.get_cut_edges();
        assert_eq!(cut_edges_after_prune.len(), 0);
    }
}
