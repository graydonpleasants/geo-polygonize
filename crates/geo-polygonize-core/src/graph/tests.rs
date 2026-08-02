#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::graph::planar_graph::PlanarGraph;
    use crate::types::{Coord3D, Line3D};
    use crate::{CancellationToken, ExecutionPolicy, PolygonizeError};
    use geo_types::{Coord, LineString};
    use std::collections::BTreeSet;

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
    fn dangle_pruning_observes_cancellation() {
        let mut graph = PlanarGraph::new();
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (1.0, 0.0)]));
        let token = CancellationToken::new();
        token.cancel();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        };

        assert!(matches!(
            graph.prune_dangles_with_execution_policy(&policy),
            Err(PolygonizeError::Cancelled { stage }) if stage == "graph_construction"
        ));
    }

    #[test]
    fn ring_operations_observe_cancellation() {
        let token = CancellationToken::new();
        token.cancel();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        };

        let mut graph = PlanarGraph::new();
        assert!(matches!(
            graph.delete_cut_edges_with_execution_policy(&policy, false),
            Err(PolygonizeError::Cancelled { stage }) if stage == "ring_extraction"
        ));

        assert!(matches!(
            graph.get_edge_rings_with_graph_ids_and_execution_policy(
                false, false, &policy, false
            ),
            Err(PolygonizeError::Cancelled { stage }) if stage == "ring_extraction"
        ));
    }

    #[test]
    fn graph_build_operations_observe_cancellation() {
        let token = CancellationToken::new();
        token.cancel();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        };
        let mut graph = PlanarGraph::new();

        assert!(matches!(
            graph.bulk_load_with_execution_policy(vec![], &policy),
            Err(PolygonizeError::Cancelled { stage }) if stage == "graph_construction"
        ));
        assert!(matches!(
            graph.sort_edges_with_execution_policy(&policy),
            Err(PolygonizeError::Cancelled { stage }) if stage == "graph_construction"
        ));
    }

    #[test]
    fn graph_limits_are_checked_before_graph_capacity_growth() {
        let lines = vec![Line3D::new(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            1,
        )];

        let mut graph = PlanarGraph::new();
        let error = graph
            .bulk_load_with_execution_policy(
                lines.clone(),
                &ExecutionPolicy {
                    max_graph_nodes: Some(1),
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            PolygonizeError::ResourceLimitExceeded {
                stage,
                limit: 1,
                observed: 2,
            } if stage == "graph_nodes"
        ));
        assert_eq!(graph.nodes_x.capacity(), 0);

        let mut graph = PlanarGraph::new();
        let error = graph
            .bulk_load_with_execution_policy(
                lines,
                &ExecutionPolicy {
                    max_graph_edges: Some(0),
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert!(matches!(
            error,
            PolygonizeError::ResourceLimitExceeded {
                stage,
                limit: 0,
                observed: 1,
            } if stage == "graph_edges"
        ));
        assert_eq!(graph.edges.capacity(), 0);
        assert_eq!(graph.directed_edges.capacity(), 0);
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

        // Coincident XY edges are dissolved while all source IDs are retained.
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.directed_edges.len(), 2);
        assert_eq!(graph.edges[0].sources.line_ids.as_slice(), &[0, 1]);

        // Verify that the edges point to the same two nodes.
        // We sort the src, dst to safely verify the topology.
        let mut n1 = [graph.directed_edges[0].src, graph.directed_edges[0].dst];
        n1.sort_unstable();
        assert_ne!(n1[0], n1[1]);
    }

    #[test]
    fn test_incremental_edges_merge_and_remove_sources() {
        use crate::types::Coord3D;

        let mut graph = PlanarGraph::new();
        let start = Coord3D::new(0.0, 0.0, 0.0);
        let end = Coord3D::new(1.0, 0.0, 0.0);
        let first = graph.add_line(Line3D::new(start, end, 10));
        let second = graph.add_line(Line3D::new(end, start, 20));

        assert_eq!(first, second);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].sources.line_ids.as_slice(), &[10, 20]);

        assert!(graph.remove_line_by_id(10));
        assert!(!graph.edges[0].deleted);
        assert_eq!(graph.edges[0].line.line_id, 20);
        assert!(graph.remove_line_by_id(20));
        assert!(graph.edges[0].deleted);
    }

    fn arrangement_graph() -> PlanarGraph {
        let mut graph = PlanarGraph::new();
        let center = Coord3D::new(0.0, 0.0, 0.0);
        for (line_id, end) in [
            (10, Coord3D::new(10.0, 0.0, 0.0)),
            (20, Coord3D::new(0.0, 10.0, 0.0)),
            (30, Coord3D::new(-10.0, 0.0, 0.0)),
            (40, Coord3D::new(0.0, -10.0, 0.0)),
        ] {
            graph.add_line(Line3D::new(center, end, line_id));
        }
        graph.add_line(Line3D::new(Coord3D::new(10.0, 0.0, 0.0), center, 11));
        graph.sort_edges();
        graph
    }

    fn arrangement_invariant_reason(graph: &PlanarGraph) -> String {
        match graph.validate_arrangement_edge_invariants().unwrap_err() {
            PolygonizeError::InternalInvariantViolation { reason } => reason,
            error => panic!("unexpected validation error: {error}"),
        }
    }

    #[test]
    fn arrangement_edge_validator_accepts_bulk_incremental_and_deleted_edges() {
        let incremental = arrangement_graph();
        incremental.validate_arrangement_edge_invariants().unwrap();

        let center = Coord3D::new(0.0, 0.0, 0.0);
        let east = Coord3D::new(10.0, 0.0, 0.0);
        let mut bulk = PlanarGraph::new();
        bulk.bulk_load(vec![
            Line3D::new(center, east, 10),
            Line3D::new(east, center, 11),
            Line3D::new(center, Coord3D::new(0.0, 10.0, 0.0), 20),
        ]);
        bulk.sort_edges();
        bulk.validate_arrangement_edge_invariants().unwrap();

        assert!(bulk.remove_line_by_id(10));
        assert!(bulk.remove_line_by_id(11));
        bulk.sort_edges();
        bulk.validate_arrangement_edge_invariants().unwrap();
        assert_eq!(bulk.nodes_degree[0], 1);
        assert_eq!(bulk.nodes_degree[1], 0);
    }

    #[test]
    fn arrangement_edge_validator_reports_deterministic_witnesses() {
        let graph = arrangement_graph();

        let mut broken = graph.clone();
        broken.directed_edges.pop();
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant directed edge count mismatch: edges=4, directed_edges=7, expected=8"
        );

        let mut broken = graph.clone();
        broken.directed_edges[0].sym_idx = 0;
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant edge 0 twin involution mismatch: 0.sym=0, 1.sym=0"
        );

        let mut broken = graph.clone();
        broken.directed_edges[1].dst = 2;
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant edge 0 twin endpoint mismatch: 0=(0->1), 1=(1->2)"
        );

        let mut broken = graph.clone();
        broken.edges[0].sources.line_ids.clear();
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant live edge 0 has no sources"
        );

        let mut broken = graph.clone();
        broken.edges[0].sources.line_ids.swap(0, 1);
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant live edge 0 sources are not strictly sorted: 11 then 10"
        );

        let mut broken = graph.clone();
        broken.nodes_degree[0] -= 1;
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant node 0 degree mismatch: degree=3, adjacency=4"
        );

        let mut broken = graph.clone();
        broken.edges[0].deleted = true;
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant node 0 adjacency[0] references deleted edge 0 via directed edge 0"
        );

        let mut broken = graph.clone();
        broken.nodes_outgoing[0].remove(0);
        broken.nodes_degree[0] -= 1;
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant directed edge 0 adjacency count mismatch: actual=0, expected=1"
        );

        let mut broken = graph;
        broken.nodes_outgoing[0].swap(0, 1);
        assert_eq!(
            arrangement_invariant_reason(&broken),
            "arrangement edge invariant node 0 angular order is not strict between directed edges 2 and 0: Greater"
        );
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
    fn maximal_ring_trace_stops_before_budgeted_materialization() {
        let mut graph = PlanarGraph::new();
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        graph.add_line_string(LineString::from(vec![(10.0, 0.0), (0.0, 10.0)]));
        graph.add_line_string(LineString::from(vec![(0.0, 10.0), (0.0, 0.0)]));
        graph.sort_edges();

        let (maximal, minimal, truncated) = graph
            .get_edge_rings_with_maximal_and_execution_policy(
                false,
                false,
                &ExecutionPolicy::default(),
                0,
                false,
            )
            .unwrap();

        assert!(maximal.is_empty());
        assert_eq!(minimal.len(), 2);
        assert!(truncated);
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

        // Only the exactly zero-length line is skipped. Small nonzero geometry
        // must not disappear because of an absolute coordinate tolerance.
        // The nodes themselves are still added to `self.nodes_x`, etc.
        // from the `entries` deduplication phase.
        // p0, p1, p2, p3 will be collected. p0 and p1 will be deduplicated.
        // p2 differs by < 1e-12 but dedup checks for exact equality, so p2 will NOT be deduplicated with p0.
        // So nodes are p0, p2, p3 (3 nodes).
        assert_eq!(graph.nodes_x.len(), 3);
        assert_eq!(graph.edges.len(), 2);
        assert_eq!(graph.directed_edges.len(), 4);
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

        let cut_edges = graph.delete_cut_edges();
        assert_eq!(cut_edges.len(), 1);

        let rings = graph.get_edge_rings();
        assert_eq!(rings.len(), 4);
    }

    #[test]
    fn test_get_cut_edges_simple() {
        let mut graph = PlanarGraph::new();

        // Add a single line which doesn't form a ring
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));

        let cut_edges = graph.delete_cut_edges();
        assert_eq!(cut_edges.len(), 1);

        let edge = &cut_edges[0];
        assert_eq!(edge.len(), 2);

        let c1 = edge[0];
        let c2 = edge[1];

        assert!(
            (c1.x == 0.0 && c1.y == 0.0 && c2.x == 10.0 && c2.y == 0.0)
                || (c1.x == 10.0 && c1.y == 0.0 && c2.x == 0.0 && c2.y == 0.0)
        );

        let mut graph = PlanarGraph::new();
        graph.add_line_string(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]));
        graph.prune_dangles();
        let cut_edges_after_prune = graph.delete_cut_edges();
        assert_eq!(cut_edges_after_prune.len(), 0);
    }

    #[test]
    fn component_processing_remaps_ring_graph_ids_to_the_global_graph() {
        let mut graph = PlanarGraph::new();
        for (offset, points) in [
            [
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(2.0, 0.0, 0.0),
                Coord3D::new(1.0, 1.0, 0.0),
            ],
            [
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(12.0, 0.0, 0.0),
                Coord3D::new(11.0, 1.0, 0.0),
            ],
        ]
        .into_iter()
        .enumerate()
        {
            for edge in 0..3 {
                graph.add_line(Line3D::new(
                    points[edge],
                    points[(edge + 1) % 3],
                    (offset * 10 + edge) as u32,
                ));
            }
        }

        let component_ids = graph.active_component_ids();
        let ((dangles, cut_edges, maximal, rings), capture_truncated) = graph
            .process_components_with_execution_policy(
                true,
                true,
                &ExecutionPolicy::default(),
                true,
                None,
            )
            .unwrap();
        assert!(dangles.is_empty());
        assert!(cut_edges.is_empty());
        assert!(maximal.is_empty());
        assert!(!capture_truncated);
        assert_eq!(rings.len(), 4);

        let mut represented_components = BTreeSet::new();
        for ring in rings {
            let component = component_ids[ring.node_ids[0]].unwrap();
            represented_components.insert(component);
            assert!(ring
                .node_ids
                .iter()
                .all(|&node| component_ids[node] == Some(component)));
            assert!(ring.edge_keys.iter().all(|&(start, end)| {
                component_ids[start] == Some(component) && component_ids[end] == Some(component)
            }));
        }
        assert_eq!(represented_components.len(), 2);
    }
}
