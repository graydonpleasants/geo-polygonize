#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::tiling::{
        ComponentFallbackDecision, ComponentFallbackDeclineReason, InputComponent,
    };
    use crate::{
        trace::TraceLevelV1, CancellationToken, Coord3D, DedupPolicy, ExecutionPolicy, Line3D,
        NodingGuarantee, NodingOptions, Polygon3D, PolygonizeError, Polygonizer,
        PolygonizerOptions, PrecisionModel, ProvenanceOptions, TileBoundarySide,
        TileComponentConnection, TileCoverageGuarantee, TileCoverageIssue,
        TileCoverageResolutionKind, TileExcludedComponentIssue, TileExecutionPolicy, TileReport,
        TileRetryPolicy, TiledPolygonizeError, TiledPolygonizer, TiledStitchedOutput, ZOptions,
    };
    use geo::{BoundingRect, Contains, Coord, Geometry, LineString, MultiLineString, Rect};
    use std::collections::BTreeSet;

    #[test]
    fn source_segment_sink_retains_chain_and_endpoint_identity() {
        let geometry = Geometry::MultiLineString(MultiLineString(vec![
            LineString::new(vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 0.0 }]),
            LineString::new(vec![
                Coord { x: 1.0, y: 1.0 },
                Coord { x: 2.0, y: 1.0 },
                Coord { x: 3.0, y: 1.0 },
            ]),
        ]));
        let geometries = vec![(&geometry, None)];
        let sink = crate::tiling::partition_source_segment_sink(&geometries, &[0]).unwrap();

        assert_eq!(sink.segments.len(), 3);
        assert_eq!(sink.segments[0].geometry_index, 0);
        assert_eq!(sink.segments[0].source.chain_index, 0);
        assert_eq!(sink.segments[0].source.segment_index, 0);
        assert_eq!(sink.segments[0].source.chain_segment_count, 1);
        assert_eq!(sink.segments[0].source.source_id, None);
        assert_eq!(sink.segments[0].line.line_id, 0);
        assert_eq!(sink.segments[0].raw_start_z_bits, 0.0f64.to_bits());
        assert_eq!(sink.segments[0].raw_end_z_bits, 0.0f64.to_bits());
        assert_eq!(sink.segments[2].source.chain_index, 1);
        assert_eq!(sink.segments[2].source.segment_index, 1);
        assert_eq!(sink.segments[2].source.chain_segment_count, 2);
        assert_eq!(sink.segments[2].line.end.x, 3.0);
    }

    #[test]
    fn inner_box_fast_path_requires_strict_halo_clearance() {
        let tile = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 });
        let line = |start: (f64, f64), end: (f64, f64)| {
            Line3D::new(
                Coord3D::new(start.0, start.1, 4.0),
                Coord3D::new(end.0, end.1, 8.0),
                3,
            )
        };

        assert!(
            crate::tiling::TiledPolygonizer::partition_inner_box_contains_segment(
                line((3.0, 3.0), (7.0, 7.0)),
                tile,
                2.0,
            )
        );
        assert!(
            !crate::tiling::TiledPolygonizer::partition_inner_box_contains_segment(
                line((2.0, 3.0), (7.0, 7.0)),
                tile,
                2.0,
            )
        );
        assert!(
            !crate::tiling::TiledPolygonizer::partition_inner_box_contains_segment(
                line((3.0, 3.0), (8.0, 7.0)),
                tile,
                2.0,
            )
        );
        assert!(
            !crate::tiling::TiledPolygonizer::partition_inner_box_contains_segment(
                line((3.0, 3.0), (7.0, 7.0)),
                tile,
                5.0,
            )
        );
        assert!(
            crate::tiling::TiledPolygonizer::partition_inner_box_contains_segment(
                line((1.0, 1.0), (9.0, 9.0)),
                tile,
                0.0,
            )
        );
    }

    #[test]
    fn streams_inner_and_boundary_segments_without_candidate_collection() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 2.0 },
                Coord { x: 8.0, y: 8.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 9.5, y: 2.0 },
                Coord { x: 10.5, y: 2.0 },
            ])),
        ];
        let geometries = geometries
            .iter()
            .map(|geometry| (geometry, geometry.bounding_rect()))
            .collect::<Vec<_>>();
        let source_segments =
            crate::tiling::partition_source_segment_sink(&geometries, &[0, 1]).unwrap();
        let tiled = TiledPolygonizer::new(bbox, 10.0).with_buffer(1.0);
        let tiles = tiled.generate_tiles().unwrap();
        let sinks = tiled
            .stream_source_segments_to_partition_sinks(&tiles, &source_segments)
            .unwrap();

        assert_eq!(sinks.len(), 4);
        assert_eq!(sinks[0].segments.len(), 2);
        assert_eq!(sinks[1].segments.len(), 1);
        assert!(sinks[2].segments.is_empty());
        assert!(sinks[3].segments.is_empty());
        assert_eq!(sinks[0].segments[0].geometry_index, 0);
        assert_eq!(sinks[0].segments[1].geometry_index, 1);
        assert_eq!(sinks[1].segments[0].geometry_index, 1);
    }

    #[test]
    fn exports_physical_tile_border_observations_before_scratch_is_released() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 1.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 1.0, y: 0.0 },
                Coord { x: 2.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 1.0 },
                Coord { x: 1.0, y: 1.0 },
                Coord { x: 2.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 0.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 1.0, y: 0.0 },
                Coord { x: 1.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 0.0 },
                Coord { x: 2.0, y: 1.0 },
            ])),
        ];
        let mut tiled = TiledPolygonizer::new(bbox, 1.0).with_buffer(0.0);
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }

        let result = tiled.polygonize().unwrap();
        let start = crate::graph::partition_border::PartitionBorderNodeKey::from_coord(
            Coord3D::new(1.0, 0.0, 0.0),
        );
        let end = crate::graph::partition_border::PartitionBorderNodeKey::from_coord(Coord3D::new(
            1.0, 1.0, 0.0,
        ));
        let key = crate::graph::partition_border::PartitionBorderEdgeKey::new(start, end).unwrap();
        let observations = result
            .partition_border_graph
            .edge_observations(key)
            .unwrap();

        assert_eq!(observations.len(), 4);
        assert!(observations
            .iter()
            .all(|observation| observation.face_ref.is_some()));
        let reconciliation = result.partition_border_graph.reconciliation_stats();
        assert_eq!(reconciliation.declared_adjacency_count, 1);
        assert_eq!(
            result.stitching_report.partition_border_adjacency_count,
            reconciliation.declared_adjacency_count
        );
        assert_eq!(
            result.stitching_report.partition_border_twin_count,
            reconciliation.matched_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_unmatched_edge_count,
            reconciliation.unmatched_edge_count
        );
        assert_eq!(
            result.stitching_report.partition_border_face_twin_count,
            result.partition_border_graph.applied_face_twins().len()
        );
        assert_eq!(
            result.stitching_report.partition_border_face_twin_count
                + result
                    .stitching_report
                    .partition_border_face_twin_missing_face_count
                + result
                    .stitching_report
                    .partition_border_face_twin_invalid_face_count,
            reconciliation.matched_twin_count
        );
        let global_face_edge_map = result.partition_border_graph.global_face_edge_map();
        let global_face_edge_map_stats = result
            .partition_border_graph
            .clone()
            .reconcile_global_face_edge_map(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_edge_map_local_graph_count,
            result.partition_border_graph.local_face_graphs().len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_edge_map_directed_edge_count,
            global_face_edge_map.len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_edge_map_local_successor_count,
            global_face_edge_map_stats.local_successor_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_edge_map_observation_count,
            global_face_edge_map_stats.mapped_observation_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_edge_map_twin_count,
            global_face_edge_map_stats.mapped_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_edge_map_unmapped_twin_count,
            global_face_edge_map_stats.unmapped_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_edge_map_ready,
            global_face_edge_map_stats.edge_map_ready
        );
        assert!(global_face_edge_map.iter().all(|edge| {
            edge.symmetric_global_dir_edge_id < global_face_edge_map.len()
                && edge
                    .local_face_successor_global_dir_edge_id
                    .is_none_or(|successor| successor < global_face_edge_map.len())
        }));
        let global_face_nodes = result.partition_border_graph.global_face_nodes();
        let global_face_node_stats = result
            .partition_border_graph
            .clone()
            .reconcile_global_face_nodes(ZOptions::default(), &ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_node_edge_count,
            global_face_node_stats.edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_node_count,
            global_face_nodes.len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_node_endpoint_count,
            global_face_node_stats.endpoint_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_node_observation_count,
            global_face_node_stats.mapped_observation_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_node_unmapped_observation_count,
            global_face_node_stats.unmapped_observation_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_node_z_candidate_count,
            global_face_node_stats.z_candidate_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_node_z_conflict_count,
            global_face_node_stats.z_conflict_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_node_ready,
            global_face_node_stats.node_map_ready
        );
        assert!(global_face_edge_map.iter().all(|edge| {
            edge.from_global_node_id
                .is_some_and(|node| node < global_face_nodes.len())
                && edge
                    .to_global_node_id
                    .is_some_and(|node| node < global_face_nodes.len())
        }));
        let global_face_next_application_stats = result
            .partition_border_graph
            .clone()
            .reconcile_global_face_next_application_plans(&ExecutionPolicy::default())
            .unwrap();
        let global_face_next_application_plans = result
            .partition_border_graph
            .global_face_next_application_plans();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_application_plan_count,
            global_face_next_application_stats.plan_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_application_link_count,
            global_face_next_application_stats.candidate_link_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_application_edge_count,
            global_face_next_application_stats.mapped_edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_application_twin_count,
            global_face_next_application_stats.mapped_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_application_unmapped_observation_count,
            global_face_next_application_stats.unmapped_observation_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_application_incomplete_plan_count,
            global_face_next_application_stats.incomplete_plan_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_application_node_discontinuity_count,
            global_face_next_application_stats.node_discontinuity_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_application_ready,
            global_face_next_application_stats.application_ready
        );
        assert!(global_face_next_application_plans.iter().all(|plan| {
            plan.global_dir_edge_ids.len() == plan.successor_global_dir_edge_ids.len()
                || !plan.closed
        }));
        let global_topology_candidate_stats = result
            .partition_border_graph
            .clone()
            .reconcile_global_topology_candidate(&ExecutionPolicy::default())
            .unwrap();
        let global_topology_candidate = result
            .partition_border_graph
            .global_topology_candidate()
            .expect("detached global topology candidate");
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_edge_count,
            global_topology_candidate_stats.edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_local_successor_count,
            global_topology_candidate_stats.local_successor_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_global_override_count,
            global_topology_candidate_stats.global_override_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_assigned_next_count,
            global_topology_candidate_stats.assigned_next_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_unassigned_next_count,
            global_topology_candidate_stats.unassigned_next_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_cycle_count,
            global_topology_candidate_stats.cycle_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_closed_cycle_edge_count,
            global_topology_candidate_stats.closed_cycle_edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_predecessor_conflict_count,
            global_topology_candidate_stats.predecessor_conflict_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_node_discontinuity_count,
            global_topology_candidate_stats.node_discontinuity_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_incomplete_application_plan_count,
            global_topology_candidate_stats.incomplete_application_plan_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_candidate_ready,
            global_topology_candidate_stats.candidate_ready
        );
        assert_eq!(
            global_topology_candidate.next_global_dir_edge_ids.len(),
            global_topology_candidate_stats.edge_count
        );
        let global_topology_application_gate_stats = result
            .partition_border_graph
            .validate_global_topology_application_gate(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_edge_count,
            global_topology_application_gate_stats.edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_successor_count,
            global_topology_application_gate_stats.candidate_successor_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_adjacency_count,
            global_topology_application_gate_stats.declared_adjacency_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_applied_twin_count,
            global_topology_application_gate_stats.applied_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_mapped_twin_count,
            global_topology_application_gate_stats.mapped_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_unmapped_twin_count,
            global_topology_application_gate_stats.unmapped_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_invalid_twin_count,
            global_topology_application_gate_stats.invalid_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_predecessor_conflict_count,
            global_topology_application_gate_stats.predecessor_conflict_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_node_discontinuity_count,
            global_topology_application_gate_stats.node_discontinuity_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_topology_application_gate_ready,
            global_topology_application_gate_stats.application_ready
        );
        let global_component_coverage_stats = result
            .partition_border_graph
            .validate_global_component_coverage(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_component_count,
            global_component_coverage_stats.component_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_face_count,
            global_component_coverage_stats.face_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_edge_count,
            global_component_coverage_stats.edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_face_edge_count,
            global_component_coverage_stats.face_edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_covered_face_edge_count,
            global_component_coverage_stats.covered_face_edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_uncovered_face_edge_count,
            global_component_coverage_stats.uncovered_face_edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_duplicate_face_count,
            global_component_coverage_stats.duplicate_face_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_duplicate_twin_edge_count,
            global_component_coverage_stats.duplicate_twin_edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_coverage_ready,
            global_component_coverage_stats.coverage_ready
        );
        let global_face_id_application_stats = result
            .partition_border_graph
            .validate_global_face_id_application(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_application_candidate_cycle_count,
            global_face_id_application_stats.candidate_cycle_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_application_assigned_face_count,
            global_face_id_application_stats.assigned_face_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_application_cycle_start_count,
            global_face_id_application_stats.candidate_cycle_start_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_application_mapped_cycle_count,
            global_face_id_application_stats.mapped_cycle_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_application_unmapped_plan_count,
            global_face_id_application_stats.unmapped_plan_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_application_duplicate_face_id_count,
            global_face_id_application_stats.duplicate_face_id_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_application_non_contiguous_face_id_count,
            global_face_id_application_stats.non_contiguous_face_id_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_application_ready,
            global_face_id_application_stats.application_ready
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_reconciled_node_count,
            result
                .partition_border_graph
                .reconciled_border_nodes()
                .len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_node_z_conflict_count,
            result
                .partition_border_graph
                .reconciled_border_nodes()
                .iter()
                .filter(|node| node.z_conflict)
                .count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_count,
            result.partition_border_graph.global_components().len()
        );
        let component_payloads = result.partition_border_graph.global_component_payloads();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_payload_count,
            component_payloads.len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_payload_source_line_count,
            component_payloads
                .iter()
                .map(|payload| payload.source_line_ids.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_payload_representative_line_count,
            component_payloads
                .iter()
                .map(|payload| payload.representative_line_ids.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_payload_z_candidate_count,
            component_payloads
                .iter()
                .map(|payload| payload.z_bits.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_payload_selected_z_node_count,
            component_payloads
                .iter()
                .map(|payload| payload.selected_z_bits.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_payload_z_conflict_node_count,
            component_payloads
                .iter()
                .map(|payload| payload.z_conflict_node_count)
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_component_payload_z_conflict_component_count,
            component_payloads
                .iter()
                .filter(|payload| payload.z_conflict_node_count > 0)
                .count()
        );
        assert_eq!(
            result.stitching_report.partition_border_global_face_count,
            result
                .partition_border_graph
                .global_components()
                .iter()
                .map(|component| component.face_refs.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_linked_face_count,
            result
                .partition_border_graph
                .global_components()
                .iter()
                .filter(|component| !component.twin_edge_keys.is_empty())
                .map(|component| component.face_refs.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_plan_count,
            result.partition_border_graph.global_face_plans().len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_candidate_count,
            result
                .partition_border_graph
                .global_face_plans()
                .iter()
                .map(|plan| plan.candidates.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_count,
            result
                .partition_border_graph
                .global_face_plans()
                .iter()
                .filter(|plan| plan.local_face_is_unbounded)
                .count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_linked_count,
            result
                .partition_border_graph
                .global_face_plans()
                .iter()
                .filter(|plan| !plan.twin_edge_keys.is_empty())
                .count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_missing_boundary_successor_count,
            result
                .partition_border_graph
                .global_face_plans()
                .iter()
                .flat_map(|plan| plan.candidates.iter())
                .filter(|candidate| candidate.local_face_boundary_successor.is_none())
                .count()
        );
        let validation = result
            .partition_border_graph
            .validate_global_face_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_validated_count,
            validation.face_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_validated_candidate_count,
            validation.candidate_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_validated_twin_count,
            validation.twin_link_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_validated_unbounded_count,
            validation.unbounded_face_count
        );
        let mutation_gate = result
            .partition_border_graph
            .validate_global_face_mutation_gate(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_boundary_transition_count,
            mutation_gate.boundary_transition_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_mutation_missing_successor_count,
            mutation_gate.missing_boundary_successor_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_mutation_ready_count,
            mutation_gate.mutation_ready_face_count
        );
        let transition_plan = result.partition_border_graph.global_face_transitions();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_transition_count,
            transition_plan
                .iter()
                .filter(|plan| plan.closed)
                .map(|plan| plan.boundary_observation_ids.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_transition_closed_count,
            transition_plan.iter().filter(|plan| plan.closed).count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_transition_incomplete_count,
            transition_plan.iter().filter(|plan| !plan.closed).count()
        );
        let twin_transition_plan = result.partition_border_graph.global_face_twin_transitions();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_twin_transition_count,
            twin_transition_plan.len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_twin_transition_ready_count,
            twin_transition_plan
                .iter()
                .filter(|link| link.forward_cycle_closed && link.reverse_cycle_closed)
                .count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_twin_transition_unmapped_count,
            result.stitching_report.partition_border_face_twin_count - twin_transition_plan.len()
        );
        let face_walk = result
            .partition_border_graph
            .validate_global_face_walk_invariants(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_walk_validated_count,
            face_walk.face_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_walk_closed_count,
            face_walk.closed_face_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_walk_source_complete_twin_count,
            face_walk.source_complete_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_walk_unbounded_component_count,
            face_walk.unbounded_component_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_walk_face_adjacency_cycle_rank,
            face_walk.face_adjacency_cycle_rank
        );
        let face_euler = result
            .partition_border_graph
            .validate_global_face_euler_witness(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_euler_transition_face_count,
            face_euler.transition_face_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_euler_closed_boundary_cycle_count,
            face_euler.closed_boundary_cycle_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_euler_boundary_vertex_count,
            face_euler.boundary_vertex_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_euler_boundary_edge_count,
            face_euler.boundary_edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_euler_cross_component_edge_count,
            face_euler.cross_component_edge_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_euler_boundary_lhs,
            face_euler.boundary_euler_lhs
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_euler_boundary_rhs,
            face_euler.boundary_euler_rhs
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_euler_boundary_consistent,
            face_euler.boundary_euler_consistent
        );
        assert!(face_euler.cross_component_edge_count > 0);
        assert!(!face_euler.boundary_euler_consistent);
        let next_candidates = result.partition_border_graph.global_face_next_candidates();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_candidate_count,
            next_candidates.len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_ready_candidate_count,
            next_candidates
                .iter()
                .filter(|candidate| candidate.ready)
                .count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_incomplete_candidate_count,
            next_candidates
                .iter()
                .filter(|candidate| !candidate.ready)
                .count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_candidate_count,
            twin_transition_plan.len()
        );
        assert!(next_candidates
            .iter()
            .all(|candidate| candidate.forward_global_successor.is_some() == candidate.ready));
        let global_successor_count = next_candidates
            .iter()
            .flat_map(|candidate| {
                [
                    candidate
                        .forward_predecessor
                        .zip(candidate.forward_global_successor),
                    candidate
                        .reverse_predecessor
                        .zip(candidate.reverse_global_successor),
                ]
            })
            .flatten()
            .map(|(predecessor, _successor)| predecessor)
            .collect::<BTreeSet<_>>()
            .len();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_global_successor_count,
            global_successor_count
        );
        let identity_plans = result.partition_border_graph.global_face_identity_plans();
        let identity_stats = result
            .partition_border_graph
            .clone()
            .reconcile_global_face_identity_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_candidate_cycle_count,
            identity_plans.len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_closed_cycle_count,
            identity_plans.iter().filter(|plan| plan.closed).count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_boundary_observation_count,
            identity_plans
                .iter()
                .map(|plan| plan.boundary_observation_ids.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_candidate_cycle_count,
            identity_stats.candidate_cycle_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_closed_cycle_count,
            identity_stats.closed_cycle_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_boundary_observation_count,
            identity_stats.boundary_observation_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_incomplete_component_count,
            identity_stats.incomplete_component_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_non_permutation_component_count,
            identity_stats.non_permutation_component_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_identity_permutation_ready,
            identity_stats.permutation_ready
        );
        let mutation_plans = result
            .partition_border_graph
            .global_face_next_mutation_plans();
        let mutation_stats = result
            .partition_border_graph
            .clone()
            .reconcile_global_face_next_mutation_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_mutation_plan_count,
            mutation_plans.len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_mutation_candidate_link_count,
            mutation_plans
                .iter()
                .map(|plan| plan.successor_observation_ids.len())
                .sum::<usize>()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_mutation_boundary_observation_count,
            mutation_stats.boundary_observation_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_mutation_ready_component_count,
            mutation_stats.ready_component_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_mutation_incomplete_component_count,
            mutation_stats.incomplete_component_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_next_mutation_ready,
            mutation_stats.mutation_ready
        );
        let id_plans = result.partition_border_graph.global_face_id_plans();
        let id_stats = result
            .partition_border_graph
            .clone()
            .reconcile_global_face_id_plans(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_candidate_cycle_count,
            id_plans.len()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_assigned_count,
            id_plans
                .iter()
                .filter(|plan| plan.candidate_global_face_id.is_some())
                .count()
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_boundary_observation_count,
            id_stats.boundary_observation_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_unbounded_candidate_count,
            id_stats.unbounded_candidate_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_incomplete_plan_count,
            id_stats.incomplete_plan_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_face_id_assignment_ready,
            id_stats.assignment_ready
        );
        let unbounded_proof = result
            .partition_border_graph
            .validate_global_unbounded_face_proof(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_proof_candidate_count,
            unbounded_proof.candidate_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_proof_ready,
            unbounded_proof.proof_ready
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_proof_closed_count,
            unbounded_proof.closed_unbounded_face_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_proof_unmapped_twin_count,
            unbounded_proof.unbounded_face_unmapped_twin_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_proof_not_ready_twin_count,
            unbounded_proof.unbounded_face_not_ready_twin_count
        );
        let unbounded_application = result
            .partition_border_graph
            .validate_global_unbounded_face_application(&ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_application_candidate_cycle_count,
            unbounded_application.candidate_cycle_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_application_candidate_unbounded_face_id_count,
            unbounded_application.candidate_unbounded_face_id_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_application_mapped_unbounded_cycle_count,
            unbounded_application.mapped_unbounded_cycle_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_application_missing_unbounded_face_id_count,
            unbounded_application.missing_unbounded_face_id_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_application_duplicate_unbounded_face_id_count,
            unbounded_application.duplicate_unbounded_face_id_count
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_unbounded_face_application_ready,
            unbounded_application.application_ready
        );
        let topology_mutation_gate = result
            .partition_border_graph
            .validate_global_topology_mutation_gate_with_evidence(
                &ExecutionPolicy::default(),
                global_topology_application_gate_stats,
                global_component_coverage_stats,
                global_face_id_application_stats,
                unbounded_application,
                face_walk,
                face_euler,
            )
            .unwrap();
        assert_eq!(
            [
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_edge_count,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_component_count,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_face_count,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_candidate_cycle_count,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_applied_twin_count,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_mapped_twin_count,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_source_complete_twin_count,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_closed_face_count,
            ],
            [
                topology_mutation_gate.edge_count,
                topology_mutation_gate.component_count,
                topology_mutation_gate.face_count,
                topology_mutation_gate.candidate_cycle_count,
                topology_mutation_gate.applied_twin_count,
                topology_mutation_gate.mapped_twin_count,
                topology_mutation_gate.source_complete_twin_count,
                topology_mutation_gate.closed_face_count,
            ]
        );
        assert_eq!(
            [
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_topology_application_ready,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_component_coverage_ready,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_face_id_application_ready,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_unbounded_face_application_ready,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_face_walk_ready,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_euler_evidence_ready,
                result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_ready,
            ],
            [
                topology_mutation_gate.topology_application_ready,
                topology_mutation_gate.component_coverage_ready,
                topology_mutation_gate.face_id_application_ready,
                topology_mutation_gate.unbounded_face_application_ready,
                topology_mutation_gate.face_walk_ready,
                topology_mutation_gate.euler_evidence_ready,
                topology_mutation_gate.gate_ready,
            ]
        );
        assert!(
            !result
                .stitching_report
                .partition_border_global_face_id_mutation_applied
                || result
                    .stitching_report
                    .partition_border_global_face_id_mutation_ready
        );
        if result
            .stitching_report
            .partition_border_global_face_id_mutation_applied
        {
            assert_eq!(
                result
                    .stitching_report
                    .partition_border_global_face_id_mutation_applied_face_id_count,
                result
                    .stitching_report
                    .partition_border_global_face_id_mutation_candidate_cycle_count
            );
            assert_eq!(
                result
                    .stitching_report
                    .partition_border_global_face_id_mutation_unbounded_face_id_count,
                1
            );
        }
        assert!(
            !result
                .stitching_report
                .partition_border_global_unbounded_face_mutation_applied
                || result
                    .stitching_report
                    .partition_border_global_unbounded_face_mutation_ready
        );
        if result
            .stitching_report
            .partition_border_global_unbounded_face_mutation_applied
        {
            assert_eq!(
                result
                    .stitching_report
                    .partition_border_global_unbounded_face_mutation_candidate_unbounded_face_id_count,
                1
            );
            assert_eq!(
                result
                    .stitching_report
                    .partition_border_global_unbounded_face_mutation_applied_unbounded_face_id,
                Some(0)
            );
        }
        assert_eq!(result.polygons.len(), 2);
    }

    #[test]
    fn boundary_noding_exports_only_physical_finite_border_spans() {
        let mut polygonizer = Polygonizer::new();
        polygonizer.add_lines(vec![
            Line3D::new(
                Coord3D::new(-2.0, 0.0, 1.0),
                Coord3D::new(12.0, 0.0, 15.0),
                41,
            ),
            Line3D::new(
                Coord3D::new(12.0, 0.0, 0.0),
                Coord3D::new(12.0, 2.0, 0.0),
                42,
            ),
            Line3D::new(
                Coord3D::new(12.0, 2.0, 0.0),
                Coord3D::new(-2.0, 2.0, 0.0),
                43,
            ),
            Line3D::new(
                Coord3D::new(-2.0, 2.0, 0.0),
                Coord3D::new(-2.0, 0.0, 0.0),
                44,
            ),
        ]);
        let (_, observations, local_face_graphs, stats, _noded_segments, _boundary_noded_segments) =
            polygonizer
                .polygonize_with_partition_border_export_and_stats(
                    7,
                    Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 1.0 }),
                )
                .unwrap();

        assert_eq!(stats.added_node_count, 2);
        assert_eq!(stats.added_edge_count, 2);
        assert_eq!(stats.split_event_count, 2);
        assert_eq!(local_face_graphs.len(), 1);
        assert!(local_face_graphs[0]
            .directed_edges
            .iter()
            .all(|edge| edge.face_ref.is_some()));

        let border = observations
            .iter()
            .filter(|observation| {
                observation.side == crate::graph::partition_border::PartitionBorderSide::MinY
            })
            .collect::<Vec<_>>();
        assert_eq!(border.len(), 2);
        assert!(border.iter().all(|observation| {
            let (start, end) = observation.edge_key.endpoints();
            let x_bits = [start.xy_bits()[0], end.xy_bits()[0]];
            x_bits == [0, 10.0f64.to_bits()] || x_bits == [10.0f64.to_bits(), 0]
        }));
        assert!(border
            .iter()
            .all(|observation| observation.source_line_ids == vec![41]));
        assert!(border.iter().all(|observation| {
            let z_bits = [observation.from_z_bits, observation.to_z_bits];
            observation.representative_line_id == Some(41)
                && (z_bits == [3.0f64.to_bits(), 13.0f64.to_bits()]
                    || z_bits == [13.0f64.to_bits(), 3.0f64.to_bits()])
        }));
    }

    #[test]
    fn trace_records_partition_boundary_noding_and_atomic_observations() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 1.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 2.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 0.0 },
                Coord { x: 2.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 1.0 },
                Coord { x: 0.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 1.0 },
                Coord { x: 0.0, y: 0.0 },
            ])),
        ];
        let mut tiled = TiledPolygonizer::new(bbox, 1.0).with_buffer(0.0);
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let noding_events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "partition_boundary_noding")
            .collect::<Vec<_>>();
        assert_eq!(noding_events.len(), 2);
        assert!(noding_events.iter().all(|event| {
            event.payload["added_node_count"].as_u64().unwrap() >= 1
                && event.payload["added_edge_count"].as_u64().unwrap() >= 1
                && event.payload["split_event_count"].as_u64().unwrap() >= 1
        }));

        let observations = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "partition_border_atomic_observation")
            .collect::<Vec<_>>();
        assert!(!observations.is_empty());
        assert!(observations.iter().all(|event| {
            event.payload["edge_key"]
                .as_array()
                .is_some_and(|endpoints| endpoints.len() == 2)
                && event.payload["from_z_bits"].as_str().is_some()
                && event.payload["to_z_bits"].as_str().is_some()
                && event.payload["source_count"].as_u64().is_some()
                && event.payload["representative_line_id"].as_u64().is_some()
                && event.payload["component_id"].as_u64().is_some()
        }));
        assert!(observations
            .iter()
            .filter(|event| event.payload["face_id"].is_number())
            .all(|event| event.payload["local_face_boundary_successor"].is_object()));
        let reconciliation = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_twin_reconciliation")
            .expect("twin reconciliation evidence");
        assert_eq!(reconciliation.payload["declared_adjacency_count"], 1);
        assert_eq!(
            reconciliation.payload["matched_twin_count"]
                .as_u64()
                .unwrap()
                + reconciliation.payload["unmatched_edge_count"]
                    .as_u64()
                    .unwrap(),
            reconciliation.payload["normalized_edge_count"]
                .as_u64()
                .unwrap()
        );
        let application = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_twin_application")
            .expect("twin application evidence");
        assert_eq!(
            application.payload["candidate_twin_count"],
            reconciliation.payload["matched_twin_count"]
        );
        assert_eq!(
            application.payload["applied_twin_count"].as_u64().unwrap()
                + application.payload["missing_face_ref_count"]
                    .as_u64()
                    .unwrap()
                + application.payload["invalid_face_ref_count"]
                    .as_u64()
                    .unwrap(),
            application.payload["candidate_twin_count"]
                .as_u64()
                .unwrap()
        );
        let edge_map = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_edge_map")
            .expect("global face edge map evidence");
        assert_eq!(
            edge_map.payload["local_graph_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_edge_map_local_graph_count as u64
            )
        );
        assert_eq!(
            edge_map.payload["directed_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_edge_map_directed_edge_count
                    as u64
            )
        );
        assert_eq!(
            edge_map.payload["mapped_observation_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_edge_map_observation_count as u64
            )
        );
        assert_eq!(
            edge_map.payload["mapped_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_edge_map_twin_count as u64
            )
        );
        assert_eq!(
            edge_map.payload["unmapped_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_edge_map_unmapped_twin_count
                    as u64
            )
        );
        assert_eq!(
            edge_map.payload["edge_map_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_edge_map_ready
            )
        );
        let global_face_nodes = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_nodes")
            .expect("global face node evidence");
        assert_eq!(
            global_face_nodes.payload["edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_node_edge_count as u64
            )
        );
        assert_eq!(
            global_face_nodes.payload["node_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_node_count as u64
            )
        );
        assert_eq!(
            global_face_nodes.payload["endpoint_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_node_endpoint_count as u64
            )
        );
        assert_eq!(
            global_face_nodes.payload["mapped_observation_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_node_observation_count as u64
            )
        );
        assert_eq!(
            global_face_nodes.payload["unmapped_observation_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_node_unmapped_observation_count
                    as u64
            )
        );
        assert_eq!(
            global_face_nodes.payload["node_map_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_node_ready
            )
        );
        let next_application = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_next_application")
            .expect("global face next application evidence");
        assert_eq!(
            next_application.payload["plan_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_application_plan_count
                    as u64
            )
        );
        assert_eq!(
            next_application.payload["candidate_link_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_application_link_count
                    as u64
            )
        );
        assert_eq!(
            next_application.payload["mapped_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_application_edge_count
                    as u64
            )
        );
        assert_eq!(
            next_application.payload["mapped_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_application_twin_count
                    as u64
            )
        );
        assert_eq!(
            next_application.payload["unmapped_observation_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_application_unmapped_observation_count
                    as u64
            )
        );
        assert_eq!(
            next_application.payload["incomplete_plan_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_application_incomplete_plan_count
                    as u64
            )
        );
        assert_eq!(
            next_application.payload["node_discontinuity_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_application_node_discontinuity_count
                    as u64
            )
        );
        assert_eq!(
            next_application.payload["application_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_application_ready
            )
        );
        let topology_candidate = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_topology_candidate")
            .expect("global topology candidate evidence");
        assert_eq!(
            topology_candidate.payload["edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_edge_count as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["local_successor_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_local_successor_count
                    as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["global_override_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_global_override_count
                    as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["assigned_next_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_assigned_next_count
                    as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["unassigned_next_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_unassigned_next_count
                    as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_cycle_count as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["closed_cycle_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_closed_cycle_edge_count
                    as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["predecessor_conflict_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_predecessor_conflict_count
                    as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["node_discontinuity_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_node_discontinuity_count
                    as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["incomplete_application_plan_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_incomplete_application_plan_count
                    as u64
            )
        );
        assert_eq!(
            topology_candidate.payload["candidate_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_candidate_ready
            )
        );
        let topology_application_gate = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_topology_application_gate")
            .expect("global topology application gate evidence");
        assert_eq!(
            topology_application_gate.payload["edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_edge_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["candidate_successor_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_successor_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["declared_adjacency_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_adjacency_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["applied_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_applied_twin_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["mapped_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_mapped_twin_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["unmapped_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_unmapped_twin_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["invalid_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_invalid_twin_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["predecessor_conflict_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_predecessor_conflict_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["node_discontinuity_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_node_discontinuity_count
                    as u64
            )
        );
        assert_eq!(
            topology_application_gate.payload["application_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_application_gate_ready
            )
        );
        let component_coverage = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_component_coverage")
            .expect("global component coverage evidence");
        assert_eq!(
            component_coverage.payload["component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_component_count
                    as u64
            )
        );
        assert_eq!(
            component_coverage.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_face_count as u64
            )
        );
        assert_eq!(
            component_coverage.payload["edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_edge_count as u64
            )
        );
        assert_eq!(
            component_coverage.payload["face_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_face_edge_count
                    as u64
            )
        );
        assert_eq!(
            component_coverage.payload["covered_face_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_covered_face_edge_count
                    as u64
            )
        );
        assert_eq!(
            component_coverage.payload["uncovered_face_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_uncovered_face_edge_count
                    as u64
            )
        );
        assert_eq!(
            component_coverage.payload["duplicate_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_duplicate_face_count
                    as u64
            )
        );
        assert_eq!(
            component_coverage.payload["duplicate_twin_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_duplicate_twin_edge_count
                    as u64
            )
        );
        assert_eq!(
            component_coverage.payload["coverage_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_coverage_ready
            )
        );
        let face_id_application = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_id_application")
            .expect("global face ID application evidence");
        assert_eq!(
            face_id_application.payload["candidate_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_application_candidate_cycle_count
                    as u64
            )
        );
        assert_eq!(
            face_id_application.payload["assigned_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_application_assigned_face_count
                    as u64
            )
        );
        assert_eq!(
            face_id_application.payload["candidate_cycle_start_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_application_cycle_start_count
                    as u64
            )
        );
        assert_eq!(
            face_id_application.payload["mapped_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_application_mapped_cycle_count
                    as u64
            )
        );
        assert_eq!(
            face_id_application.payload["unmapped_plan_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_application_unmapped_plan_count
                    as u64
            )
        );
        assert_eq!(
            face_id_application.payload["duplicate_face_id_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_application_duplicate_face_id_count
                    as u64
            )
        );
        assert_eq!(
            face_id_application.payload["non_contiguous_face_id_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_application_non_contiguous_face_id_count
                    as u64
            )
        );
        assert_eq!(
            face_id_application.payload["application_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_application_ready
            )
        );
        let node_reconciliation = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_node_reconciliation")
            .expect("border node reconciliation evidence");
        assert_eq!(
            node_reconciliation.payload["node_count"].as_u64(),
            Some(
                traced
                    .result
                    .partition_border_graph
                    .reconciled_border_nodes()
                    .len() as u64
            )
        );
        assert_eq!(
            node_reconciliation.payload["z_conflict_count"].as_u64(),
            Some(0)
        );
        assert_eq!(
            node_reconciliation.payload["z_policy"],
            "InterpolateAlongEdge"
        );
        assert_eq!(
            node_reconciliation.payload["conflict_tolerance"],
            "0x0000000000000000"
        );
        let global_components = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_component_reconciliation")
            .expect("global component reconciliation evidence");
        assert_eq!(
            global_components.payload["component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_count as u64
            )
        );
        assert_eq!(
            global_components.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_count as u64
            )
        );
        assert_eq!(
            global_components.payload["linked_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_linked_face_count as u64
            )
        );
        let global_component_payloads = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_component_payloads")
            .expect("global component payload evidence");
        assert_eq!(
            global_component_payloads.payload["component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_payload_count as u64
            )
        );
        assert_eq!(
            global_component_payloads.payload["source_line_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_payload_source_line_count
                    as u64
            )
        );
        assert_eq!(
            global_component_payloads.payload["representative_line_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_payload_representative_line_count
                    as u64
            )
        );
        assert_eq!(
            global_component_payloads.payload["z_candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_payload_z_candidate_count
                    as u64
            )
        );
        assert_eq!(
            global_component_payloads.payload["selected_z_node_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_payload_selected_z_node_count
                    as u64
            )
        );
        assert_eq!(
            global_component_payloads.payload["z_conflict_node_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_payload_z_conflict_node_count
                    as u64
            )
        );
        assert_eq!(
            global_component_payloads.payload["z_conflict_component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_component_payload_z_conflict_component_count
                    as u64
            )
        );
        let global_face_plan = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_plan")
            .expect("global face plan evidence");
        assert_eq!(
            global_face_plan.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_plan_count as u64
            )
        );
        assert_eq!(
            global_face_plan.payload["candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_candidate_count as u64
            )
        );
        assert_eq!(
            global_face_plan.payload["missing_successor_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_missing_successor_count as u64
            )
        );
        assert_eq!(
            global_face_plan.payload["unbounded_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_count as u64
            )
        );
        assert_eq!(
            global_face_plan.payload["linked_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_linked_count as u64
            )
        );
        assert_eq!(
            global_face_plan.payload["missing_boundary_successor_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_missing_boundary_successor_count
                    as u64
            )
        );
        let global_face_validation = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_validation")
            .expect("global face validation evidence");
        assert_eq!(
            global_face_validation.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_validated_count as u64
            )
        );
        assert_eq!(
            global_face_validation.payload["candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_validated_candidate_count as u64
            )
        );
        assert_eq!(
            global_face_validation.payload["twin_link_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_validated_twin_count as u64
            )
        );
        assert_eq!(
            global_face_validation.payload["unbounded_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_validated_unbounded_count as u64
            )
        );
        let global_face_gate = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_mutation_gate")
            .expect("global face mutation gate evidence");
        assert_eq!(
            global_face_gate.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_validated_count as u64
            )
        );
        assert_eq!(
            global_face_gate.payload["candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_validated_candidate_count as u64
            )
        );
        assert_eq!(
            global_face_gate.payload["boundary_transition_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_boundary_transition_count as u64
            )
        );
        assert_eq!(
            global_face_gate.payload["missing_boundary_successor_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_mutation_missing_successor_count
                    as u64
            )
        );
        assert_eq!(
            global_face_gate.payload["mutation_ready_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_mutation_ready_count as u64
            )
        );
        let global_face_transition_plan = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_transition_plan")
            .expect("global face transition plan evidence");
        assert_eq!(
            global_face_transition_plan.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_plan_count as u64
            )
        );
        assert_eq!(
            global_face_transition_plan.payload["candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_candidate_count as u64
            )
        );
        assert_eq!(
            global_face_transition_plan.payload["boundary_transition_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_transition_count as u64
            )
        );
        assert_eq!(
            global_face_transition_plan.payload["closed_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_transition_closed_count as u64
            )
        );
        assert_eq!(
            global_face_transition_plan.payload["incomplete_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_transition_incomplete_count
                    as u64
            )
        );
        let global_face_twin_transition = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_twin_transitions")
            .expect("global face twin transition evidence");
        assert_eq!(
            global_face_twin_transition.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_plan_count as u64
            )
        );
        assert_eq!(
            global_face_twin_transition.payload["transition_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_transition_count as u64
            )
        );
        assert_eq!(
            global_face_twin_transition.payload["applied_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_face_twin_count as u64
            )
        );
        assert_eq!(
            global_face_twin_transition.payload["mapped_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_twin_transition_count as u64
            )
        );
        assert_eq!(
            global_face_twin_transition.payload["unmapped_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_twin_transition_unmapped_count
                    as u64
            )
        );
        assert_eq!(
            global_face_twin_transition.payload["mutation_ready_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_twin_transition_ready_count
                    as u64
            )
        );
        let global_face_walk = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_walk_invariants")
            .expect("global face walk invariant evidence");
        assert_eq!(
            global_face_walk.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_walk_validated_count as u64
            )
        );
        assert_eq!(
            global_face_walk.payload["closed_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_walk_closed_count as u64
            )
        );
        assert_eq!(
            global_face_walk.payload["source_complete_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_walk_source_complete_twin_count
                    as u64
            )
        );
        assert_eq!(
            global_face_walk.payload["unbounded_component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_walk_unbounded_component_count
                    as u64
            )
        );
        assert_eq!(
            global_face_walk.payload["face_adjacency_cycle_rank"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_walk_face_adjacency_cycle_rank
                    as u64
            )
        );
        let global_face_euler = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_euler_witness")
            .expect("global face Euler witness evidence");
        assert_eq!(
            global_face_euler.payload["transition_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_euler_transition_face_count
                    as u64
            )
        );
        assert_eq!(
            global_face_euler.payload["closed_boundary_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_euler_closed_boundary_cycle_count
                    as u64
            )
        );
        assert_eq!(
            global_face_euler.payload["boundary_vertex_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_euler_boundary_vertex_count
                    as u64
            )
        );
        assert_eq!(
            global_face_euler.payload["boundary_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_euler_boundary_edge_count as u64
            )
        );
        assert_eq!(
            global_face_euler.payload["boundary_euler_lhs"].as_i64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_euler_boundary_lhs
            )
        );
        assert_eq!(
            global_face_euler.payload["boundary_euler_rhs"].as_i64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_euler_boundary_rhs
            )
        );
        assert_eq!(
            global_face_euler.payload["boundary_euler_consistent"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_euler_boundary_consistent
            )
        );
        let global_face_next_candidates = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_next_candidates")
            .expect("global face next candidate evidence");
        assert_eq!(
            global_face_next_candidates.payload["twin_candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_candidate_count as u64
            )
        );
        assert_eq!(
            global_face_next_candidates.payload["ready_candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_ready_candidate_count as u64
            )
        );
        assert_eq!(
            global_face_next_candidates.payload["incomplete_candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_incomplete_candidate_count
                    as u64
            )
        );
        assert_eq!(
            global_face_next_candidates.payload["global_successor_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_global_successor_count
                    as u64
            )
        );
        let global_face_identity_plans = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_identity_plans")
            .expect("global face identity plan evidence");
        assert_eq!(
            global_face_identity_plans.payload["candidate_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_identity_candidate_cycle_count
                    as u64
            )
        );
        assert_eq!(
            global_face_identity_plans.payload["boundary_observation_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_identity_boundary_observation_count
                    as u64
            )
        );
        assert_eq!(
            global_face_identity_plans.payload["closed_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_identity_closed_cycle_count
                    as u64
            )
        );
        assert_eq!(
            global_face_identity_plans.payload["incomplete_component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_identity_incomplete_component_count
                    as u64
            )
        );
        assert_eq!(
            global_face_identity_plans.payload["non_permutation_component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_identity_non_permutation_component_count
                    as u64
            )
        );
        assert_eq!(
            global_face_identity_plans.payload["permutation_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_identity_permutation_ready
            )
        );
        let global_face_next_mutation_plans = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_next_mutation_plans")
            .expect("global face next mutation plan evidence");
        assert_eq!(
            global_face_next_mutation_plans.payload["plan_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_mutation_plan_count as u64
            )
        );
        assert_eq!(
            global_face_next_mutation_plans.payload["candidate_link_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_mutation_candidate_link_count
                    as u64
            )
        );
        assert_eq!(
            global_face_next_mutation_plans.payload["ready_component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_mutation_ready_component_count
                    as u64
            )
        );
        assert_eq!(
            global_face_next_mutation_plans.payload["incomplete_component_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_mutation_incomplete_component_count
                    as u64
            )
        );
        assert_eq!(
            global_face_next_mutation_plans.payload["mutation_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_next_mutation_ready
            )
        );
        let global_face_id_plans = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_id_plans")
            .expect("global face ID plan evidence");
        assert_eq!(
            global_face_id_plans.payload["candidate_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_candidate_cycle_count as u64
            )
        );
        assert_eq!(
            global_face_id_plans.payload["assigned_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_assigned_count as u64
            )
        );
        assert_eq!(
            global_face_id_plans.payload["boundary_observation_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_boundary_observation_count
                    as u64
            )
        );
        assert_eq!(
            global_face_id_plans.payload["unbounded_candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_unbounded_candidate_count
                    as u64
            )
        );
        assert_eq!(
            global_face_id_plans.payload["incomplete_plan_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_incomplete_plan_count as u64
            )
        );
        assert_eq!(
            global_face_id_plans.payload["assignment_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_assignment_ready
            )
        );
        let unbounded_proof = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_unbounded_face_proof")
            .expect("global unbounded face proof evidence");
        assert_eq!(
            unbounded_proof.payload["candidate_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_proof_candidate_count
                    as u64
            )
        );
        assert_eq!(
            global_face_euler.payload["cross_component_edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_euler_cross_component_edge_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_proof.payload["proof_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_proof_ready
            )
        );
        assert_eq!(
            unbounded_proof.payload["closed_unbounded_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_proof_closed_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_proof.payload["unbounded_face_unmapped_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_proof_unmapped_twin_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_proof.payload["unbounded_face_not_ready_twin_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_proof_not_ready_twin_count
                    as u64
            )
        );
        let unbounded_application = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_unbounded_face_application")
            .expect("global unbounded face application evidence");
        assert_eq!(
            unbounded_application.payload["candidate_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_application_candidate_cycle_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_application.payload["candidate_unbounded_face_id_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_application_candidate_unbounded_face_id_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_application.payload["mapped_unbounded_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_application_mapped_unbounded_cycle_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_application.payload["missing_unbounded_face_id_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_application_missing_unbounded_face_id_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_application.payload["duplicate_unbounded_face_id_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_application_duplicate_unbounded_face_id_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_application.payload["application_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_application_ready
            )
        );
        let topology_mutation_gate = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_topology_mutation_gate")
            .expect("global topology mutation gate evidence");
        assert_eq!(
            topology_mutation_gate.payload["edge_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_edge_count
                    as u64
            )
        );
        assert_eq!(
            topology_mutation_gate.payload["face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_face_count
                    as u64
            )
        );
        assert_eq!(
            topology_mutation_gate.payload["closed_face_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_closed_face_count
                    as u64
            )
        );
        assert_eq!(
            topology_mutation_gate.payload["topology_application_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_topology_application_ready
            )
        );
        assert_eq!(
            topology_mutation_gate.payload["face_walk_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_face_walk_ready
            )
        );
        assert_eq!(
            topology_mutation_gate.payload["euler_evidence_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_euler_evidence_ready
            )
        );
        assert_eq!(
            topology_mutation_gate.payload["gate_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_topology_mutation_gate_ready
            )
        );
        let face_id_mutation = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_face_id_mutation")
            .expect("global face ID mutation evidence");
        assert_eq!(
            face_id_mutation.payload["candidate_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_mutation_candidate_cycle_count
                    as u64
            )
        );
        assert_eq!(
            face_id_mutation.payload["applied_face_id_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_mutation_applied_face_id_count
                    as u64
            )
        );
        assert_eq!(
            face_id_mutation.payload["unbounded_face_id_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_mutation_unbounded_face_id_count
                    as u64
            )
        );
        assert_eq!(
            face_id_mutation.payload["mutation_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_mutation_ready
            )
        );
        assert_eq!(
            face_id_mutation.payload["applied"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_face_id_mutation_applied
            )
        );
        let unbounded_face_mutation = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_unbounded_face_mutation")
            .expect("global unbounded face mutation evidence");
        assert_eq!(
            unbounded_face_mutation.payload["candidate_cycle_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_mutation_candidate_cycle_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_face_mutation.payload["candidate_unbounded_face_id_count"].as_u64(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_mutation_candidate_unbounded_face_id_count
                    as u64
            )
        );
        assert_eq!(
            unbounded_face_mutation.payload["mutation_ready"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_mutation_ready
            )
        );
        assert_eq!(
            unbounded_face_mutation.payload["applied"].as_bool(),
            Some(
                traced
                    .result
                    .stitching_report
                    .partition_border_global_unbounded_face_mutation_applied
            )
        );
    }

    #[test]
    fn fixed_and_certified_boundary_export_survives_input_permutation() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 1.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -1.0, y: 0.0 },
                Coord { x: 3.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 3.0, y: 0.0 },
                Coord { x: 3.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 3.0, y: 1.0 },
                Coord { x: -1.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -1.0, y: 1.0 },
                Coord { x: -1.0, y: 0.0 },
            ])),
        ];
        let options = [
            PolygonizerOptions {
                node_input: true,
                precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
                ..Default::default()
            },
            PolygonizerOptions {
                node_input: true,
                precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
                noding: NodingOptions {
                    guarantee: NodingGuarantee::CertifiedFixedPrecision,
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        let atomic_signature = |trace: &crate::trace::TopologyTraceV1| {
            let mut signature = trace
                .events
                .iter()
                .filter(|event| event.kind == "partition_border_atomic_observation")
                .map(|event| {
                    serde_json::to_string(&serde_json::json!({
                        "partition_id": event.payload["partition_id"],
                        "edge_key": event.payload["edge_key"],
                        "from": event.payload["from"],
                        "to": event.payload["to"],
                        "from_z_bits": event.payload["from_z_bits"],
                        "to_z_bits": event.payload["to_z_bits"],
                        "side": event.payload["side"],
                        "source_count": event.payload["source_count"],
                    }))
                    .unwrap()
                })
                .collect::<Vec<_>>();
            signature.sort_unstable();
            signature
        };
        let noding_signature = |trace: &crate::trace::TopologyTraceV1| {
            let mut signature = trace
                .events
                .iter()
                .filter(|event| event.kind == "partition_boundary_noding")
                .map(|event| {
                    (
                        event.payload["partition_id"].as_u64().unwrap(),
                        event.payload["added_node_count"].as_u64().unwrap(),
                        event.payload["added_edge_count"].as_u64().unwrap(),
                        event.payload["split_event_count"].as_u64().unwrap(),
                    )
                })
                .collect::<Vec<_>>();
            signature.sort_unstable();
            signature
        };

        let mut precision_signatures = Vec::new();
        for option in options {
            let mut forward = TiledPolygonizer::new(bbox, 1.0)
                .with_buffer(0.0)
                .with_options(option.clone());
            for geometry in &geometries {
                forward.add_geometry(geometry);
            }
            let forward = forward
                .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                .unwrap();
            let forward_atomic = atomic_signature(&forward.trace);
            let forward_noding = noding_signature(&forward.trace);
            assert_eq!(forward_noding.len(), 2);
            assert!(forward_noding
                .iter()
                .all(|(_, nodes, edges, splits)| { *nodes > 0 && *edges > 0 && *splits > 0 }));
            assert!(!forward_atomic.is_empty());

            let mut reversed = TiledPolygonizer::new(bbox, 1.0)
                .with_buffer(0.0)
                .with_options(option);
            for geometry in geometries.iter().rev() {
                reversed.add_geometry(geometry);
            }
            let reversed = reversed
                .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                .unwrap();
            assert_eq!(forward_atomic, atomic_signature(&reversed.trace));
            assert_eq!(forward_noding, noding_signature(&reversed.trace));
            assert_eq!(
                forward.result.partition_border_graph.edge_count(),
                reversed.result.partition_border_graph.edge_count()
            );
            assert_eq!(
                forward.result.partition_border_graph.node_count(),
                reversed.result.partition_border_graph.node_count()
            );

            precision_signatures.push(forward_atomic);
        }
        assert_eq!(precision_signatures[0], precision_signatures[1]);
    }

    #[test]
    fn test_tiled_polygonization_grid() {
        // Create a 2x2 grid of squares
        // 0,0 - 10,0 - 20,0
        //  |     |      |
        // 0,10- 10,10- 20,10
        //  |     |      |
        // 0,20- 10,20- 20,20

        let geoms = vec![
            // Horizontals
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 20.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 10.0 },
                Coord { x: 20.0, y: 10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 20.0 },
                Coord { x: 20.0, y: 20.0 },
            ])),
            // Verticals
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 0.0, y: 20.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 10.0, y: 0.0 },
                Coord { x: 10.0, y: 20.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 20.0, y: 0.0 },
                Coord { x: 20.0, y: 20.0 },
            ])),
        ];

        // BBox covers 0,0 to 20,20
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        // Tile size 10 (exactly matching lines) or 15 (offset)
        // Let's try 15 to ensure polygons span tiles
        // Add buffer of 5.0 to ensure full polygons are captured in each tile
        let mut tiler = TiledPolygonizer::new(bbox, 15.0).with_buffer(5.0);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let traced = tiler
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let result = traced.result;
        let stitched_event = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "tiled_stitched_output")
            .unwrap();
        assert_eq!(result.polygons.len(), 4);
        assert_eq!(
            result.stitched_output.is_some(),
            result
                .stitching_report
                .partition_border_global_stitched_output_ready
        );
        // The internal grid borders produce ambiguous four-observation
        // buckets. Keep the tiled result usable, but fail closed until a
        // face-qualified pairing contract exists for those spans.
        assert!(result.stitched_output.is_none());
        assert_eq!(result.stitching_report.partition_border_twin_count, 0);
        assert!(
            !result
                .stitching_report
                .partition_border_global_extraction_readiness_ready
        );
        assert_eq!(
            stitched_event.payload["ready"],
            result.stitched_output.is_some()
        );
        if let Some(stitched_output) = result.stitched_output.as_ref() {
            assert_eq!(stitched_output.polygons.len(), 4);
            assert!(stitched_output
                .polygons
                .iter()
                .all(|polygon| polygon.exterior.iter().all(|coord| coord.z == 0.0)));
        }
        let polys = result.polygons;

        // Should find 4 polygons
        assert_eq!(polys.len(), 4);

        // Check areas
        for p in polys {
            assert!((p.unsigned_area_2d() - 100.0).abs() < 1e-6);
        }
    }

    #[test]
    fn opt_in_untiled_equivalence_is_gated_by_ready_stitched_output() {
        let geometries = vec![
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 2.0 },
                Coord { x: 18.0, y: 2.0 },
                Coord { x: 18.0, y: 18.0 },
                Coord { x: 2.0, y: 18.0 },
                Coord { x: 2.0, y: 2.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 6.0, y: 6.0 },
                Coord { x: 14.0, y: 6.0 },
                Coord { x: 14.0, y: 14.0 },
                Coord { x: 6.0, y: 14.0 },
                Coord { x: 6.0, y: 6.0 },
            ])),
        ];
        let mut tiler = TiledPolygonizer::new(
            Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 }),
            10.0,
        )
        .with_buffer(40.0)
        .with_options(PolygonizerOptions {
            node_input: true,
            ..Default::default()
        })
        .with_untiled_equivalence_check();
        for geometry in &geometries {
            tiler.add_geometry(geometry);
        }

        let traced = tiler
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let result = traced.result;
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_stitched_output_ready,
            result.stitched_output.is_some()
        );
        if result.stitched_output.is_none() {
            if !result
                .stitching_report
                .partition_border_global_untiled_equivalence_checked
            {
                assert!(
                    !result
                        .stitching_report
                        .partition_border_global_untiled_equivalence_ready
                );
                assert_eq!(
                    result
                        .stitching_report
                        .partition_border_global_untiled_equivalence_mismatch_count,
                    0
                );
            }
        } else {
            assert!(
                result
                    .stitching_report
                    .partition_border_global_untiled_equivalence_checked
            );
            assert!(
                result
                    .stitching_report
                    .partition_border_global_untiled_equivalence_ready
            );
            assert_eq!(
                result
                    .stitching_report
                    .partition_border_global_untiled_equivalence_mismatch_count,
                0
            );
        }
        let event = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "tiled_untiled_equivalence")
            .unwrap();
        assert_eq!(
            event.payload["checked"],
            result
                .stitching_report
                .partition_border_global_untiled_equivalence_checked
        );
        assert_eq!(
            event.payload["ready"],
            result
                .stitching_report
                .partition_border_global_untiled_equivalence_ready
        );
        assert_eq!(
            event.payload["mismatch_count"],
            result
                .stitching_report
                .partition_border_global_untiled_equivalence_mismatch_count
        );
    }

    #[test]
    fn untiled_equivalence_compares_canonical_output_and_detects_mismatch() {
        let geometries = vec![
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 10.0, y: 0.0 },
                Coord { x: 10.0, y: 10.0 },
                Coord { x: 0.0, y: 10.0 },
                Coord { x: 0.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 2.0 },
                Coord { x: 8.0, y: 2.0 },
                Coord { x: 8.0, y: 8.0 },
                Coord { x: 2.0, y: 8.0 },
                Coord { x: 2.0, y: 2.0 },
            ])),
        ];
        let geometry_refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let options = PolygonizerOptions {
            node_input: true,
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            input_profile_id: Some("equivalence-test".to_string()),
            ..Default::default()
        };
        let mut polygonizer = Polygonizer::with_options(options.clone());
        for geometry in &geometries {
            polygonizer.add_borrowed_geometry(geometry);
        }
        let expected = polygonizer.polygonize().unwrap();
        let stitched_output = TiledStitchedOutput {
            polygons: expected.polygons.clone(),
            dangles: expected.dangles.clone(),
            cut_edges: expected.cut_edges.clone(),
            invalid_rings: expected.invalid_rings.clone(),
        };
        let stats = crate::tiling::compare_stitched_output_with_untiled(
            Some(&stitched_output),
            &geometry_refs,
            &options,
            &ExecutionPolicy::default(),
        )
        .unwrap();
        assert_eq!(
            stats,
            crate::tiling::TiledUntiledEquivalenceStats {
                checked: true,
                ready: true,
                mismatch_count: 0,
            }
        );

        let mut mismatch = stitched_output;
        mismatch.polygons[0].exterior[0].x += 1.0;
        let stats = crate::tiling::compare_stitched_output_with_untiled(
            Some(&mismatch),
            &geometry_refs,
            &options,
            &ExecutionPolicy::default(),
        )
        .unwrap();
        assert!(stats.checked);
        assert!(!stats.ready);
        assert_eq!(stats.mismatch_count, 1);

        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let execution_policy = ExecutionPolicy {
            cancellation_token: Some(cancellation),
            ..Default::default()
        };
        assert!(matches!(
            crate::tiling::compare_stitched_output_with_untiled(
                Some(&mismatch),
                &geometry_refs,
                &options,
                &execution_policy,
            ),
            Err(PolygonizeError::Cancelled { stage })
                if stage == "tiled_untiled_equivalence"
        ));
    }

    #[test]
    fn test_tiled_polygonization_exact_boundary() {
        // Tile size 10, lines on 10.
        // This tests the "ownership" logic at boundaries.

        let geoms = vec![
            // Horizontals
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 20.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 10.0 },
                Coord { x: 20.0, y: 10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 20.0 },
                Coord { x: 20.0, y: 20.0 },
            ])),
            // Verticals
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 0.0, y: 20.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 10.0, y: 0.0 },
                Coord { x: 10.0, y: 20.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 20.0, y: 0.0 },
                Coord { x: 20.0, y: 20.0 },
            ])),
        ];

        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        // Tile size 10.
        // Tiles: [0,10]x[0,10], [10,20]x[0,10], etc.
        let mut tiler = TiledPolygonizer::new(bbox, 10.0);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize().unwrap().polygons;

        assert_eq!(polys.len(), 4);
    }

    #[test]
    fn test_tiled_polygonization_centroid_on_max_boundary() {
        // A square centered at (20, 5).
        // 19,0 -> 21,0 -> 21,10 -> 19,10 -> 19,0.
        // Centroid is x=20, y=5.
        // BBox passed is 0,0 -> 20,20.
        // This simulates a polygon on the edge of the world.

        let geoms = vec![Geometry::LineString(LineString::new(vec![
            Coord { x: 19.0, y: 0.0 },
            Coord { x: 21.0, y: 0.0 },
            Coord { x: 21.0, y: 10.0 },
            Coord { x: 19.0, y: 10.0 },
            Coord { x: 19.0, y: 0.0 },
        ]))];

        // BBox 0,0 -> 20,20.
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        // Tile size 10.
        // Tiles: [0,10) and [10,20).
        let mut tiler = TiledPolygonizer::new(bbox, 10.0).with_buffer(5.0);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize().unwrap().polygons;
        assert_eq!(
            polys.len(),
            1,
            "Should identify polygon with centroid on the boundary"
        );
    }

    #[test]
    fn test_lexicographic_min_vertex_ownership() {
        use crate::options::TileOwnershipPolicy;

        // A single square crossing the x=10 boundary.
        // Bbox: [8, 0] to [12, 4].
        // Centroid is x=10, y=2.
        // Lexicographic Min Vertex is x=8, y=0.
        let geoms = vec![Geometry::LineString(LineString::new(vec![
            Coord { x: 8.0, y: 0.0 },
            Coord { x: 12.0, y: 0.0 },
            Coord { x: 12.0, y: 4.0 },
            Coord { x: 8.0, y: 4.0 },
            Coord { x: 8.0, y: 0.0 },
        ]))];

        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        let mut tiler = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(5.0)
            .with_ownership_policy(TileOwnershipPolicy::LexicographicMinVertex);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize().unwrap().polygons;
        assert_eq!(
            polys.len(),
            1,
            "Should identify polygon based on LexicographicMinVertex"
        );
    }

    #[test]
    fn representative_ownership_uses_an_interior_point() {
        use crate::options::TileOwnershipPolicy;
        use crate::Polygon3D;

        let polygon = Polygon3D::new(
            vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(4.0, 0.0, 0.0),
                Coord3D::new(4.0, 4.0, 0.0),
                Coord3D::new(3.0, 4.0, 0.0),
                Coord3D::new(3.0, 1.0, 0.0),
                Coord3D::new(1.0, 1.0, 0.0),
                Coord3D::new(1.0, 4.0, 0.0),
                Coord3D::new(0.0, 4.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
            ],
            vec![],
            vec![],
            vec![],
        );
        let polygon_2d = polygon.to_polygon_2d();
        assert!(!polygon_2d.contains(&polygon.centroid_2d().unwrap()));

        let tiler = TiledPolygonizer::new(
            Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 4.0 }),
            2.0,
        )
        .with_ownership_policy(TileOwnershipPolicy::RepresentativePointInsidePolygon);
        assert!(polygon_2d.contains(&tiler.ownership_point(&polygon).unwrap()));
    }

    #[test]
    fn rejects_invalid_tiling_configuration_and_options() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 1.0 });
        assert!(matches!(
            TiledPolygonizer::new(bbox, 0.0).polygonize(),
            Err(PolygonizeError::InvalidArgumentType { field, .. }) if field == "tile_size"
        ));
        assert!(matches!(
            TiledPolygonizer::new(bbox, 1.0)
                .with_buffer(f64::NAN)
                .polygonize(),
            Err(PolygonizeError::InvalidArgumentType { field, .. }) if field == "buffer"
        ));
        assert!(matches!(
            TiledPolygonizer::new(bbox, 1.0)
                .with_retry_policy(TileRetryPolicy {
                    max_attempts: 0,
                    buffer_increment: 1.0,
                    max_buffer: 2.0,
                })
                .polygonize(),
            Err(PolygonizeError::InvalidArgumentType { field, .. })
                if field == "retry_policy.max_attempts"
        ));
        assert!(matches!(
            TiledPolygonizer::new(
                Rect::new(Coord { x: 1.0, y: 1.0 }, Coord { x: 1.0, y: 1.0 }),
                1.0,
            )
            .polygonize(),
            Err(PolygonizeError::InvalidGeometry { .. })
        ));

        let options = PolygonizerOptions {
            pre_snap_tolerance: 1.0,
            ..Default::default()
        };
        assert!(matches!(
            TiledPolygonizer::new(bbox, 1.0)
                .with_options(options)
                .polygonize(),
            Err(PolygonizeError::UnsupportedOptionCombination { .. })
        ));

        let square = Geometry::LineString(LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ]));
        let mut options = PolygonizerOptions::default();
        options.output_filter.minimum_face_area = Some(2.0);
        let mut tiler = TiledPolygonizer::new(bbox, 1.0).with_options(options);
        tiler.add_geometry(&square);
        assert!(tiler.polygonize().unwrap().polygons.is_empty());
    }

    #[test]
    fn bounds_tiled_call_cardinality_before_materializing_tiles() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let bounded =
            TiledPolygonizer::new(bbox, 10.0).with_tile_execution_policy(TileExecutionPolicy {
                max_tiles: Some(1),
                ..Default::default()
            });
        assert!(matches!(
            bounded.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 4,
            }) if stage == "tile_count"
        ));

        let extreme = TiledPolygonizer::new(
            Rect::new(
                Coord { x: 0.0, y: 0.0 },
                Coord {
                    x: f64::MAX,
                    y: 1.0,
                },
            ),
            1.0,
        );
        assert!(matches!(
            extreme.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded { ref stage, .. })
                if stage == "tile_count"
        ));
    }

    #[test]
    fn bounds_tiled_geometry_assignments_and_input_count() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let line = Geometry::LineString(LineString::new(vec![
            Coord { x: 0.0, y: 5.0 },
            Coord { x: 20.0, y: 5.0 },
        ]));
        let mut assignments =
            TiledPolygonizer::new(bbox, 10.0).with_tile_execution_policy(TileExecutionPolicy {
                max_tile_geometry_assignments: Some(1),
                ..Default::default()
            });
        assignments.add_geometry(&line);
        assert!(matches!(
            assignments.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            }) if stage == "tile_geometry_assignments"
        ));

        let mut input_bound =
            TiledPolygonizer::new(bbox, 10.0).with_tile_execution_policy(TileExecutionPolicy {
                max_input_geometries: Some(0),
                ..Default::default()
            });
        input_bound.add_geometry(&line);
        assert!(matches!(
            input_bound.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 1,
            }) if stage == "tile_input_geometries"
        ));
    }

    #[test]
    fn tile_generation_observes_cancellation_and_parallelism_validation() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let token = CancellationToken::new();
        token.cancel();
        let cancelled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        });
        assert!(matches!(
            cancelled.polygonize(),
            Err(PolygonizeError::Cancelled { ref stage }) if stage == "tile_generation"
        ));

        let invalid =
            TiledPolygonizer::new(bbox, 10.0).with_tile_execution_policy(TileExecutionPolicy {
                max_parallel_tiles: Some(0),
                ..Default::default()
            });
        assert!(matches!(
            invalid.polygonize(),
            Err(PolygonizeError::InvalidArgumentType { ref field, .. })
                if field == "tile_execution_policy.max_parallel_tiles"
        ));
    }

    #[test]
    fn canonical_tile_keys_normalize_signed_zero() {
        let polygon = |zero: f64| {
            Polygon3D::new(
                vec![
                    Coord3D::new(zero, 0.0, 0.0),
                    Coord3D::new(1.0, 0.0, 0.0),
                    Coord3D::new(1.0, 1.0, 0.0),
                    Coord3D::new(zero, 1.0, 0.0),
                    Coord3D::new(zero, 0.0, 0.0),
                ],
                vec![],
                vec![],
                vec![],
            )
        };
        assert_eq!(
            crate::tiling::canonical_polygon_key(&polygon(-0.0)),
            crate::tiling::canonical_polygon_key(&polygon(0.0))
        );
    }

    #[test]
    fn partition_snapshot_matches_independent_reprocess() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 2.0 });
        let square = Geometry::LineString(LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ]));
        let mut serial = TiledPolygonizer::new(bbox, 2.0)
            .with_tile_execution_policy(TileExecutionPolicy {
                max_parallel_tiles: Some(1),
                ..Default::default()
            })
            .with_options(PolygonizerOptions {
                provenance: ProvenanceOptions {
                    enabled: true,
                    include_boundary_line_ids: true,
                },
                input_profile_id: Some("partition-oracle-v1".to_string()),
                ..Default::default()
            });
        serial.add_geometry(&square);
        let result = serial.polygonize().unwrap();
        let snapshot = &result.partition_snapshots[0];

        assert_eq!(snapshot.schema_version, 12);
        assert_eq!(snapshot.partition_id, 0);
        assert_eq!(snapshot.selected_input_geometry_indices, vec![0]);
        assert_eq!(snapshot.selected_source_segments.len(), 4);
        assert_eq!(snapshot.local_noded_segments.len(), 4);
        assert_eq!(snapshot.boundary_noded_segments.len(), 4);
        assert!(!snapshot.atomic_observations.is_empty());
        let observation_with_boundary_successor = snapshot
            .atomic_observations
            .iter()
            .find(|observation| observation.local_face_boundary_successor.is_some())
            .unwrap();
        assert_eq!(
            observation_with_boundary_successor
                .local_face_boundary_successor
                .unwrap()
                .partition_id,
            0
        );
        let observation_with_face_state = snapshot
            .atomic_observations
            .iter()
            .find(|observation| {
                observation.face_ref.is_some() && observation.local_face_successor.is_some()
            })
            .unwrap();
        assert_eq!(observation_with_face_state.face_ref.unwrap()[0], 0);
        assert!(!snapshot.local_face_graphs.is_empty());
        assert!(!snapshot.boundary_nodes.is_empty());
        assert!(snapshot.local_face_graphs[0]
            .nodes
            .iter()
            .any(|node| node.outgoing_local_dir_edge_ids.len() >= 2));
        assert_eq!(snapshot.local_face_graphs[0].graph_state.node_count, 4);
        assert_eq!(snapshot.local_face_graphs[0].graph_state.edge_count, 4);
        assert_eq!(
            snapshot.local_face_graphs[0].graph_state.active_edge_count,
            4
        );
        assert_eq!(
            snapshot.local_face_graphs[0]
                .graph_state
                .active_directed_edge_count,
            8
        );
        assert!(snapshot.local_face_graphs[0].graph_state.face_count > 0);
        assert!(
            snapshot.local_face_graphs[0]
                .graph_state
                .unbounded_face_count
                > 0
        );
        assert_eq!(
            snapshot.local_face_graphs[0].directed_edges[0].representative_line_id,
            Some(0)
        );
        assert_eq!(snapshot.topology.polygons.len(), 1);
        let provenance = snapshot.topology.polygons[0].provenance.as_ref().unwrap();
        assert!(provenance.boundary_line_ids.is_empty());
        assert_eq!(
            provenance.input_profile_id.as_deref(),
            Some("partition-oracle-v1")
        );
        let independent = serial
            .process_one_partition(0, bbox, serial.buffer)
            .unwrap();
        assert_eq!(snapshot, &independent);
        assert_eq!(snapshot.diff(&independent), None);
        assert_eq!(serial.partition_oracle_first_difference().unwrap(), None);
        let mut mismatch = independent.clone();
        mismatch.selected_input_geometry_indices.push(1);
        assert_eq!(
            snapshot.diff(&mismatch).unwrap().path,
            "$.selected_input_geometry_indices"
        );
        let mut source_mismatch = independent.clone();
        source_mismatch.selected_source_segments[0].segment_index += 1;
        assert_eq!(
            snapshot.diff(&source_mismatch).unwrap().path,
            "$.selected_source_segments"
        );
        let mut noded_segment_mismatch = independent.clone();
        noded_segment_mismatch.local_noded_segments[0]
            .source_line_ids
            .push(u32::MAX);
        assert_eq!(
            snapshot.diff(&noded_segment_mismatch).unwrap().path,
            "$.local_noded_segments"
        );
        let mut noded_representative_mismatch = independent.clone();
        noded_representative_mismatch.local_noded_segments[0].representative_line_id =
            Some(u32::MAX);
        assert_eq!(
            snapshot.diff(&noded_representative_mismatch).unwrap().path,
            "$.local_noded_segments"
        );
        let mut boundary_noded_segment_mismatch = independent.clone();
        boundary_noded_segment_mismatch.boundary_noded_segments[0]
            .source_line_ids
            .push(u32::MAX);
        assert_eq!(
            snapshot
                .diff(&boundary_noded_segment_mismatch)
                .unwrap()
                .path,
            "$.boundary_noded_segments"
        );
        let mut boundary_noding_mismatch = independent.clone();
        boundary_noding_mismatch.boundary_noding.added_node_count += 1;
        assert_eq!(
            snapshot.diff(&boundary_noding_mismatch).unwrap().path,
            "$.boundary_noding.added_node_count"
        );
        let mut atomic_observation_mismatch = independent.clone();
        atomic_observation_mismatch.atomic_observations[0].from_z_bits ^= 1;
        assert_eq!(
            snapshot.diff(&atomic_observation_mismatch).unwrap().path,
            "$.atomic_observations"
        );
        let mut boundary_successor_mismatch = independent.clone();
        boundary_successor_mismatch.atomic_observations[0].local_face_boundary_successor = None;
        assert_eq!(
            snapshot.diff(&boundary_successor_mismatch).unwrap().path,
            "$.atomic_observations"
        );
        let mut atomic_face_state_mismatch = independent.clone();
        let face_state_index = atomic_face_state_mismatch
            .atomic_observations
            .iter()
            .position(|observation| observation.face_ref.is_some())
            .unwrap();
        atomic_face_state_mismatch.atomic_observations[face_state_index].face_ref = None;
        atomic_face_state_mismatch.atomic_observations[face_state_index].local_face_is_unbounded =
            !atomic_face_state_mismatch.atomic_observations[face_state_index]
                .local_face_is_unbounded;
        assert_eq!(
            snapshot.diff(&atomic_face_state_mismatch).unwrap().path,
            "$.atomic_observations"
        );
        let mut local_face_graph_mismatch = independent.clone();
        local_face_graph_mismatch.local_face_graphs[0].directed_edges[0].local_face_is_unbounded =
            !local_face_graph_mismatch.local_face_graphs[0].directed_edges[0]
                .local_face_is_unbounded;
        assert_eq!(
            snapshot.diff(&local_face_graph_mismatch).unwrap().path,
            "$.local_face_graphs"
        );
        let mut local_face_representative_mismatch = independent.clone();
        local_face_representative_mismatch.local_face_graphs[0].directed_edges[0]
            .representative_line_id = Some(u32::MAX);
        assert_eq!(
            snapshot
                .diff(&local_face_representative_mismatch)
                .unwrap()
                .path,
            "$.local_face_graphs"
        );
        let mut local_node_mismatch = independent.clone();
        local_node_mismatch.local_face_graphs[0].nodes[0]
            .outgoing_local_dir_edge_ids
            .push(usize::MAX);
        assert_eq!(
            snapshot.diff(&local_node_mismatch).unwrap().path,
            "$.local_face_graphs"
        );
        let mut local_graph_state_mismatch = independent.clone();
        local_graph_state_mismatch.local_face_graphs[0]
            .graph_state
            .face_count += 1;
        assert_eq!(
            snapshot.diff(&local_graph_state_mismatch).unwrap().path,
            "$.local_face_graphs"
        );
        let mut boundary_node_mismatch = independent.clone();
        boundary_node_mismatch.boundary_nodes[0].z_bits.push(1);
        assert_eq!(
            snapshot.diff(&boundary_node_mismatch).unwrap().path,
            "$.boundary_nodes"
        );
        let mut non_polygon_mismatch = independent.clone();
        non_polygon_mismatch
            .non_polygon
            .dangles
            .push(vec![[0, 0, 0], [1, 1, 0]]);
        assert_eq!(
            snapshot.diff(&non_polygon_mismatch).unwrap().path,
            "$.non_polygon"
        );
        let mut topology_mismatch = independent.clone();
        topology_mismatch.topology.schema_version += 1;
        assert_eq!(
            snapshot.diff(&topology_mismatch).unwrap().path,
            "$.topology.schema_version"
        );
        let mut provenance_mismatch = independent.clone();
        provenance_mismatch.topology.polygons[0]
            .provenance
            .as_mut()
            .unwrap()
            .input_profile_id = Some("different-profile".to_string());
        assert_eq!(
            snapshot.diff(&provenance_mismatch).unwrap().path,
            "$.topology.polygons[0].provenance.input_profile_id"
        );
        let mut source_id_mismatch = independent.clone();
        source_id_mismatch.topology.polygons[0]
            .provenance
            .as_mut()
            .unwrap()
            .boundary_line_ids
            .push("0xdeadbeef".to_string());
        assert_eq!(
            snapshot.diff(&source_id_mismatch).unwrap().path,
            "$.topology.polygons[0].provenance.boundary_line_ids[0]"
        );
        let repeated = serial.polygonize().unwrap();
        assert_eq!(
            snapshot.fingerprint_sha256(),
            repeated.partition_snapshots[0].fingerprint_sha256()
        );
        assert_eq!(result.partition_snapshots.len(), result.tile_reports.len());
    }

    #[test]
    fn partition_oracle_survives_input_metamorphisms() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 2.0 });
        let points = [
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 2.0, y: 0.0 },
            Coord { x: 2.0, y: 2.0 },
            Coord { x: 0.0, y: 2.0 },
        ];
        let ring = Geometry::LineString(LineString::new(vec![
            points[0], points[1], points[2], points[3], points[0],
        ]));
        let reversed = Geometry::LineString(LineString::new(
            [points[0], points[3], points[2], points[1], points[0]].to_vec(),
        ));
        let edges = [
            LineString::new(vec![points[0], points[1]]),
            LineString::new(vec![points[1], points[2]]),
            LineString::new(vec![points[2], points[3]]),
            LineString::new(vec![points[3], points[0]]),
        ];
        let permuted = edges
            .iter()
            .rev()
            .cloned()
            .map(Geometry::LineString)
            .collect::<Vec<_>>();
        let grouped = Geometry::MultiLineString(MultiLineString::new(edges.to_vec()));
        let duplicate_vertex = Geometry::LineString(LineString::new(vec![
            points[0], points[1], points[1], points[2], points[3], points[0],
        ]));
        let duplicate_edge = Geometry::MultiLineString(MultiLineString::new(vec![
            edges[0].clone(),
            edges[0].clone(),
            edges[1].clone(),
            edges[2].clone(),
            edges[3].clone(),
        ]));

        let run = |label: &str, geometries: Vec<Geometry<f64>>| {
            let mut tiled = TiledPolygonizer::new(bbox, 3.0).with_buffer(0.25);
            for geometry in &geometries {
                tiled.add_geometry(geometry);
            }
            let result = tiled.polygonize().unwrap();
            for (partition_id, report) in result.tile_reports.iter().enumerate() {
                let independent = tiled
                    .process_one_partition(partition_id, report.tile_bbox, tiled.buffer)
                    .unwrap();
                assert_eq!(
                    result.partition_snapshots[partition_id], independent,
                    "{label} partition {partition_id} differs from independent reprocess"
                );
            }
            let mut output = result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>();
            output.sort_unstable();
            output
        };

        let expected = run("ring", vec![ring]);
        assert_eq!(run("reversed", vec![reversed]), expected);
        assert_eq!(run("permuted", permuted), expected);
        assert_eq!(run("grouped", vec![grouped]), expected);
        assert_eq!(run("duplicate vertex", vec![duplicate_vertex]), expected);
        assert_eq!(run("duplicate edge", vec![duplicate_edge]), expected);
    }

    #[test]
    fn partition_oracle_survives_tile_metamorphisms() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 4.0 });
        let make_square = |offset: f64| {
            Geometry::LineString(LineString::new(vec![
                Coord {
                    x: 1.0 + offset,
                    y: 1.0 + offset,
                },
                Coord {
                    x: 3.0 + offset,
                    y: 1.0 + offset,
                },
                Coord {
                    x: 3.0 + offset,
                    y: 3.0 + offset,
                },
                Coord {
                    x: 1.0 + offset,
                    y: 3.0 + offset,
                },
                Coord {
                    x: 1.0 + offset,
                    y: 1.0 + offset,
                },
            ]))
        };
        let run = |label: &str,
                   bbox: Rect<f64>,
                   tile_size: f64,
                   buffer: f64,
                   options: PolygonizerOptions,
                   geometry: Geometry<f64>,
                   offset: f64| {
            let mut tiled = TiledPolygonizer::new(bbox, tile_size)
                .with_buffer(buffer)
                .with_options(options);
            tiled.add_geometry(&geometry);
            let result = tiled.polygonize().unwrap();
            for (partition_id, report) in result.tile_reports.iter().enumerate() {
                let independent = tiled
                    .process_one_partition(partition_id, report.tile_bbox, tiled.buffer)
                    .unwrap();
                assert_eq!(
                    result.partition_snapshots[partition_id], independent,
                    "{label} partition {partition_id} differs from independent reprocess"
                );
            }
            let mut output = result
                .polygons
                .iter()
                .map(|polygon| {
                    let mut polygon = polygon.clone();
                    for coordinate in polygon.exterior.iter_mut().chain(
                        polygon
                            .interiors
                            .iter_mut()
                            .flat_map(|ring| ring.iter_mut()),
                    ) {
                        coordinate.x -= offset;
                        coordinate.y -= offset;
                    }
                    crate::tiling::canonical_polygon_key(&polygon)
                })
                .collect::<Vec<_>>();
            output.sort_unstable();
            output
        };

        let expected = run(
            "base",
            bbox,
            2.0,
            4.0,
            PolygonizerOptions::default(),
            make_square(0.0),
            0.0,
        );
        assert_eq!(
            run(
                "tile size",
                bbox,
                3.0,
                4.0,
                PolygonizerOptions::default(),
                make_square(0.0),
                0.0,
            ),
            expected
        );
        assert_eq!(
            run(
                "buffer",
                bbox,
                4.0,
                0.0,
                PolygonizerOptions::default(),
                make_square(0.0),
                0.0,
            ),
            expected
        );
        assert_eq!(
            run(
                "precision",
                bbox,
                4.0,
                0.0,
                PolygonizerOptions {
                    precision_model: PrecisionModel::FixedGrid { grid_size: 0.5 },
                    ..Default::default()
                },
                make_square(0.0),
                0.0,
            ),
            expected
        );
        let shifted_bbox = Rect::new(Coord { x: 1.0, y: 1.0 }, Coord { x: 5.0, y: 5.0 });
        assert_eq!(
            run(
                "tile origin",
                shifted_bbox,
                2.0,
                4.0,
                PolygonizerOptions::default(),
                make_square(1.0),
                1.0,
            ),
            expected
        );
    }

    #[test]
    fn partition_snapshot_exhaustively_scans_bounded_empty_neighbors() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 4.0 });
        let square = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.25, y: 1.25 },
            Coord { x: 1.75, y: 1.25 },
            Coord { x: 1.75, y: 1.75 },
            Coord { x: 1.25, y: 1.75 },
            Coord { x: 1.25, y: 1.25 },
        ]));
        let mut tiled = TiledPolygonizer::new(bbox, 1.0).with_buffer(0.0);
        tiled.add_geometry(&square);

        let result = tiled.polygonize().unwrap();
        assert_eq!(result.partition_snapshots.len(), 16);
        assert_eq!(result.partition_snapshots.len(), result.tile_reports.len());
        let nonempty_partition_ids = result
            .partition_snapshots
            .iter()
            .filter(|snapshot| !snapshot.selected_input_geometry_indices.is_empty())
            .map(|snapshot| snapshot.partition_id)
            .collect::<Vec<_>>();
        assert_eq!(nonempty_partition_ids, vec![5]);
        assert_eq!(
            result
                .partition_snapshots
                .iter()
                .filter(|snapshot| snapshot.selected_input_geometry_indices.is_empty())
                .count(),
            15
        );

        for (partition_id, report) in result.tile_reports.iter().enumerate() {
            let independent = tiled
                .process_one_partition(partition_id, report.tile_bbox, tiled.buffer)
                .unwrap();
            assert_eq!(
                result.partition_snapshots[partition_id], independent,
                "partition {partition_id} differs from independent bounded scan"
            );
        }
    }

    #[test]
    fn partition_oracle_normalizes_bulk_and_independent_errors() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 2.0 });
        let square = Geometry::LineString(LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ]));
        let mut tiled = TiledPolygonizer::new(bbox, 2.0).with_execution_policy(ExecutionPolicy {
            max_graph_nodes: Some(0),
            ..Default::default()
        });
        tiled.add_geometry(&square);
        let components = tiled.input_components().unwrap();
        let bulk_error = match tiled.process_tile_with_retries(
            0,
            bbox,
            &components,
            None,
            &std::sync::atomic::AtomicUsize::new(0),
        ) {
            Ok(_) => panic!("expected bulk partition processing to fail"),
            Err(error) => error,
        };
        let independent_error = tiled
            .process_one_partition(0, bbox, tiled.buffer)
            .unwrap_err();
        let bulk = crate::fingerprint::normalize_polygonize_error(&bulk_error);
        let independent = crate::fingerprint::normalize_polygonize_error(&independent_error);
        assert_eq!(bulk, independent);
        assert_eq!(bulk.family, "resource_limit");
        assert_eq!(bulk.code, "resource_limit_exceeded");
    }

    #[test]
    fn reports_tile_topology_and_merge_counts() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 2.0 });
        let square = Geometry::LineString(LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ]));
        let dangle = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.5, y: 0.0 },
            Coord { x: 1.5, y: 1.0 },
        ]));
        let mut tiler = TiledPolygonizer::new(bbox, 2.0);
        tiler.add_geometry(&square);
        tiler.add_geometry(&dangle);

        let result = tiler.polygonize().unwrap();
        assert_eq!(result.tile_reports.len(), 1);
        let report = &result.tile_reports[0];
        assert_eq!(report.input_geometry_count, 2);
        assert_eq!(report.polygon_count, 1);
        assert_eq!(report.owned_polygon_count, 1);
        assert_eq!(report.dangle_count, 1);
        assert_eq!(report.cut_edge_count, 0);
        assert_eq!(report.invalid_ring_count, 0);
        assert_eq!(result.stitching_report.merged_polygon_count, 1);
        assert_eq!(result.stitching_report.duplicate_polygon_count, 0);
        assert_eq!(result.stitching_report.output_polygon_count, 1);
    }

    #[test]
    fn tiled_merge_applies_aggregate_output_limit() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let squares = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 1.0, y: 1.0 },
                Coord { x: 3.0, y: 1.0 },
                Coord { x: 3.0, y: 3.0 },
                Coord { x: 1.0, y: 3.0 },
                Coord { x: 1.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 11.0, y: 1.0 },
                Coord { x: 13.0, y: 1.0 },
                Coord { x: 13.0, y: 3.0 },
                Coord { x: 11.0, y: 3.0 },
                Coord { x: 11.0, y: 1.0 },
            ])),
        ];
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(ExecutionPolicy {
            max_output_polygons: Some(1),
            ..Default::default()
        });
        for square in &squares {
            tiled.add_geometry(square);
        }

        assert!(matches!(
            tiled.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            }) if stage == "output_polygons"
        ));
    }

    #[test]
    fn tiled_merge_applies_aggregate_coordinate_limit() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let squares = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 1.0, y: 1.0 },
                Coord { x: 3.0, y: 1.0 },
                Coord { x: 3.0, y: 3.0 },
                Coord { x: 1.0, y: 3.0 },
                Coord { x: 1.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 11.0, y: 1.0 },
                Coord { x: 13.0, y: 1.0 },
                Coord { x: 13.0, y: 3.0 },
                Coord { x: 11.0, y: 3.0 },
                Coord { x: 11.0, y: 1.0 },
            ])),
        ];
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(ExecutionPolicy {
            max_output_coordinates: Some(5),
            ..Default::default()
        });
        for square in &squares {
            tiled.add_geometry(square);
        }

        assert!(matches!(
            tiled.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 5,
                observed: 10,
            }) if stage == "output_coordinates"
        ));
    }

    #[test]
    fn reports_owned_faces_that_escape_an_internal_halo() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let face = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.0, y: 2.0 },
            Coord { x: 19.0, y: 2.0 },
            Coord { x: 19.0, y: 8.0 },
            Coord { x: 1.0, y: 8.0 },
            Coord { x: 1.0, y: 2.0 },
        ]));
        let mut tiler = TiledPolygonizer::new(bbox, 10.0).with_buffer(2.0);
        tiler.add_geometry(&face);

        let result = tiler.polygonize().unwrap();
        let issues: Vec<_> = result
            .tile_reports
            .iter()
            .flat_map(|report| &report.coverage_issues)
            .collect();

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].unresolved_sides, vec![TileBoundarySide::MinX]);
        assert_eq!(issues[0].polygon_bbox.min().x, 1.0);
        assert_eq!(issues[0].polygon_bbox.max().x, 19.0);
        assert!(!issues[0].representative_source_line_ids.is_empty());
        assert!(issues[0].aggregate_source_line_ids.is_empty());
        assert!(!issues[0].aggregate_source_line_ids_complete);
        assert_eq!(result.stitching_report.unresolved_tile_count, 1);
        assert_eq!(result.stitching_report.unresolved_owned_polygon_count, 1);
        let traced = tiler
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let event = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "tile_owned_face_boundary")
            .unwrap();
        assert_eq!(event.payload["polygon_index"], 0);
        assert_eq!(event.payload["unresolved_sides"][0], "min_x");
        assert!(!event.payload["representative_source_line_ids"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(event.payload["aggregate_source_line_ids_complete"], false);

        let mut tiler = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(PolygonizerOptions {
                provenance: ProvenanceOptions {
                    enabled: true,
                    include_boundary_line_ids: true,
                },
                ..Default::default()
            });
        tiler.add_geometry(&face);
        let result = tiler.polygonize().unwrap();
        let issue = result
            .tile_reports
            .iter()
            .flat_map(|report| &report.coverage_issues)
            .next()
            .unwrap();
        assert!(!issue.aggregate_source_line_ids.is_empty());
        assert!(issue.aggregate_source_line_ids_complete);
    }

    #[test]
    fn grouped_line_containers_preserve_observed_coverage_contract() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let grouped = [
            Geometry::MultiLineString(MultiLineString::new(vec![
                LineString::new(vec![Coord { x: 1.0, y: 2.0 }, Coord { x: 19.0, y: 2.0 }]),
                LineString::new(vec![Coord { x: 19.0, y: 8.0 }, Coord { x: 1.0, y: 8.0 }]),
            ])),
            Geometry::MultiLineString(MultiLineString::new(vec![
                LineString::new(vec![Coord { x: 19.0, y: 2.0 }, Coord { x: 19.0, y: 8.0 }]),
                LineString::new(vec![Coord { x: 1.0, y: 8.0 }, Coord { x: 1.0, y: 2.0 }]),
            ])),
        ];
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_buffer(2.0);
        for geometry in &grouped {
            tiled.add_geometry(geometry);
        }

        let result = tiled.polygonize().unwrap();
        assert_eq!(result.polygons.len(), 1);
        assert_eq!(result.tile_reports.len(), 2);
        assert!(result
            .tile_reports
            .iter()
            .all(|report| report.input_geometry_count == grouped.len()));
        assert!(result
            .tile_reports
            .iter()
            .any(|report| !report.coverage_issues.is_empty()));

        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateOwnedFaces),
            Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_owned_polygon_count: 1,
                ..
            })
        ));
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete { .. })
        ));
    }

    #[test]
    fn component_fallback_recovers_closed_boundary_region() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let face = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.0, y: 2.0 },
            Coord { x: 19.0, y: 2.0 },
            Coord { x: 19.0, y: 8.0 },
            Coord { x: 1.0, y: 8.0 },
            Coord { x: 1.0, y: 2.0 },
        ]));
        let mut untiled = Polygonizer::new();
        untiled.add_borrowed_geometry(&face);
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_component_fallback();
        tiled.add_geometry(&face);

        let expected = untiled
            .polygonize()
            .unwrap()
            .polygons
            .into_iter()
            .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
            .collect::<Vec<_>>();
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(result.polygons.len(), 1);
        assert!(result.stitching_report.component_fallback_attempted);
        assert!(result.stitching_report.component_fallback_used);
        assert_eq!(
            result.stitching_report.coverage_resolution.resolution,
            TileCoverageResolutionKind::ComponentFallback
        );
        assert_eq!(
            result
                .stitching_report
                .coverage_resolution
                .unresolved_issue_count,
            0
        );
        assert_eq!(result.stitching_report.component_fallback_count, 1);
        assert_eq!(result.stitching_report.component_fallback_polygon_count, 1);
        assert_eq!(
            result
                .stitching_report
                .component_fallback_replaced_polygon_count,
            1
        );

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let recovered = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "tile_component_fallback")
            .unwrap();
        assert_eq!(
            recovered.payload["input_geometry_indices"],
            serde_json::json!([0])
        );
        assert_eq!(recovered.payload["output_polygon_count"], 1);
        assert_eq!(recovered.payload["recovered_component_count"], 1);
        assert!(!traced
            .trace
            .events
            .iter()
            .any(|event| event.kind == "tile_component_fallback_declined"));

        let mut limited = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_component_fallback()
            .with_tile_execution_policy(TileExecutionPolicy {
                max_fallback_regions: Some(0),
                ..Default::default()
            });
        limited.add_geometry(&face);
        assert!(matches!(
            limited.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 1,
            }) if stage == "tile_fallback_regions"
        ));
    }

    #[test]
    fn component_fallback_does_not_authorize_unresolved_ownership_domain_evidence() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let recovered_face = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.0, y: 2.0 },
            Coord { x: 19.0, y: 2.0 },
            Coord { x: 19.0, y: 8.0 },
            Coord { x: 1.0, y: 8.0 },
            Coord { x: 1.0, y: 2.0 },
        ]));
        let out_of_domain_face = Geometry::LineString(LineString::new(vec![
            Coord { x: 16.0, y: 2.0 },
            Coord { x: 26.0, y: 2.0 },
            Coord { x: 26.0, y: 8.0 },
            Coord { x: 16.0, y: 8.0 },
            Coord { x: 16.0, y: 2.0 },
        ]));
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_component_fallback();
        tiled.add_geometry(&recovered_face);
        tiled.add_geometry(&out_of_domain_face);

        let observed = tiled.polygonize().unwrap();
        assert!(observed.stitching_report.component_fallback_used);
        assert!(observed.stitching_report.unresolved_ownership_domain_count > 0);
        assert!(
            observed
                .stitching_report
                .coverage_resolution
                .resolved_issue_count
                > 0
        );
        assert!(
            observed
                .stitching_report
                .coverage_resolution
                .unresolved_issue_count
                > 0
        );
        assert_eq!(
            observed.stitching_report.coverage_resolution.resolution,
            TileCoverageResolutionKind::Partial
        );
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage),
            Err(TiledPolygonizeError::CoverageIncomplete {
                coverage_resolution,
                ..
            }) if coverage_resolution.unresolved_issue_count > 0
        ));
    }

    #[test]
    fn component_fallback_preserves_nested_closed_boundary_containment() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 1.0, y: 1.0 },
                Coord { x: 19.0, y: 1.0 },
                Coord { x: 19.0, y: 9.0 },
                Coord { x: 1.0, y: 9.0 },
                Coord { x: 1.0, y: 1.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 4.0, y: 3.0 },
                Coord { x: 16.0, y: 3.0 },
                Coord { x: 16.0, y: 7.0 },
                Coord { x: 4.0, y: 7.0 },
                Coord { x: 4.0, y: 3.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in &geometries {
            untiled.add_borrowed_geometry(geometry);
            tiled.add_geometry(geometry);
        }

        let expected = untiled
            .polygonize()
            .unwrap()
            .polygons
            .into_iter()
            .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
            .collect::<Vec<_>>();
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(result.polygons.len(), 2);
        assert!(result
            .polygons
            .iter()
            .any(|polygon| !polygon.interiors.is_empty()));
        assert_eq!(result.stitching_report.component_fallback_count, 1);
    }

    #[test]
    fn declines_closed_boundary_fallback_for_open_single_geometry() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let open_boundary = Geometry::MultiLineString(MultiLineString::new(vec![
            LineString::new(vec![Coord { x: 1.0, y: 2.0 }, Coord { x: 19.0, y: 2.0 }]),
            LineString::new(vec![Coord { x: 19.0, y: 2.0 }, Coord { x: 19.0, y: 8.0 }]),
            LineString::new(vec![Coord { x: 19.0, y: 8.0 }, Coord { x: 1.0, y: 8.0 }]),
            LineString::new(vec![Coord { x: 1.0, y: 8.0 }, Coord { x: 1.0, y: 2.0 }]),
        ]));
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_component_fallback();
        tiled.add_geometry(&open_boundary);

        let result = tiled.polygonize().unwrap();
        assert!(result.stitching_report.component_fallback_attempted);
        assert!(!result.stitching_report.component_fallback_used);
        assert_eq!(
            result.stitching_report.component_fallback_decline_reason,
            Some("non_closed_recovery_region")
        );

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let declined = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "tile_component_fallback_declined")
            .unwrap();
        assert_eq!(declined.payload["reason"], "non_closed_recovery_region");
        assert_eq!(
            declined.payload["unresolved_owned_polygon_count"],
            result.stitching_report.unresolved_owned_polygon_count
        );
        assert_eq!(
            declined.payload["unresolved_input_geometry_count"],
            result.stitching_report.unresolved_input_geometry_count
        );
        assert_eq!(declined.payload["unresolved_component_count"], 0);

        let error = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap_err();
        assert!(matches!(
            error,
            TiledPolygonizeError::CoverageIncomplete {
                component_fallback_decline_reason: Some("non_closed_recovery_region"),
                ..
            }
        ));
    }

    #[test]
    fn declines_closed_boundary_fallback_for_empty_recovery_output() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let degenerate_closed = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.0, y: 2.0 },
            Coord { x: 19.0, y: 2.0 },
            Coord { x: 1.0, y: 2.0 },
            Coord { x: 1.0, y: 2.0 },
        ]));
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(0.0)
            .with_component_fallback();
        tiled.add_geometry(&degenerate_closed);

        let result = tiled.polygonize().unwrap();
        assert!(result.polygons.is_empty());
        assert!(result.stitching_report.component_fallback_attempted);
        assert!(!result.stitching_report.component_fallback_used);
        assert_eq!(
            result.stitching_report.component_fallback_decline_reason,
            Some("empty_recovery_output")
        );

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let declined = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "tile_component_fallback_declined")
            .unwrap();
        assert_eq!(declined.payload["reason"], "empty_recovery_output");
        assert_eq!(
            declined.payload["unresolved_input_geometry_count"],
            result.stitching_report.unresolved_input_geometry_count
        );
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete {
                component_fallback_decline_reason: Some("empty_recovery_output"),
                ..
            })
        ));

        let mut globally_fallback = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(0.0)
            .with_component_fallback()
            .with_untiled_fallback();
        globally_fallback.add_geometry(&degenerate_closed);
        let result = globally_fallback.polygonize().unwrap();
        assert!(result.polygons.is_empty());
        assert!(result.stitching_report.untiled_fallback_attempted);
        assert!(result.stitching_report.untiled_fallback_authoritative);
        assert_eq!(
            result
                .stitching_report
                .untiled_fallback_output_polygon_count,
            0
        );
        assert!(result.stitching_report.untiled_fallback_used);
        assert_eq!(
            result.stitching_report.coverage_resolution.resolution,
            TileCoverageResolutionKind::UntiledFallback
        );
        assert_eq!(
            result
                .stitching_report
                .coverage_resolution
                .unresolved_issue_count,
            0
        );
        assert_eq!(
            result.stitching_report.component_fallback_decline_reason,
            Some("empty_recovery_output")
        );
        let traced = globally_fallback
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let recovery_events = traced
            .trace
            .events
            .iter()
            .filter_map(|event| match event.kind.as_str() {
                "tile_component_fallback_declined" | "tile_untiled_fallback" => {
                    Some(event.kind.as_str())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            recovery_events,
            vec!["tile_component_fallback_declined", "tile_untiled_fallback"]
        );
    }

    #[test]
    fn reports_boundary_inputs_when_no_local_face_is_reconstructed() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 1.0, y: 2.0 },
                Coord { x: 19.0, y: 2.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 19.0, y: 2.0 },
                Coord { x: 19.0, y: 8.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 19.0, y: 8.0 },
                Coord { x: 1.0, y: 8.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 1.0, y: 8.0 },
                Coord { x: 1.0, y: 2.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_buffer(2.0);
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        assert_eq!(untiled.polygonize().unwrap().polygons.len(), 1);
        let result = tiled.polygonize().unwrap();
        assert!(result.polygons.is_empty());
        let issues: Vec<_> = result
            .tile_reports
            .iter()
            .flat_map(|report| &report.input_boundary_issues)
            .collect();
        assert_eq!(issues.len(), 4);
        assert!(issues
            .iter()
            .all(|issue| { issue.input_geometry_index == 0 || issue.input_geometry_index == 2 }));
        assert!(result.tile_reports[0]
            .input_boundary_issues
            .iter()
            .all(|issue| issue.unresolved_sides == vec![TileBoundarySide::MaxX]));
        assert!(result.tile_reports[1]
            .input_boundary_issues
            .iter()
            .all(|issue| issue.unresolved_sides == vec![TileBoundarySide::MinX]));
        assert_eq!(result.stitching_report.unresolved_input_tile_count, 2);
        assert_eq!(result.stitching_report.unresolved_input_geometry_count, 4);
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_input_tile_count: 2,
                unresolved_input_geometry_count: 4,
                ..
            })
        ));
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let boundary_events: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_input_boundary")
            .collect();
        assert_eq!(boundary_events.len(), 4);
        assert_eq!(boundary_events[0].payload["tile_index"], 0);
        assert_eq!(boundary_events[0].payload["input_geometry_index"], 0);
        assert_eq!(boundary_events[0].payload["unresolved_sides"][0], "max_x");
        let bounded = tiled.polygonize_with_trace(TraceLevelV1::Full, 0).unwrap();
        assert!(bounded.trace.events.is_empty());
        assert!(bounded.trace.truncated);
        assert_eq!(
            bounded
                .result
                .stitching_report
                .unresolved_input_geometry_count,
            4
        );
    }

    #[test]
    fn documents_component_excluded_from_every_halo() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 30.0 },
                Coord { x: -10.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_buffer(2.0);
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        assert_eq!(untiled.polygonize().unwrap().polygons.len(), 1);
        let observed = tiled.polygonize().unwrap();
        assert!(observed.polygons.is_empty());
        assert_eq!(observed.stitching_report.unresolved_input_geometry_count, 0);
        assert_eq!(observed.stitching_report.unresolved_owned_polygon_count, 0);
        assert_eq!(observed.stitching_report.unresolved_component_tile_count, 4);
        assert_eq!(observed.stitching_report.unresolved_component_count, 4);
        for report in &observed.tile_reports {
            assert_eq!(report.excluded_component_issues.len(), 1);
            let issue = &report.excluded_component_issues[0];
            assert_eq!(issue.input_geometry_indices, vec![0, 1, 2, 3]);
            assert_eq!(issue.component_bbox.min(), Coord { x: -10.0, y: -10.0 });
            assert_eq!(issue.component_bbox.max(), Coord { x: 30.0, y: 30.0 });
            assert_eq!(issue.connection, TileComponentConnection::ExactEndpoint);
        }
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let component_events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_excluded_endpoint_component")
            .collect::<Vec<_>>();
        assert_eq!(component_events.len(), 4);
        assert_eq!(component_events[0].payload["tile_index"], 0);
        assert_eq!(
            component_events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3])
        );
        let bounded = tiled.polygonize_with_trace(TraceLevelV1::Full, 0).unwrap();
        assert!(bounded.trace.events.is_empty());
        assert!(bounded.trace.truncated);
        assert_eq!(
            bounded.result.stitching_report.unresolved_component_count,
            4
        );
        assert!(tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateOwnedFaces)
            .is_ok());
        let error = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap_err();
        assert!(matches!(
            error,
            TiledPolygonizeError::CoverageIncomplete {
                unresolved_component_tile_count: 4,
                unresolved_component_count: 4,
                ..
            }
        ));
    }

    #[test]
    fn component_fallback_recovers_an_envelope_disjoint_component() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 30.0 },
                Coord { x: -10.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_component_fallback();
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        let untiled = untiled.polygonize().unwrap();
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(result.polygons.len(), 1);
        assert_eq!(
            crate::tiling::canonical_polygon_key(&result.polygons[0]),
            crate::tiling::canonical_polygon_key(&untiled.polygons[0])
        );
        assert!(result.stitching_report.component_fallback_used);
        assert!(!result.stitching_report.untiled_fallback_used);
        assert_eq!(result.stitching_report.retained_tile_polygon_count, 0);
        assert_eq!(result.stitching_report.component_fallback_count, 1);
        assert_eq!(result.stitching_report.component_fallback_polygon_count, 1);
        assert_eq!(result.stitching_report.unresolved_component_count, 4);

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_component_fallback")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3])
        );
        assert_eq!(events[0].payload["output_polygon_count"], 1);
        assert_eq!(events[0].payload["retained_tile_polygon_count"], 0);

        let bounded = tiled.polygonize_with_trace(TraceLevelV1::Full, 0).unwrap();
        assert!(bounded.trace.events.is_empty());
        assert!(bounded.trace.truncated);
        assert!(bounded.result.stitching_report.component_fallback_used);
        assert_eq!(bounded.result.polygons.len(), 1);

        let mut limited = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_execution_policy(ExecutionPolicy {
                max_output_polygons: Some(0),
                ..Default::default()
            })
            .with_component_fallback();
        for boundary in &boundaries {
            limited.add_geometry(boundary);
        }
        assert!(matches!(
            limited.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 1,
            }) if stage == "output_polygons"
        ));
    }

    #[test]
    fn component_fallback_recovers_input_boundary_connected_region() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 4.0, y: 4.0 },
                Coord { x: 6.0, y: 4.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 6.0, y: 4.0 },
                Coord { x: 16.0, y: 4.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 16.0, y: 4.0 },
                Coord { x: 16.0, y: 16.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 16.0, y: 16.0 },
                Coord { x: 4.0, y: 16.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 4.0, y: 16.0 },
                Coord { x: 4.0, y: 4.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(1.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in &geometries {
            untiled.add_borrowed_geometry(geometry);
            tiled.add_geometry(geometry);
        }

        let expected = untiled.polygonize().unwrap();
        assert_eq!(expected.polygons.len(), 1);
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(result.polygons.len(), 1);
        assert_eq!(
            crate::tiling::canonical_polygon_key(&result.polygons[0]),
            crate::tiling::canonical_polygon_key(&expected.polygons[0])
        );
        assert!(result.stitching_report.component_fallback_used);
        assert!(!result.stitching_report.untiled_fallback_used);
        assert_eq!(result.stitching_report.component_fallback_count, 1);
        assert!(result.stitching_report.unresolved_input_geometry_count > 0);
        assert!(result
            .tile_reports
            .iter()
            .all(|report| report.excluded_component_issues.is_empty()));

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_component_fallback")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3, 4])
        );

        let mut reversed = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(1.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in geometries.iter().rev() {
            reversed.add_geometry(geometry);
        }
        let reversed = reversed
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            reversed
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn component_fallback_merges_disjoint_retained_output() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 60.0, y: 60.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 30.0 },
                Coord { x: -10.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 52.0, y: 52.0 },
                Coord { x: 58.0, y: 52.0 },
                Coord { x: 58.0, y: 58.0 },
                Coord { x: 52.0, y: 58.0 },
                Coord { x: 52.0, y: 52.0 },
            ])),
        ];
        let options = PolygonizerOptions {
            node_input: true,
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            input_profile_id: Some("tiled-partial-merge-v1".to_string()),
            ..Default::default()
        };
        let mut untiled = Polygonizer::with_options(options.clone());
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(0.0)
            .with_options(options.clone())
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in &geometries {
            untiled.add_borrowed_geometry(geometry);
            tiled.add_geometry(geometry);
        }

        let expected = untiled.polygonize().unwrap();
        assert_eq!(expected.polygons.len(), 2);
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            expected
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>()
        );
        assert!(result.stitching_report.component_fallback_used);
        assert!(!result.stitching_report.untiled_fallback_used);
        assert!(result.stitching_report.unresolved_input_geometry_count > 0);
        assert!(result
            .tile_reports
            .iter()
            .flat_map(|report| &report.input_boundary_issues)
            .all(|issue| issue.input_geometry_index < 4));

        let mut expected_provenance = expected
            .polygons
            .iter()
            .map(|polygon| {
                let provenance = polygon.provenance.as_ref().unwrap();
                (
                    crate::tiling::canonical_polygon_key(polygon),
                    provenance.boundary_line_ids.clone(),
                    provenance.input_profile_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        let mut actual_provenance = result
            .polygons
            .iter()
            .map(|polygon| {
                let provenance = polygon.provenance.as_ref().unwrap();
                (
                    crate::tiling::canonical_polygon_key(polygon),
                    provenance.boundary_line_ids.clone(),
                    provenance.input_profile_id.clone(),
                )
            })
            .collect::<Vec<_>>();
        expected_provenance.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        actual_provenance.sort_unstable_by(|left, right| left.0.cmp(&right.0));
        assert_eq!(actual_provenance, expected_provenance);

        let mut reversed = TiledPolygonizer::new(bbox, 10.0)
            .with_options(options)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in geometries.iter().rev() {
            reversed.add_geometry(geometry);
        }
        let reversed = reversed
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            reversed
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>()
        );
        assert!(reversed.stitching_report.component_fallback_used);
    }

    #[test]
    fn component_fallback_merges_multiple_disjoint_components() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 100.0, y: 100.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 30.0 },
                Coord { x: -10.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 70.0, y: -10.0 },
                Coord { x: 110.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 110.0, y: -10.0 },
                Coord { x: 110.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 110.0, y: 30.0 },
                Coord { x: 70.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 70.0, y: 30.0 },
                Coord { x: 70.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 42.0, y: 42.0 },
                Coord { x: 48.0, y: 42.0 },
                Coord { x: 48.0, y: 48.0 },
                Coord { x: 42.0, y: 48.0 },
                Coord { x: 42.0, y: 42.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 52.0, y: 52.0 },
                Coord { x: 58.0, y: 52.0 },
                Coord { x: 58.0, y: 58.0 },
                Coord { x: 52.0, y: 58.0 },
                Coord { x: 52.0, y: 52.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in &geometries {
            untiled.add_borrowed_geometry(geometry);
            tiled.add_geometry(geometry);
        }

        let expected = untiled.polygonize().unwrap();
        assert_eq!(expected.polygons.len(), 4);
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(result.polygons.len(), 4);
        assert_eq!(
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            expected
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>()
        );
        assert!(result.stitching_report.component_fallback_used);
        assert!(!result.stitching_report.untiled_fallback_used);
        assert_eq!(result.stitching_report.retained_tile_polygon_count, 2);
        assert_eq!(result.stitching_report.component_fallback_count, 2);
        assert_eq!(result.stitching_report.component_fallback_polygon_count, 2);

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let fallback_events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_component_fallback")
            .collect::<Vec<_>>();
        assert_eq!(fallback_events.len(), 2);
        assert!(fallback_events
            .iter()
            .all(|event| event.payload["output_polygon_count"] == 1));
        assert!(fallback_events
            .iter()
            .all(|event| event.payload["retained_tile_polygon_count"] == 2));

        let mut reversed = TiledPolygonizer::new(bbox, 10.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in geometries.iter().rev() {
            reversed.add_geometry(geometry);
        }
        let reversed = reversed
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            reversed
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>()
        );
        assert!(reversed.stitching_report.component_fallback_used);
    }

    #[test]
    fn component_fallback_declines_coverage_evidence_outside_recovery_region() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 1.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 1.0, y: 0.0 },
                Coord { x: 1.0, y: 1.0 },
            ])),
        ];
        let retained_polygon = Polygon3D::new(
            vec![
                Coord3D::new(10.0, 10.0, 0.0),
                Coord3D::new(11.0, 10.0, 0.0),
                Coord3D::new(11.0, 11.0, 0.0),
                Coord3D::new(10.0, 11.0, 0.0),
                Coord3D::new(10.0, 10.0, 0.0),
            ],
            vec![],
            vec![],
            vec![],
        );
        let report = TileReport {
            tile_bbox: bbox,
            input_geometry_count: 0,
            polygon_count: 1,
            owned_polygon_count: 1,
            dangle_count: 0,
            cut_edge_count: 0,
            invalid_ring_count: 0,
            coverage_issues: vec![TileCoverageIssue {
                polygon_index: 0,
                polygon_bbox: Rect::new(Coord { x: 10.0, y: 10.0 }, Coord { x: 11.0, y: 11.0 }),
                unresolved_sides: vec![TileBoundarySide::MaxX],
                representative_source_line_ids: vec![],
                aggregate_source_line_ids: vec![],
                aggregate_source_line_ids_complete: false,
            }],
            ownership_domain_issues: vec![],
            input_boundary_issues: vec![],
            excluded_component_issues: vec![TileExcludedComponentIssue {
                input_geometry_indices: vec![0, 1],
                component_bbox: Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 1.0 }),
                connection: TileComponentConnection::ExactEndpoint,
            }],
            retry_attempts: vec![],
            retry_exhausted: false,
        };
        let mut tiled = TiledPolygonizer::new(bbox, 10.0);
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let component = InputComponent {
            input_geometry_indices: vec![0, 1],
            bbox: Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 1.0 }),
            connection: TileComponentConnection::ExactEndpoint,
        };

        assert!(matches!(
            tiled.try_component_fallback(&[vec![retained_polygon]], &[report], &[component]),
            Ok(ComponentFallbackDecision::Declined(
                ComponentFallbackDeclineReason::OwnedFaceCoverageEvidence,
            ))
        ));
    }

    #[test]
    fn component_fallback_observes_pre_cancelled_execution_policy() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let token = CancellationToken::new();
        token.cancel();
        let tiled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        });
        let component_indices = vec![0, 1];
        let report = TileReport {
            tile_bbox: bbox,
            input_geometry_count: 0,
            polygon_count: 0,
            owned_polygon_count: 0,
            dangle_count: 0,
            cut_edge_count: 0,
            invalid_ring_count: 0,
            coverage_issues: Vec::new(),
            ownership_domain_issues: Vec::new(),
            input_boundary_issues: Vec::new(),
            excluded_component_issues: vec![TileExcludedComponentIssue {
                input_geometry_indices: component_indices.clone(),
                component_bbox: bbox,
                connection: TileComponentConnection::ExactEndpoint,
            }],
            retry_attempts: Vec::new(),
            retry_exhausted: false,
        };
        let component = InputComponent {
            input_geometry_indices: component_indices,
            bbox,
            connection: TileComponentConnection::ExactEndpoint,
        };

        assert!(matches!(
            tiled.try_component_fallback(&[Vec::new()], &[report], &[component]),
            Err(PolygonizeError::Cancelled { stage }) if stage == "tile_component_fallback"
        ));
    }

    #[test]
    fn component_fallback_observes_region_selection_cancellation() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometries = (0..=256)
            .map(|_| {
                Geometry::LineString(LineString::new(vec![
                    Coord { x: 0.0, y: 0.0 },
                    Coord { x: 1.0, y: 0.0 },
                ]))
            })
            .collect::<Vec<_>>();
        let token = CancellationToken::new();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            cancel_at_work_item: Some((token, 256)),
            ..Default::default()
        };
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(policy);
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let component_indices = vec![0, 1];
        let report = TileReport {
            tile_bbox: bbox,
            input_geometry_count: 0,
            polygon_count: 0,
            owned_polygon_count: 0,
            dangle_count: 0,
            cut_edge_count: 0,
            invalid_ring_count: 0,
            coverage_issues: Vec::new(),
            ownership_domain_issues: Vec::new(),
            input_boundary_issues: Vec::new(),
            excluded_component_issues: vec![TileExcludedComponentIssue {
                input_geometry_indices: component_indices.clone(),
                component_bbox: Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 1.0 }),
                connection: TileComponentConnection::ExactEndpoint,
            }],
            retry_attempts: Vec::new(),
            retry_exhausted: false,
        };
        let component = InputComponent {
            input_geometry_indices: component_indices,
            bbox: Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 1.0 }),
            connection: TileComponentConnection::ExactEndpoint,
        };

        assert!(matches!(
            tiled.try_component_fallback(&[Vec::new()], &[report], &[component]),
            Err(PolygonizeError::Cancelled { stage }) if stage == "tile_component_fallback"
        ));
    }

    #[test]
    fn fallback_merge_observes_cancellation_during_recovery_output() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let token = CancellationToken::new();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            cancel_at_work_item: Some((token, 256)),
            ..Default::default()
        };
        let tiled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(policy);
        let fallback_polygons = (0..=256)
            .map(|index| {
                let x = index as f64;
                Polygon3D::new(
                    vec![
                        Coord3D::new(x, 0.0, 0.0),
                        Coord3D::new(x + 1.0, 0.0, 0.0),
                        Coord3D::new(x + 1.0, 1.0, 0.0),
                        Coord3D::new(x, 1.0, 0.0),
                        Coord3D::new(x, 0.0, 0.0),
                    ],
                    vec![],
                    vec![],
                    vec![],
                )
            })
            .collect();

        assert!(matches!(
            tiled.merge_fallback_polygons(Vec::new(), &[], fallback_polygons),
            Err(PolygonizeError::Cancelled { stage }) if stage == "tile_fallback_merge"
        ));
    }

    #[test]
    fn tile_processing_observes_cancellation_before_empty_tile_return() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let token = CancellationToken::new();
        token.cancel();
        let tiled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        });

        assert!(matches!(
            tiled.process_tile(0, bbox, &[], 0.0, None),
            Err(PolygonizeError::Cancelled { stage }) if stage == "tile_processing"
        ));
    }

    #[test]
    fn tile_processing_observes_midflight_filter_cancellation() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let token = CancellationToken::new();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            cancel_at_work_item: Some((token, 256)),
            ..Default::default()
        };
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(policy);
        let geometries = (0..=256)
            .map(|_| {
                Geometry::LineString(LineString::new(vec![
                    Coord { x: 0.0, y: 0.0 },
                    Coord { x: 1.0, y: 0.0 },
                ]))
            })
            .collect::<Vec<_>>();
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }

        assert!(matches!(
            tiled.process_tile(0, bbox, &[], 0.0, None),
            Err(PolygonizeError::Cancelled { stage }) if stage == "tile_processing"
        ));
    }

    #[test]
    fn tile_processing_observes_partial_component_member_cancellation() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometry = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 2.0, y: 1.0 },
        ]));
        let token = CancellationToken::new();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            cancel_at_work_item: Some((token, 256)),
            ..Default::default()
        };
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_execution_policy(policy);
        tiled.add_geometry(&geometry);
        let component = InputComponent {
            input_geometry_indices: vec![0; 257],
            bbox,
            connection: TileComponentConnection::ExactEndpoint,
        };

        assert!(matches!(
            tiled.process_tile(0, bbox, &[component], 0.0, None),
            Err(PolygonizeError::Cancelled { stage }) if stage == "tile_processing"
        ));
    }

    #[test]
    fn component_fallback_keeps_recovered_output_deterministic() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 30.0 },
                Coord { x: -10.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in &geometries {
            untiled.add_borrowed_geometry(geometry);
            tiled.add_geometry(geometry);
        }

        let expected = untiled
            .polygonize()
            .unwrap()
            .polygons
            .into_iter()
            .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
            .collect::<Vec<_>>();
        let forward = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        let forward_keys = forward
            .polygons
            .iter()
            .map(crate::tiling::canonical_polygon_key)
            .collect::<Vec<_>>();
        assert_eq!(forward_keys, expected);
        assert!(forward.stitching_report.component_fallback_used);
        assert!(!forward.stitching_report.untiled_fallback_used);

        let mut reversed = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in geometries.iter().rev() {
            reversed.add_geometry(geometry);
        }
        let reversed = reversed
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        let reversed_keys = reversed
            .polygons
            .iter()
            .map(crate::tiling::canonical_polygon_key)
            .collect::<Vec<_>>();
        assert_eq!(reversed_keys, forward_keys);
        assert!(reversed.stitching_report.component_fallback_used);
        assert_eq!(reversed.stitching_report.output_polygon_count, 1);

        for (options, connection) in [
            (
                PolygonizerOptions {
                    node_input: true,
                    pre_snap_tolerance: 0.5,
                    ..Default::default()
                },
                TileComponentConnection::PreSnap,
            ),
            (
                PolygonizerOptions {
                    node_input: true,
                    precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
                    ..Default::default()
                },
                TileComponentConnection::FixedGrid,
            ),
        ] {
            let mut configured_untiled = Polygonizer::with_options(options.clone());
            let mut configured_tiled = TiledPolygonizer::new(bbox, 10.0)
                .with_buffer(2.0)
                .with_options(options)
                .with_component_fallback();
            for geometry in &geometries {
                configured_untiled.add_borrowed_geometry(geometry);
                configured_tiled.add_geometry(geometry);
            }
            let expected = configured_untiled
                .polygonize()
                .unwrap()
                .polygons
                .into_iter()
                .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
                .collect::<Vec<_>>();
            let result = configured_tiled
                .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
                .unwrap();
            let actual = result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>();
            assert_eq!(actual, expected);
            assert!(result.stitching_report.component_fallback_used);
            assert!(result.tile_reports.iter().all(|report| {
                report.excluded_component_issues.len() == 1
                    && report.excluded_component_issues[0].connection == connection
            }));
        }
    }

    #[test]
    fn documents_intersection_connected_component_excluded_from_every_halo() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -20.0, y: -10.0 },
                Coord { x: 40.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -20.0 },
                Coord { x: 30.0, y: 40.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 40.0, y: 30.0 },
                Coord { x: -20.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 40.0 },
                Coord { x: -10.0, y: -20.0 },
            ])),
        ];
        let mut untiled = Polygonizer::with_options(PolygonizerOptions {
            node_input: true,
            ..Default::default()
        });
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_buffer(2.0);
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        assert_eq!(untiled.polygonize().unwrap().polygons.len(), 1);
        let observed = tiled.polygonize().unwrap();
        assert!(observed.polygons.is_empty());
        assert_eq!(observed.stitching_report.unresolved_component_count, 4);
        assert!(observed.tile_reports.iter().all(|report| {
            report.excluded_component_issues.len() == 1
                && report.excluded_component_issues[0].connection
                    == TileComponentConnection::SegmentIntersection
        }));
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_excluded_segment_component")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].payload["tile_index"], 0);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3])
        );
        let bounded = tiled.polygonize_with_trace(TraceLevelV1::Full, 0).unwrap();
        assert!(bounded.trace.events.is_empty());
        assert!(bounded.trace.truncated);
        assert_eq!(
            bounded.result.stitching_report.unresolved_component_count,
            4
        );
        let error = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap_err();
        assert!(matches!(
            error,
            TiledPolygonizeError::CoverageIncomplete {
                unresolved_component_count: 4,
                ..
            }
        ));

        let mut unnoded = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(PolygonizerOptions::default());
        for boundary in &boundaries {
            unnoded.add_geometry(boundary);
        }
        assert_eq!(
            unnoded
                .polygonize()
                .unwrap()
                .stitching_report
                .unresolved_component_count,
            0
        );

        let mut limited = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_execution_policy(ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            });
        for boundary in &boundaries {
            limited.add_geometry(boundary);
        }
        assert!(matches!(
            limited.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                stage,
                limit: 0,
                observed: 1,
                }) if stage == "candidate_pairs"
        ));

        let mut split_limited = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(PolygonizerOptions {
                node_input: true,
                precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
                noding: NodingOptions {
                    guarantee: NodingGuarantee::CertifiedFixedPrecision,
                    ..Default::default()
                },
                ..Default::default()
            })
            .with_execution_policy(ExecutionPolicy {
                max_split_events: Some(0),
                ..Default::default()
            });
        for boundary in &boundaries {
            split_limited.add_geometry(boundary);
        }
        assert!(matches!(
            split_limited.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                stage,
                limit: 0,
                observed,
            }) if stage == "split_events" && observed > 0
        ));
    }

    #[test]
    fn documents_pre_snap_connected_region_excluded_from_every_halo() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.1, y: -10.1 },
                Coord { x: 30.1, y: 30.1 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.2, y: 30.2 },
                Coord { x: -10.2, y: 30.2 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.3, y: 30.3 },
                Coord { x: -10.3, y: -10.3 },
            ])),
        ];
        let options = PolygonizerOptions {
            node_input: true,
            pre_snap_tolerance: 0.5,
            ..Default::default()
        };
        let mut untiled = Polygonizer::with_options(options.clone());
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(options);
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        assert_eq!(untiled.polygonize().unwrap().polygons.len(), 1);
        assert_eq!(tiled.input_components().unwrap().len(), 1);
        let observed = tiled.polygonize().unwrap();
        assert!(observed.polygons.is_empty());
        assert!(observed
            .tile_reports
            .iter()
            .all(|report| report.input_geometry_count == 0));
        assert_eq!(observed.stitching_report.unresolved_input_geometry_count, 0);
        assert_eq!(observed.stitching_report.unresolved_component_tile_count, 4);
        assert_eq!(observed.stitching_report.unresolved_component_count, 4);
        assert!(observed.tile_reports.iter().all(|report| {
            report.excluded_component_issues.len() == 1
                && report.excluded_component_issues[0].connection
                    == TileComponentConnection::PreSnap
        }));
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_excluded_pre_snap_component")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].payload["tile_index"], 0);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3])
        );
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_component_tile_count: 4,
                unresolved_component_count: 4,
                ..
            })
        ));
    }

    #[test]
    fn documents_fixed_grid_connected_region_excluded_from_every_halo() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.4, y: -10.4 },
                Coord { x: 30.4, y: 30.4 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.3, y: 30.3 },
                Coord { x: -10.3, y: 30.3 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.4, y: 30.4 },
                Coord { x: -10.4, y: -10.4 },
            ])),
        ];
        let options = PolygonizerOptions {
            node_input: true,
            precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
            ..Default::default()
        };
        let mut untiled = Polygonizer::with_options(options.clone());
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(options);
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        assert_eq!(untiled.polygonize().unwrap().polygons.len(), 1);
        assert_eq!(tiled.input_components().unwrap().len(), 1);
        let observed = tiled.polygonize().unwrap();
        assert!(observed.polygons.is_empty());
        assert!(observed
            .tile_reports
            .iter()
            .all(|report| report.input_geometry_count == 0));
        assert_eq!(observed.stitching_report.unresolved_input_geometry_count, 0);
        assert_eq!(observed.stitching_report.unresolved_component_tile_count, 4);
        assert_eq!(observed.stitching_report.unresolved_component_count, 4);
        assert!(observed.tile_reports.iter().all(|report| {
            report.excluded_component_issues.len() == 1
                && report.excluded_component_issues[0].connection
                    == TileComponentConnection::FixedGrid
        }));
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_excluded_fixed_grid_component")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].payload["tile_index"], 0);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3])
        );
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_component_tile_count: 4,
                unresolved_component_count: 4,
                ..
            })
        ));
    }

    #[test]
    fn documents_certified_fixed_grid_hot_pixel_region_excluded_from_every_halo() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let options = PolygonizerOptions {
            node_input: true,
            precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
            noding: NodingOptions {
                guarantee: NodingGuarantee::CertifiedFixedPrecision,
                ..Default::default()
            },
            ..Default::default()
        };
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord {
                    x: -200.0,
                    y: -11.0,
                },
                Coord { x: 100.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 100.0, y: 30.0 },
                Coord { x: -200.0, y: 31.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
        ];
        let mut untiled = Polygonizer::with_options(options.clone());
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(options.clone());
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        assert_eq!(untiled.polygonize().unwrap().polygons.len(), 1);
        let components = tiled.input_components().unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].input_geometry_indices, vec![0, 1, 2, 3]);
        let observed = tiled.polygonize().unwrap();
        assert!(observed.polygons.is_empty());
        assert!(observed
            .tile_reports
            .iter()
            .all(|report| report.input_geometry_count == 0));
        assert_eq!(observed.stitching_report.unresolved_input_geometry_count, 0);
        assert_eq!(observed.stitching_report.unresolved_component_tile_count, 4);
        assert_eq!(observed.stitching_report.unresolved_component_count, 4);
        assert!(observed.tile_reports.iter().all(|report| {
            report.excluded_component_issues.len() == 1
                && report.excluded_component_issues[0].connection
                    == TileComponentConnection::FixedGrid
        }));
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_excluded_fixed_grid_component")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 4);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3])
        );
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_component_tile_count: 4,
                unresolved_component_count: 4,
                ..
            })
        ));

        let mut limited = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(options)
            .with_execution_policy(ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            });
        for boundary in &boundaries {
            limited.add_geometry(boundary);
        }
        assert!(matches!(
            limited.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                stage,
                limit: 0,
                observed: 1,
            }) if stage == "candidate_pairs"
        ));
    }

    #[test]
    fn documents_partially_observed_pre_snap_component_without_boundary_evidence() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let x_ranges = [
            (-10.0, -2.25),
            (-1.75, 7.75),
            (8.25, 11.75),
            (12.25, 17.75),
            (18.25, 21.75),
            (22.25, 30.0),
        ];
        let mut boundaries = Vec::new();
        for y in [5.0, 15.0] {
            for &(min_x, max_x) in &x_ranges {
                boundaries.push(Geometry::LineString(LineString::new(vec![
                    Coord { x: min_x, y },
                    Coord { x: max_x, y },
                ])));
            }
        }
        boundaries.extend([
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 5.0 },
                Coord { x: -10.0, y: 15.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 15.0 },
                Coord { x: 30.0, y: 5.0 },
            ])),
        ]);
        let options = PolygonizerOptions {
            node_input: true,
            pre_snap_tolerance: 1.0,
            ..Default::default()
        };
        let mut untiled = Polygonizer::with_options(options.clone());
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(options);
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        assert_eq!(untiled.polygonize().unwrap().polygons.len(), 1);
        assert_eq!(tiled.input_components().unwrap().len(), 1);
        let observed = tiled.polygonize().unwrap();
        assert!(observed.polygons.is_empty());
        assert!(observed
            .tile_reports
            .iter()
            .all(|report| report.input_boundary_issues.is_empty()));
        assert!(observed.tile_reports.iter().all(|report| {
            report.excluded_component_issues.len() == 1
                && report.excluded_component_issues[0].connection
                    == TileComponentConnection::PreSnap
        }));
        assert_eq!(observed.stitching_report.unresolved_input_geometry_count, 0);
        assert_eq!(observed.stitching_report.unresolved_component_count, 4);
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_component_tile_count: 4,
                unresolved_component_count: 4,
                ..
            })
        ));
    }

    #[test]
    fn component_fallback_recovers_partially_observed_pre_snap_component() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let x_ranges = [
            (-10.0, -2.25),
            (-1.75, 7.75),
            (8.25, 11.75),
            (12.25, 17.75),
            (18.25, 21.75),
            (22.25, 30.0),
        ];
        let mut boundaries = Vec::new();
        for y in [5.0, 15.0] {
            for &(min_x, max_x) in &x_ranges {
                boundaries.push(Geometry::LineString(LineString::new(vec![
                    Coord { x: min_x, y },
                    Coord { x: max_x, y },
                ])));
            }
        }
        boundaries.extend([
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 5.0 },
                Coord { x: -10.0, y: 15.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 15.0 },
                Coord { x: 30.0, y: 5.0 },
            ])),
        ]);
        let inner = Geometry::LineString(LineString::new(vec![
            Coord { x: 2.0, y: 2.0 },
            Coord { x: 4.0, y: 2.0 },
            Coord { x: 4.0, y: 4.0 },
            Coord { x: 2.0, y: 4.0 },
            Coord { x: 2.0, y: 2.0 },
        ]));
        let options = PolygonizerOptions {
            node_input: true,
            pre_snap_tolerance: 1.0,
            ..Default::default()
        };
        let mut untiled = Polygonizer::with_options(options.clone());
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
        }
        untiled.add_borrowed_geometry(&inner);
        let expected = untiled
            .polygonize()
            .unwrap()
            .polygons
            .into_iter()
            .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
            .collect::<Vec<_>>();

        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(options)
            .with_component_fallback();
        for boundary in &boundaries {
            tiled.add_geometry(boundary);
        }
        tiled.add_geometry(&inner);
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        let actual = result
            .polygons
            .iter()
            .map(crate::tiling::canonical_polygon_key)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        assert!(result.stitching_report.component_fallback_used);
        assert_eq!(result.stitching_report.retained_tile_polygon_count, 1);
        assert_eq!(
            result
                .stitching_report
                .component_fallback_replaced_polygon_count,
            1
        );
    }

    #[test]
    fn reports_partially_observed_fixed_grid_component() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 11.6, y: 5.0 },
                Coord { x: 11.8, y: 5.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 12.2, y: 5.0 },
                Coord { x: 15.0, y: 5.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 11.6, y: 15.0 },
                Coord { x: 11.8, y: 15.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 12.2, y: 15.0 },
                Coord { x: 15.0, y: 15.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 11.6, y: 5.0 },
                Coord { x: 11.6, y: 7.6 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 11.6, y: 8.4 },
                Coord { x: 11.6, y: 11.6 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 11.6, y: 12.4 },
                Coord { x: 11.6, y: 15.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 15.0, y: 5.0 },
                Coord { x: 15.0, y: 7.6 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 15.0, y: 8.4 },
                Coord { x: 15.0, y: 11.6 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 15.0, y: 12.4 },
                Coord { x: 15.0, y: 15.0 },
            ])),
        ];
        let options = PolygonizerOptions {
            node_input: true,
            precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
            ..Default::default()
        };
        let mut untiled = Polygonizer::with_options(options.clone());
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(options);
        for boundary in &boundaries {
            untiled.add_borrowed_geometry(boundary);
            tiled.add_geometry(boundary);
        }

        assert_eq!(untiled.polygonize().unwrap().polygons.len(), 1);
        let components = tiled.input_components().unwrap();
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].connection, TileComponentConnection::FixedGrid);
        let observed = tiled.polygonize().unwrap();
        assert!(observed.polygons.is_empty());
        assert!(observed
            .tile_reports
            .iter()
            .all(|report| report.input_boundary_issues.is_empty()));
        assert!(observed.tile_reports.iter().all(|report| {
            report.excluded_component_issues.len() == 1
                && report.excluded_component_issues[0].connection
                    == TileComponentConnection::FixedGrid
        }));
        assert_eq!(observed.stitching_report.unresolved_component_count, 4);
        assert!(matches!(
            tiled.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_component_tile_count: 4,
                unresolved_component_count: 4,
                ..
            })
        ));

        let certified_options = PolygonizerOptions {
            node_input: true,
            precision_model: PrecisionModel::FixedGrid { grid_size: 1.0 },
            noding: NodingOptions {
                guarantee: NodingGuarantee::CertifiedFixedPrecision,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut certified_untiled = Polygonizer::with_options(certified_options.clone());
        let mut certified_tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_options(certified_options);
        for boundary in &boundaries {
            certified_untiled.add_borrowed_geometry(boundary);
            certified_tiled.add_geometry(boundary);
        }
        assert_eq!(certified_untiled.polygonize().unwrap().polygons.len(), 1);
        let certified_components = certified_tiled.input_components().unwrap();
        assert_eq!(certified_components.len(), 1);
        assert_eq!(
            certified_components[0].connection,
            TileComponentConnection::FixedGrid
        );
        let certified_observed = certified_tiled.polygonize().unwrap();
        assert!(certified_observed.polygons.is_empty());
        assert!(certified_observed
            .tile_reports
            .iter()
            .all(|report| report.input_boundary_issues.is_empty()));
        assert!(certified_observed.tile_reports.iter().all(|report| {
            report.excluded_component_issues.len() == 1
                && report.excluded_component_issues[0].connection
                    == TileComponentConnection::FixedGrid
        }));
        assert_eq!(
            certified_observed
                .stitching_report
                .unresolved_component_count,
            4
        );
        let traced = certified_tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_excluded_fixed_grid_component")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].payload["tile_index"], 0);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([1, 3, 4, 5, 6, 7, 8, 9])
        );
    }

    #[test]
    fn bounded_halo_retry_resolves_an_excluded_component() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let boundaries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 30.0 },
                Coord { x: -10.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
        ];
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_retry_policy(TileRetryPolicy {
                max_attempts: 1,
                buffer_increment: 40.0,
                max_buffer: 42.0,
            });
        for boundary in &boundaries {
            tiled.add_geometry(boundary);
        }

        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(result.polygons.len(), 1);
        assert_eq!(result.stitching_report.retried_tile_count, 4);
        assert_eq!(result.stitching_report.retry_attempt_count, 4);
        assert_eq!(result.stitching_report.retry_exhausted_tile_count, 0);
        assert!(result.tile_reports.iter().all(|report| {
            report.retry_attempts.len() == 1
                && report.retry_attempts[0].buffer == 42.0
                && report.retry_attempts[0].resolved
        }));
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let retry_events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_halo_retry")
            .collect::<Vec<_>>();
        assert_eq!(retry_events.len(), 4);
        assert_eq!(retry_events[0].payload["tile_index"], 0);
        assert_eq!(retry_events[0].payload["attempt"], 1);
        assert_eq!(retry_events[0].payload["buffer"], 42.0);
        assert_eq!(retry_events[0].payload["resolved"], true);
        let bounded = tiled.polygonize_with_trace(TraceLevelV1::Full, 0).unwrap();
        assert!(bounded.trace.events.is_empty());
        assert!(bounded.trace.truncated);
        assert_eq!(bounded.result.stitching_report.retry_attempt_count, 4);

        let mut exhausted = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_retry_policy(TileRetryPolicy {
                max_attempts: 1,
                buffer_increment: 1.0,
                max_buffer: 3.0,
            });
        for boundary in &boundaries {
            exhausted.add_geometry(boundary);
        }
        assert!(matches!(
            exhausted.polygonize_with_coverage_guarantee(
                TileCoverageGuarantee::ValidateObservedCoverage
            ),
            Err(TiledPolygonizeError::CoverageIncomplete {
                retry_attempt_count: 4,
                retry_exhausted_tile_count: 4,
                tile_reports,
                ..
            }) if tile_reports.iter().all(|report| report.retry_exhausted)
        ));

        let mut budgeted = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_retry_policy(TileRetryPolicy {
                max_attempts: 2,
                buffer_increment: 1.0,
                max_buffer: 4.0,
            })
            .with_execution_policy(ExecutionPolicy {
                max_tile_retry_attempts: Some(1),
                ..Default::default()
            });
        for boundary in &boundaries {
            budgeted.add_geometry(boundary);
        }
        assert!(matches!(
            budgeted.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            }) if stage == "tile_retry_attempts"
        ));

        let mut total_bound = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_retry_policy(TileRetryPolicy {
                max_attempts: 1,
                buffer_increment: 1.0,
                max_buffer: 3.0,
            })
            .with_tile_execution_policy(TileExecutionPolicy {
                max_retry_attempts_total: Some(0),
                max_parallel_tiles: Some(1),
                ..Default::default()
            });
        for boundary in &boundaries {
            total_bound.add_geometry(boundary);
        }
        assert!(matches!(
            total_bound.polygonize(),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 0,
                observed: 1,
            }) if stage == "tile_retry_attempts_total"
        ));
    }

    #[test]
    fn component_fallback_replaces_nested_retained_region() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 30.0 },
                Coord { x: -10.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 2.0 },
                Coord { x: 4.0, y: 2.0 },
                Coord { x: 4.0, y: 4.0 },
                Coord { x: 2.0, y: 4.0 },
                Coord { x: 2.0, y: 2.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in &geometries {
            untiled.add_borrowed_geometry(geometry);
            tiled.add_geometry(geometry);
        }

        let expected = untiled
            .polygonize()
            .unwrap()
            .polygons
            .into_iter()
            .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
            .collect::<Vec<_>>();
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(result.polygons.len(), 2);
        assert!(result
            .polygons
            .iter()
            .any(|polygon| !polygon.interiors.is_empty()));
        assert!(result.stitching_report.component_fallback_used);
        assert!(!result.stitching_report.untiled_fallback_used);
        assert_eq!(result.stitching_report.component_fallback_count, 1);
        assert_eq!(result.stitching_report.component_fallback_polygon_count, 2);
        assert_eq!(
            result
                .stitching_report
                .component_fallback_replaced_polygon_count,
            1
        );

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_component_fallback")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3, 4])
        );
        assert_eq!(events[0].payload["output_polygon_count"], 2);
        assert_eq!(events[0].payload["retained_tile_polygon_count"], 1);
        assert_eq!(events[0].payload["replaced_retained_polygon_count"], 1);
        assert_eq!(events[0].payload["recovered_component_count"], 1);
    }

    #[test]
    fn component_fallback_groups_overlapping_excluded_components() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let mut geometries = Vec::new();
        for (min, max) in [(-10.0, 30.0), (-20.0, 40.0)] {
            geometries.extend([
                Geometry::LineString(LineString::new(vec![
                    Coord { x: min, y: min },
                    Coord { x: max, y: min },
                ])),
                Geometry::LineString(LineString::new(vec![
                    Coord { x: max, y: min },
                    Coord { x: max, y: max },
                ])),
                Geometry::LineString(LineString::new(vec![
                    Coord { x: max, y: max },
                    Coord { x: min, y: max },
                ])),
                Geometry::LineString(LineString::new(vec![
                    Coord { x: min, y: max },
                    Coord { x: min, y: min },
                ])),
            ]);
        }
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash)
            .with_component_fallback();
        for geometry in &geometries {
            untiled.add_borrowed_geometry(geometry);
            tiled.add_geometry(geometry);
        }

        let expected = untiled
            .polygonize()
            .unwrap()
            .polygons
            .into_iter()
            .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
            .collect::<Vec<_>>();
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(result.polygons.len(), 2);
        assert_eq!(result.stitching_report.component_fallback_count, 2);
        assert_eq!(result.stitching_report.component_fallback_polygon_count, 2);
        assert_eq!(
            result
                .stitching_report
                .component_fallback_replaced_polygon_count,
            0
        );

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_component_fallback")
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].payload["input_geometry_indices"],
            serde_json::json!([0, 1, 2, 3, 4, 5, 6, 7])
        );
        assert_eq!(events[0].payload["recovered_component_count"], 2);
    }

    #[test]
    fn untiled_fallback_preserves_global_containment() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: -10.0 },
                Coord { x: 30.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: -10.0 },
                Coord { x: 30.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 30.0, y: 30.0 },
                Coord { x: -10.0, y: 30.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: -10.0, y: 30.0 },
                Coord { x: -10.0, y: -10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 2.0 },
                Coord { x: 4.0, y: 2.0 },
                Coord { x: 4.0, y: 4.0 },
                Coord { x: 2.0, y: 4.0 },
                Coord { x: 2.0, y: 2.0 },
            ])),
        ];
        let mut untiled = Polygonizer::new();
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_retry_policy(TileRetryPolicy {
                max_attempts: 1,
                buffer_increment: 1.0,
                max_buffer: 3.0,
            })
            .with_untiled_fallback();
        for geometry in &geometries {
            untiled.add_borrowed_geometry(geometry);
            tiled.add_geometry(geometry);
        }

        let untiled = untiled.polygonize().unwrap();
        assert_eq!(untiled.polygons.len(), 2);
        assert!(untiled
            .polygons
            .iter()
            .any(|polygon| !polygon.interiors.is_empty()));
        let result = tiled
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .unwrap();
        assert_eq!(
            result
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>(),
            untiled
                .polygons
                .iter()
                .map(crate::tiling::canonical_polygon_key)
                .collect::<Vec<_>>()
        );
        assert!(result.stitching_report.untiled_fallback_attempted);
        assert!(result.stitching_report.untiled_fallback_authoritative);
        assert_eq!(
            result
                .stitching_report
                .untiled_fallback_output_polygon_count,
            2
        );
        assert!(result.stitching_report.untiled_fallback_used);
        assert_eq!(result.stitching_report.retry_exhausted_tile_count, 4);
        assert!(result
            .tile_reports
            .iter()
            .any(|report| !report.excluded_component_issues.is_empty()));
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let fallback_events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_untiled_fallback")
            .collect::<Vec<_>>();
        assert_eq!(fallback_events.len(), 1);
        assert_eq!(fallback_events[0].payload["input_geometry_count"], 5);
        assert_eq!(fallback_events[0].payload["output_polygon_count"], 2);
        let recovery_events = traced
            .trace
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event.kind.as_str(),
                    "tile_halo_retry" | "tile_untiled_fallback"
                )
            })
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>();
        assert_eq!(recovery_events.len(), 5);
        assert!(recovery_events[..4]
            .iter()
            .all(|kind| *kind == "tile_halo_retry"));
        assert_eq!(recovery_events[4], "tile_untiled_fallback");
        let bounded = tiled.polygonize_with_trace(TraceLevelV1::Full, 0).unwrap();
        assert!(bounded.trace.events.is_empty());
        assert!(bounded.trace.truncated);
        assert!(bounded.result.stitching_report.untiled_fallback_used);

        let mut limited = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_execution_policy(ExecutionPolicy {
                max_input_segments: Some(4),
                ..Default::default()
            })
            .with_untiled_fallback();
        for geometry in &geometries {
            limited.add_geometry(geometry);
        }
        let limited_error = limited.polygonize().unwrap_err();
        assert!(
            matches!(
                limited_error,
                PolygonizeError::ResourceLimitExceeded {
                    ref stage,
                    limit: 4,
                    observed: 5,
                } if stage == "input_segments"
            ),
            "{limited_error:?}"
        );
    }

    #[test]
    fn tiled_component_preflight_observes_midflight_cancellation() {
        let bbox = Rect::new(Coord { x: -2.0, y: -2.0 }, Coord { x: 2.0, y: 2.0 });
        let token = CancellationToken::new();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            cancel_at_work_item: Some((token, 256)),
            ..Default::default()
        };
        let lines = (0..24)
            .map(|index| {
                let angle = index as f64 * std::f64::consts::TAU / 24.0;
                Geometry::LineString(LineString::new(vec![
                    Coord { x: 0.0, y: 0.0 },
                    Coord {
                        x: angle.cos(),
                        y: angle.sin(),
                    },
                ]))
            })
            .collect::<Vec<_>>();
        let mut tiled = TiledPolygonizer::new(bbox, 2.0).with_execution_policy(policy);
        for line in &lines {
            tiled.add_geometry(line);
        }

        assert!(matches!(
            tiled.polygonize(),
            Err(PolygonizeError::Cancelled { stage }) if stage == "candidate_enumeration"
        ));
    }

    #[test]
    fn validated_owned_face_coverage_rejects_reported_halo_escape() {
        assert_eq!(
            TileCoverageGuarantee::default(),
            TileCoverageGuarantee::BestEffort
        );
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let face = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.0, y: 2.0 },
            Coord { x: 19.0, y: 2.0 },
            Coord { x: 19.0, y: 8.0 },
            Coord { x: 1.0, y: 8.0 },
            Coord { x: 1.0, y: 2.0 },
        ]));
        let mut tiler = TiledPolygonizer::new(bbox, 10.0).with_buffer(2.0);
        tiler.add_geometry(&face);

        assert!(tiler
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::BestEffort)
            .is_ok());
        let error = tiler
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateOwnedFaces)
            .unwrap_err();
        assert!(matches!(
            &error,
            TiledPolygonizeError::CoverageIncomplete {
                unresolved_tile_count: 1,
                unresolved_owned_polygon_count: 1,
                tile_reports,
                ..
            } if tile_reports.iter().any(|report| !report.coverage_issues.is_empty())
        ));

        let mut sufficient = TiledPolygonizer::new(bbox, 10.0).with_buffer(10.0);
        sufficient.add_geometry(&face);
        assert!(sufficient
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
            .is_ok());
    }

    #[test]
    fn permuted_tile_traversal_preserves_canonical_output() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let face = Geometry::LineString(LineString::new(vec![
            Coord { x: 2.0, y: 2.0 },
            Coord { x: 18.0, y: 2.0 },
            Coord { x: 18.0, y: 18.0 },
            Coord { x: 2.0, y: 18.0 },
            Coord { x: 2.0, y: 2.0 },
        ]));
        let mut tiler = TiledPolygonizer::new(bbox, 7.0)
            .with_buffer(20.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash);
        tiler.add_geometry(&face);

        let forward = tiler.polygonize().unwrap();
        let mut tiles = tiler.generate_tiles().unwrap();
        let mut state = 0x71_1e_u64;
        for upper in (1..tiles.len()).rev() {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            tiles.swap(upper, (state as usize) % (upper + 1));
        }
        let input_components = tiler.input_components().unwrap();
        let permuted = tiler
            .polygonize_tiles(tiles, &input_components, None)
            .unwrap();

        assert_eq!(permuted.polygons.len(), forward.polygons.len());
        assert_eq!(permuted.polygons[0].exterior, forward.polygons[0].exterior);
        assert_eq!(
            permuted.stitching_report.output_polygon_count,
            forward.stitching_report.output_polygon_count
        );
    }

    #[test]
    fn trace_records_physical_tile_ownership_and_dedup_decisions() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let square = Geometry::LineString(LineString::new(vec![
            Coord { x: 8.0, y: 1.0 },
            Coord { x: 12.0, y: 1.0 },
            Coord { x: 12.0, y: 9.0 },
            Coord { x: 8.0, y: 9.0 },
            Coord { x: 8.0, y: 1.0 },
        ]));
        let mut tiler = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(5.0)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash);
        tiler.add_geometry(&square);

        let traced = tiler
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let ownership: Vec<_> = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_ownership")
            .collect();
        assert_eq!(ownership.len(), 2);
        assert_eq!(ownership[0].payload["owned"], false);
        assert_eq!(ownership[1].payload["owned"], true);
        let dedup = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "tile_deduplication")
            .unwrap();
        assert_eq!(dedup.payload["retained"], true);
        assert_eq!(traced.result.polygons.len(), 1);
    }

    #[test]
    fn tile_trace_capture_stops_before_budgeted_growth() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 });
        let square = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 9.0, y: 1.0 },
            Coord { x: 9.0, y: 9.0 },
            Coord { x: 1.0, y: 9.0 },
            Coord { x: 1.0, y: 1.0 },
        ]));
        let mut tiler = TiledPolygonizer::new(bbox, 10.0);
        tiler.add_geometry(&square);
        let expected = tiler.polygonize().unwrap();

        let traced = tiler.polygonize_with_trace(TraceLevelV1::Full, 0).unwrap();

        assert_eq!(traced.result.polygons.len(), expected.polygons.len());
        assert_eq!(
            traced.result.polygons[0].exterior,
            expected.polygons[0].exterior
        );
        assert!(traced.trace.events.is_empty());
        assert!(traced.trace.truncated);
    }
}

#[test]
fn test_dedup_policy_canonical_ring_hash() {
    use crate::options::DedupPolicy;
    use crate::TiledPolygonizer;
    use geo::{Coord, Geometry, LineString, Rect};

    let geom1 = Geometry::LineString(LineString::new(vec![
        Coord { x: 1.0, y: 1.0 },
        Coord { x: 9.0, y: 1.0 },
        Coord { x: 9.0, y: 9.0 },
        Coord { x: 1.0, y: 9.0 },
        Coord { x: 1.0, y: 1.0 },
    ]));

    let geom2 = Geometry::LineString(LineString::new(vec![
        Coord { x: 9.0, y: 1.0 },
        Coord { x: 9.0, y: 9.0 },
        Coord { x: 1.0, y: 9.0 },
        Coord { x: 1.0, y: 1.0 },
        Coord { x: 9.0, y: 1.0 },
    ]));

    let mut t_keep = TiledPolygonizer::new(
        Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 }),
        10.0,
    )
    .with_dedup_policy(DedupPolicy::KeepAll);
    t_keep.add_geometry(&geom1);
    t_keep.add_geometry(&geom2);
    let polys_keep = t_keep.polygonize().unwrap().polygons;
    assert_eq!(polys_keep.len(), 1);

    let mut t_dedup = TiledPolygonizer::new(
        Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 }),
        10.0,
    )
    .with_dedup_policy(DedupPolicy::CanonicalRingHash);
    t_dedup.add_geometry(&geom1);
    t_dedup.add_geometry(&geom2);
    let polys_dedup = t_dedup.polygonize().unwrap().polygons;
    assert_eq!(polys_dedup.len(), 1);
}

#[test]
fn canonical_dedup_key_compares_exact_geometry() {
    use super::canonical_polygon_key;
    use crate::{Coord3D, Polygon3D};

    let polygon = |max_x| {
        Polygon3D::new(
            vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(max_x, 0.0, 0.0),
                Coord3D::new(max_x, 1.0, 0.0),
                Coord3D::new(0.0, 1.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
            ],
            vec![],
            vec![],
            vec![],
        )
    };

    let equivalent = Polygon3D::new(
        vec![
            Coord3D::new(1.0, 1.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 1.0, 0.0),
            Coord3D::new(1.0, 1.0, 0.0),
        ],
        vec![],
        vec![],
        vec![],
    );
    let key = canonical_polygon_key(&polygon(1.0));

    assert_eq!(key, canonical_polygon_key(&equivalent));
    assert_ne!(key, canonical_polygon_key(&polygon(2.0)));
}

#[test]
fn canonical_dedup_merges_duplicate_provenance() {
    use super::merge_duplicate_polygon_provenance;
    use crate::types::PolygonProvenance;
    use crate::{Coord3D, Polygon3D};

    let exterior = vec![
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(1.0, 0.0, 0.0),
        Coord3D::new(1.0, 1.0, 0.0),
        Coord3D::new(0.0, 1.0, 0.0),
        Coord3D::new(0.0, 0.0, 0.0),
    ];
    let mut retained = Polygon3D::new(exterior.clone(), vec![], vec![10, 11], vec![]);
    retained.set_boundary_source_line_ids(vec![3, 1]);
    retained.provenance = Some(PolygonProvenance {
        boundary_line_ids: vec![3, 1],
        input_profile_id: Some("profile-a".to_string()),
    });
    let mut duplicate = Polygon3D::new(exterior, vec![], vec![20, 21], vec![]);
    duplicate.set_boundary_source_line_ids(vec![2, 3]);
    duplicate.provenance = Some(PolygonProvenance {
        boundary_line_ids: vec![4, 2],
        input_profile_id: Some("profile-b".to_string()),
    });

    merge_duplicate_polygon_provenance(&mut retained, &duplicate, false);

    assert_eq!(retained.boundary_source_line_ids, vec![1, 2, 3]);
    let provenance = retained.provenance.unwrap();
    assert_eq!(provenance.boundary_line_ids, vec![1, 2, 3, 4]);
    assert_eq!(provenance.input_profile_id, None);
    assert_eq!(retained.exterior_ids, vec![10, 11]);
}

#[test]
fn reports_single_geometry_face_outside_ownership_domain() {
    use crate::{
        Polygonizer, TileCoverageGuarantee, TileRetryPolicy, TiledPolygonizeError, TiledPolygonizer,
    };
    use geo::{Coord, Geometry, LineString, Rect};

    let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 32.0, y: 32.0 });
    let boundary = Geometry::LineString(LineString::new(vec![
        Coord { x: 25.0, y: 17.0 },
        Coord { x: 45.0, y: 17.0 },
        Coord { x: 45.0, y: 41.0 },
        Coord { x: 25.0, y: 41.0 },
        Coord { x: 25.0, y: 17.0 },
    ]));
    let mut untiled = Polygonizer::new();
    untiled.add_borrowed_geometry(&boundary);
    let mut tiled = TiledPolygonizer::new(bbox, 16.0).with_buffer(0.0);
    tiled.add_geometry(&boundary);

    let expected = untiled.polygonize().unwrap();
    assert_eq!(expected.polygons.len(), 1);
    assert!(tiled.input_components().unwrap().is_empty());
    let observed = tiled.polygonize().unwrap();
    assert!(observed.polygons.is_empty());
    assert!(observed
        .tile_reports
        .iter()
        .any(|report| report.input_geometry_count == 1));
    assert!(observed.tile_reports.iter().all(|report| {
        report.coverage_issues.is_empty()
            && report.input_boundary_issues.is_empty()
            && report.excluded_component_issues.is_empty()
    }));
    let ownership_domain_issues = observed
        .tile_reports
        .iter()
        .flat_map(|report| &report.ownership_domain_issues)
        .collect::<Vec<_>>();
    assert_eq!(ownership_domain_issues.len(), 1);
    assert_eq!(
        ownership_domain_issues[0].ownership_point,
        crate::Coord3D::new(35.0, 29.0, 0.0)
    );
    assert_eq!(observed.stitching_report.unresolved_input_geometry_count, 0);
    assert_eq!(observed.stitching_report.unresolved_component_count, 0);
    assert_eq!(
        observed
            .stitching_report
            .unresolved_ownership_domain_tile_count,
        1
    );
    assert_eq!(
        observed.stitching_report.unresolved_ownership_domain_count,
        1
    );
    assert_eq!(observed.stitching_report.retry_attempt_count, 0);
    assert!(matches!(
        tiled.polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage),
        Err(TiledPolygonizeError::CoverageIncomplete {
            unresolved_tile_count: 0,
            unresolved_ownership_domain_tile_count: 1,
            unresolved_ownership_domain_count: 1,
            unresolved_input_geometry_count: 0,
            unresolved_component_count: 0,
            ..
        })
    ));

    let mut fallback = TiledPolygonizer::new(bbox, 16.0)
        .with_buffer(0.0)
        .with_retry_policy(TileRetryPolicy {
            max_attempts: 3,
            buffer_increment: 4.0,
            max_buffer: 12.0,
        })
        .with_untiled_fallback();
    fallback.add_geometry(&boundary);
    let fallback_result = fallback
        .polygonize_with_coverage_guarantee(crate::TileCoverageGuarantee::ValidateObservedCoverage)
        .unwrap();
    assert_eq!(
        fallback_result
            .polygons
            .iter()
            .map(crate::tiling::canonical_polygon_key)
            .collect::<Vec<_>>(),
        expected
            .polygons
            .iter()
            .map(crate::tiling::canonical_polygon_key)
            .collect::<Vec<_>>()
    );
    assert!(fallback_result.stitching_report.untiled_fallback_used);
    assert_eq!(fallback_result.stitching_report.retry_attempt_count, 0);
    let traced = fallback
        .polygonize_with_trace(crate::trace::TraceLevelV1::Full, usize::MAX)
        .unwrap();
    assert_eq!(
        traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "tile_ownership_domain")
            .count(),
        1
    );
    let fallback_event = traced
        .trace
        .events
        .iter()
        .find(|event| event.kind == "tile_untiled_fallback")
        .expect("ownership-domain evidence triggers global fallback");
    assert_eq!(
        fallback_event.payload["unresolved_ownership_domain_count"],
        1
    );
}

#[test]
fn ownership_domain_evidence_follows_policy_and_input_order() {
    use crate::{DedupPolicy, TileOwnershipPolicy, TiledPolygonizer};
    use geo::{Coord, Geometry, LineString, Rect};

    let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 32.0, y: 32.0 });
    let outer = Geometry::LineString(LineString::new(vec![
        Coord { x: 25.0, y: 17.0 },
        Coord { x: 45.0, y: 17.0 },
        Coord { x: 45.0, y: 41.0 },
        Coord { x: 25.0, y: 41.0 },
        Coord { x: 25.0, y: 17.0 },
    ]));
    let inner = Geometry::LineString(LineString::new(vec![
        Coord { x: 2.0, y: 2.0 },
        Coord { x: 4.0, y: 2.0 },
        Coord { x: 4.0, y: 4.0 },
        Coord { x: 2.0, y: 4.0 },
        Coord { x: 2.0, y: 2.0 },
    ]));

    for (policy, expected_polygon_count, expected_issue_count) in [
        (TileOwnershipPolicy::Centroid, 1usize, 1usize),
        (TileOwnershipPolicy::RepresentativePointInsidePolygon, 1, 1),
        (TileOwnershipPolicy::LexicographicMinVertex, 2, 0),
    ] {
        let mut outputs = Vec::new();
        for reverse in [false, true] {
            let inputs = if reverse {
                [&inner, &outer]
            } else {
                [&outer, &inner]
            };
            let mut tiled = TiledPolygonizer::new(bbox, 16.0)
                .with_buffer(0.0)
                .with_ownership_policy(policy.clone())
                .with_dedup_policy(DedupPolicy::CanonicalRingHash);
            for geometry in inputs {
                tiled.add_geometry(geometry);
            }

            let result = tiled.polygonize().unwrap();
            assert_eq!(result.polygons.len(), expected_polygon_count);
            assert_eq!(
                result
                    .tile_reports
                    .iter()
                    .map(|report| report.ownership_domain_issues.len())
                    .sum::<usize>(),
                expected_issue_count
            );
            outputs.push(
                result
                    .polygons
                    .iter()
                    .map(crate::tiling::canonical_polygon_key)
                    .collect::<Vec<_>>(),
            );
        }
        assert_eq!(outputs[0], outputs[1], "policy {policy:?}");
    }
}

#[test]
fn ownership_domain_evidence_declines_component_fallback_without_indexed_component() {
    use crate::{TileCoverageGuarantee, TiledPolygonizeError, TiledPolygonizer};
    use geo::{Coord, Geometry, LineString, Rect};

    let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 32.0, y: 32.0 });
    let boundary = Geometry::LineString(LineString::new(vec![
        Coord { x: 25.0, y: 17.0 },
        Coord { x: 45.0, y: 17.0 },
        Coord { x: 45.0, y: 41.0 },
        Coord { x: 25.0, y: 41.0 },
        Coord { x: 25.0, y: 17.0 },
    ]));
    let mut tiled = TiledPolygonizer::new(bbox, 16.0)
        .with_buffer(0.0)
        .with_component_fallback();
    tiled.add_geometry(&boundary);

    let observed = tiled.polygonize().unwrap();
    assert!(observed.polygons.is_empty());
    assert!(observed.stitching_report.component_fallback_attempted);
    assert!(!observed.stitching_report.component_fallback_used);
    assert_eq!(
        observed.stitching_report.component_fallback_decline_reason,
        Some("no_indexed_component_evidence")
    );
    assert_eq!(
        observed.stitching_report.unresolved_ownership_domain_count,
        1
    );
    assert!(!observed.stitching_report.untiled_fallback_used);

    assert!(matches!(
        tiled.polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage),
        Err(TiledPolygonizeError::CoverageIncomplete {
            component_fallback_decline_reason: Some("no_indexed_component_evidence"),
            unresolved_ownership_domain_count: 1,
            ..
        })
    ));
    let traced = tiled
        .polygonize_with_trace(crate::trace::TraceLevelV1::Full, usize::MAX)
        .unwrap();
    let declined = traced
        .trace
        .events
        .iter()
        .find(|event| event.kind == "tile_component_fallback_declined")
        .expect("component fallback decline is traced");
    assert_eq!(declined.payload["reason"], "no_indexed_component_evidence");
    assert_eq!(declined.payload["unresolved_ownership_domain_count"], 1);
    assert!(!traced
        .trace
        .events
        .iter()
        .any(|event| event.kind == "tile_component_fallback"));
}

#[test]
fn component_fallback_recovers_mixed_owned_face_and_component_evidence() {
    use crate::{DedupPolicy, Polygonizer, TileCoverageGuarantee, TiledPolygonizer};
    use geo::{Coord, Geometry, LineString, Rect};

    let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
    let geometries = [
        Geometry::LineString(LineString::new(vec![
            Coord { x: 4.0, y: 4.0 },
            Coord { x: 8.0, y: 4.0 },
        ])),
        Geometry::LineString(LineString::new(vec![
            Coord { x: 8.0, y: 4.0 },
            Coord { x: 8.0, y: 12.0 },
        ])),
        Geometry::LineString(LineString::new(vec![
            Coord { x: 8.0, y: 12.0 },
            Coord { x: 4.0, y: 12.0 },
        ])),
        Geometry::LineString(LineString::new(vec![
            Coord { x: 4.0, y: 12.0 },
            Coord { x: 4.0, y: 4.0 },
        ])),
    ];
    let mut untiled = Polygonizer::new();
    let mut tiled = TiledPolygonizer::new(bbox, 10.0)
        .with_buffer(2.0)
        .with_dedup_policy(DedupPolicy::CanonicalRingHash)
        .with_component_fallback();
    for geometry in &geometries {
        untiled.add_borrowed_geometry(geometry);
        tiled.add_geometry(geometry);
    }

    let expected = untiled
        .polygonize()
        .unwrap()
        .polygons
        .into_iter()
        .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
        .collect::<Vec<_>>();
    let result = tiled
        .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
        .unwrap();
    assert_eq!(
        result
            .polygons
            .iter()
            .map(crate::tiling::canonical_polygon_key)
            .collect::<Vec<_>>(),
        expected
    );
    assert!(result.stitching_report.component_fallback_used);
    assert!(!result.stitching_report.untiled_fallback_used);
    assert_eq!(result.stitching_report.component_fallback_count, 1);
    assert_eq!(result.stitching_report.component_fallback_polygon_count, 1);
    assert_eq!(
        result
            .stitching_report
            .component_fallback_replaced_polygon_count,
        1
    );
    assert!(result.stitching_report.unresolved_owned_polygon_count > 0);
    assert!(result.stitching_report.unresolved_input_geometry_count > 0);
    let traced = tiled
        .polygonize_with_trace(crate::trace::TraceLevelV1::Full, usize::MAX)
        .unwrap();
    let fallback_events = traced
        .trace
        .events
        .iter()
        .filter(|event| event.kind == "tile_component_fallback")
        .collect::<Vec<_>>();
    assert_eq!(fallback_events.len(), 1);
    assert_eq!(
        fallback_events[0].payload["input_geometry_indices"],
        serde_json::json!([0, 1, 2, 3])
    );
    assert_eq!(fallback_events[0].payload["output_polygon_count"], 1);
    assert_eq!(
        fallback_events[0].payload["replaced_retained_polygon_count"],
        1
    );
    assert!(!traced
        .trace
        .events
        .iter()
        .any(|event| event.kind == "tile_component_fallback_declined"));
}

#[test]
fn component_fallback_declines_unclosed_coverage_region_outside_component() {
    use crate::{
        DedupPolicy, Polygonizer, TileCoverageGuarantee, TiledPolygonizeError, TiledPolygonizer,
    };
    use geo::{Coord, Geometry, LineString, Rect};

    let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 60.0, y: 60.0 });
    let geometries = [
        Geometry::LineString(LineString::new(vec![
            Coord { x: -10.0, y: -10.0 },
            Coord { x: 30.0, y: -10.0 },
        ])),
        Geometry::LineString(LineString::new(vec![
            Coord { x: 30.0, y: -10.0 },
            Coord { x: 30.0, y: 30.0 },
        ])),
        Geometry::LineString(LineString::new(vec![
            Coord { x: 30.0, y: 30.0 },
            Coord { x: -10.0, y: 30.0 },
        ])),
        Geometry::LineString(LineString::new(vec![
            Coord { x: -10.0, y: 30.0 },
            Coord { x: -10.0, y: -10.0 },
        ])),
        Geometry::LineString(LineString::new(vec![
            Coord { x: 44.0, y: 44.0 },
            Coord { x: 48.0, y: 44.0 },
            Coord { x: 48.0, y: 52.0 },
            Coord { x: 44.0, y: 52.0 },
            Coord { x: 44.0, y: 44.0 },
        ])),
    ];
    let mut tiled = TiledPolygonizer::new(bbox, 10.0)
        .with_buffer(2.0)
        .with_dedup_policy(DedupPolicy::CanonicalRingHash)
        .with_component_fallback();
    for geometry in &geometries {
        tiled.add_geometry(geometry);
    }

    let result =
        tiled.polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage);
    assert!(matches!(
        result,
        Err(TiledPolygonizeError::CoverageIncomplete {
            component_fallback_decline_reason: Some("input_boundary_outside_recovery_region"),
            ..
        })
    ));

    let mut expected = Polygonizer::new();
    for geometry in &geometries {
        expected.add_borrowed_geometry(geometry);
    }
    let expected = expected
        .polygonize()
        .unwrap()
        .polygons
        .into_iter()
        .map(|polygon| crate::tiling::canonical_polygon_key(&polygon))
        .collect::<Vec<_>>();
    let mut globally_fallback = TiledPolygonizer::new(bbox, 10.0)
        .with_buffer(2.0)
        .with_dedup_policy(DedupPolicy::CanonicalRingHash)
        .with_component_fallback()
        .with_untiled_fallback();
    for geometry in &geometries {
        globally_fallback.add_geometry(geometry);
    }
    let result = globally_fallback
        .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
        .unwrap();
    assert_eq!(
        result
            .polygons
            .iter()
            .map(crate::tiling::canonical_polygon_key)
            .collect::<Vec<_>>(),
        expected
    );
    assert!(result.stitching_report.component_fallback_attempted);
    assert!(!result.stitching_report.component_fallback_used);
    assert_eq!(
        result.stitching_report.component_fallback_decline_reason,
        Some("input_boundary_outside_recovery_region")
    );
    assert!(result.stitching_report.untiled_fallback_used);
    let traced = globally_fallback
        .polygonize_with_trace(crate::trace::TraceLevelV1::Full, usize::MAX)
        .unwrap();
    let recovery_events = traced
        .trace
        .events
        .iter()
        .filter_map(|event| match event.kind.as_str() {
            "tile_component_fallback_declined" | "tile_untiled_fallback" => {
                Some(event.kind.as_str())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        recovery_events,
        vec!["tile_component_fallback_declined", "tile_untiled_fallback"]
    );
}
