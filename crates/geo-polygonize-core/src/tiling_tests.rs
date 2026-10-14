#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::tiling::{
        ComponentFallbackDecision, ComponentFallbackDeclineReason, InputComponent,
        PartitionDeclaredAdjacencyV1,
    };
    use crate::{
        trace::TraceLevelV1, CancellationToken, Coord3D, DedupPolicy, ExecutionPolicy, Line3D,
        NodingGuarantee, NodingOptions, Polygon3D, PolygonizeError, Polygonizer,
        PolygonizerOptions, PrecisionModel, ProvenanceOptions, TileBoundarySide,
        TileComponentConnection, TileCoverageGuarantee, TileCoverageIssue,
        TileCoverageResolutionKind, TileExcludedComponentIssue, TileExecutionPolicy, TileReport,
        TileRetryPolicy, TiledPolygonizeError, TiledPolygonizer, TiledStitchedOutput, ZOptions,
    };
    use geo::{
        BoundingRect, Contains, Coord, Geometry, LineString, MapCoords, MultiLineString, Rect,
    };
    use std::collections::BTreeSet;

    type ProofCoordinateMap = fn(Coord<f64>) -> Coord<f64>;
    const PHYSICAL_PROOF_REPRESENTATIONS: [(&str, ProofCoordinateMap); 5] = [
        ("identity", |point| point),
        ("vertical", |Coord { x, y }| Coord { x: y, y: x }),
        ("negative", |Coord { x, y }| Coord {
            x: x - 12.0,
            y: y - 4.0,
        }),
        ("reflected", |Coord { x, y }| Coord { x: -x, y }),
        ("large", |Coord { x, y }| Coord {
            x: x + 1_000_000.0,
            y: y - 1_000_000.0,
        }),
    ];

    #[test]
    fn coverage_global_arrangement_has_one_exterior_without_promoting_ambiguous_claims() {
        let mut geometries = Vec::new();
        for axis in 0..=2 {
            for segment in 0..2 {
                let (a, b) = (axis as f64, segment as f64);
                for coords in [vec![(a, b), (a, b + 1.0)], vec![(b, a), (b + 1.0, a)]] {
                    geometries.push(Geometry::LineString(LineString::from(coords)));
                }
            }
        }
        let mut signatures = Vec::new();
        for reverse in [false, true] {
            if reverse {
                geometries.reverse();
                for geometry in &mut geometries {
                    if let Geometry::LineString(line) = geometry {
                        line.0.reverse();
                    }
                }
            }
            let mut tiled = TiledPolygonizer::new(
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 2.0 }),
                1.0,
            )
            .with_buffer(0.0)
            .with_untiled_equivalence_check();
            for geometry in &geometries {
                tiled.add_geometry(geometry);
            }
            let result = tiled
                .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                .unwrap();
            let report = &result.result.stitching_report;
            assert_eq!(report.partition_border_global_arrangement_face_count, 5);
            assert_eq!(
                report.partition_border_global_arrangement_alias_slot_count,
                24
            );
            assert!(!report.partition_border_global_arrangement_adopted);
            assert!(report.partition_border_global_arrangement_blocked_span_count > 0);
            assert_eq!(
                report.partition_border_global_arrangement_alias_edge_count,
                8
            );
            assert_eq!(
                report.partition_border_global_arrangement_unbounded_face_count,
                1
            );
            assert_eq!(
                report.partition_border_global_arrangement_mapped_edge_count,
                32
            );
            assert!(!report.partition_border_global_stitched_output_ready);
            assert!(!report.partition_border_global_untiled_equivalence_checked);
            assert!(!report.partition_border_global_untiled_equivalence_ready);
            assert!(result.result.stitched_output.is_none());
            let binding = &result
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_input_bound_alias_view")
                .unwrap()
                .payload;
            assert_eq!(binding["prepared"], true);
            assert_eq!(binding["bound"], true);
            assert_eq!(binding["output_selectable"], false);
            assert_eq!(binding["view"]["source_segment_count"], 12);
            assert_eq!(binding["view"]["noded_segment_count"], 12);
            assert_eq!(binding["view"]["physical_slot_count"], 24);
            assert_eq!(binding["view"]["active_alias_count"], 32);
            assert_eq!(binding["view"]["raw_record_count"], 16);
            assert_eq!(binding["view"]["raw_coordinate_count"], 32);
            assert_eq!(binding["view"]["raw_segment_alias_count"], 16);
            let claims = &result
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_claims")
                .unwrap()
                .payload["resolutions"];
            let claims = claims.as_array().unwrap();
            assert_eq!(claims.len(), 12);
            assert_eq!(
                claims
                    .iter()
                    .filter(|claim| claim["status"] == "ambiguous"
                        && claim["reason"] == "observation_multiplicity")
                    .count(),
                4
            );
            assert_eq!(
                claims
                    .iter()
                    .filter(|claim| claim["status"] == "incomplete"
                        && claim["reason"] == "missing_corroboration")
                    .count(),
                8
            );
            assert!(claims
                .iter()
                .all(|claim| claim["status"] != "ready" && claim["face_pair"].is_null()));
            let mut retained_claims = claims
                .iter()
                .flat_map(|claim| claim["local_dir_edge_ids"].as_array().unwrap())
                .map(|id| id.as_u64().unwrap())
                .collect::<Vec<_>>();
            retained_claims.sort_unstable();
            assert_eq!(retained_claims, (0..32).collect::<Vec<_>>());
            let candidate = &result
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_candidate")
                .unwrap()
                .payload;
            assert_eq!(candidate["constructed"], false);
            assert!(candidate["candidate"].is_null());
            assert_eq!(
                candidate["blocked_span_count"],
                report.partition_border_global_arrangement_blocked_span_count
            );
            let faces = &result
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_faces")
                .unwrap()
                .payload;
            assert_eq!(faces["reconciled"], false);
            assert!(faces["faces"].is_null());
            assert!(faces["components"].is_null());
            let extraction = &result
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_extraction")
                .unwrap()
                .payload;
            assert_eq!(extraction["materialized"], false);
            assert!(extraction["rings"].is_null());
            assert!(extraction["local_aliases"].is_null());
            let conformance = &result
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_conformance")
                .unwrap()
                .payload;
            assert_eq!(conformance["candidate_built"], false);
            assert_eq!(conformance["equivalence_checked"], false);
            assert_eq!(conformance["equivalence_ready"], false);
            assert_eq!(conformance["output_selectable"], false);
            let adoption = result
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_adoption")
                .unwrap();
            let aliases = adoption.payload["local_edges_by_physical_edge"]
                .as_array()
                .unwrap();
            let mut retained = aliases
                .iter()
                .flat_map(|group| group.as_array().unwrap())
                .map(|id| id.as_u64().unwrap())
                .collect::<Vec<_>>();
            retained.sort_unstable();
            assert_eq!(retained, (0..32).collect::<Vec<_>>());
            assert_eq!(
                aliases
                    .iter()
                    .filter(|group| group.as_array().unwrap().len() == 2)
                    .count(),
                8
            );
            let witness = &result
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_witness")
                .unwrap()
                .payload;
            assert_eq!(witness["node_count"], 9);
            assert_eq!(witness["physical_edge_count"], 12);
            let exterior = &witness["unbounded_face_ids"][0];
            assert_eq!(
                witness["face_ids_by_local_edge"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter(|id| *id == exterior)
                    .count(),
                8
            );
            assert_eq!(
                witness["face_ids_by_direction"].as_array().unwrap().len(),
                24
            );
            signatures.push((
                witness["unbounded_face_ids"].clone(),
                witness["face_ids_by_direction"].clone(),
            ));
        }
        assert_eq!(signatures[0], signatures[1]);
    }

    #[test]
    fn input_bound_alias_view_retains_complete_grid_and_rejects_tampering() {
        use crate::graph::partition_border::{PartitionBorderEdgeKey, PartitionBorderGraph};
        use crate::tiling::{
            classify_non_polygon_evidence, coord3d_from_bits, input_bound_physical_alias_view,
            partition_local_face_graphs, prepare_partition_input_evidence, PartitionMosaic,
        };
        let mut geometries = Vec::new();
        for axis in 0..=2 {
            for segment in 0..2 {
                let (a, b) = (axis as f64, segment as f64);
                for coords in [vec![(a, b), (a, b + 1.0)], vec![(b, a), (b + 1.0, a)]] {
                    geometries.push(Geometry::LineString(LineString::from(coords)));
                }
            }
        }
        let refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 2.0 });
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 1.0).with_buffer(0.0);
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let tiles = tiled.generate_tiles().unwrap();
        let mut observations = Vec::new();
        let mut locals = Vec::new();
        let mut mosaic = PartitionMosaic::default();
        // Reverse insertion must not change partition-qualified evidence.
        for (partition, &tile) in tiles.iter().enumerate().rev() {
            let (_, _, _, edges, local, _, _, _, _, _, _, _, snapshot) = tiled
                .process_tile(partition, tile, &tiles, &[], 0.0, 0, None)
                .unwrap();
            observations.extend(edges);
            locals.extend(local);
            mosaic.replace_partition(snapshot, &policy).unwrap();
        }
        let rebuild =
            |locals: Vec<crate::graph::partition_border::PartitionBorderLocalFaceGraph>,
             mosaic: &PartitionMosaic| {
                let mut graph = PartitionBorderGraph::default();
                for mut observation in observations.clone() {
                    let Some(native) = locals
                        .iter()
                        .find(|local| {
                            (local.partition_id, local.component_id)
                                == (observation.partition_id, observation.component_id)
                        })
                        .and_then(|local| {
                            local.directed_edges.iter().find(|edge| {
                                edge.local_dir_edge_id == observation.local_dir_edge_id
                            })
                        })
                    else {
                        continue;
                    };
                    observation.from = native.from;
                    observation.to = native.to;
                    observation.from_z_bits = native.from_z_bits;
                    observation.to_z_bits = native.to_z_bits;
                    observation.edge_key = native.edge_key;
                    observation.source_line_ids = native.source_line_ids.clone();
                    observation.representative_line_id = native.source_line_ids.first().copied();
                    observation.face_ref = native.face_ref;
                    observation.face_id = native.face_ref.map(|face| face.face_id);
                    observation.local_face_successor = native.local_face_successor;
                    observation.local_face_is_unbounded = native.local_face_is_unbounded;
                    graph.insert(observation).unwrap();
                }
                for local in locals {
                    graph.insert_local_face_graph(local).unwrap();
                }
                graph.reconcile_global_face_edge_map(&policy).unwrap();
                graph
                    .reconcile_global_face_nodes(options.z, &policy)
                    .unwrap();
                let decode = |family: &[Vec<[u64; 3]>]| {
                    family
                        .iter()
                        .map(|bits| bits.iter().copied().map(coord3d_from_bits).collect())
                        .collect()
                };
                for snapshot in mosaic.partitions.values() {
                    graph
                        .retain_global_non_polygon_extraction_payloads(
                            &policy,
                            snapshot.partition_id,
                            decode(&snapshot.non_polygon.dangles),
                            decode(&snapshot.non_polygon.cut_edges),
                            decode(&snapshot.non_polygon.invalid_rings),
                        )
                        .unwrap();
                }
                graph
                    .materialize_global_non_polygon_extraction_payloads(&policy)
                    .unwrap();
                graph
            };
        let graph = rebuild(locals.clone(), &mosaic);
        let witness = graph.global_arrangement_witness(&policy).unwrap().unwrap();
        let stats = graph
            .clone()
            .materialize_global_non_polygon_extraction_payloads(&policy)
            .unwrap();
        assert!(!stats.payload_ready);
        assert_eq!(stats.duplicate_payload_count, 4);
        let prepare = |mosaic: &PartitionMosaic| {
            prepare_partition_input_evidence(mosaic, &tiles, &refs, domain, &options, &policy)
                .unwrap()
                .unwrap()
        };
        let prepared = prepare(&mosaic);
        assert_eq!(prepared.reference.polygons.len(), 4);
        assert!(prepared.reference.dangles.is_empty());
        let before = mosaic.fingerprint_sha256();
        let graph_before = format!("{graph:?}");
        for _ in 0..2 {
            let view = input_bound_physical_alias_view(
                &prepared, &graph, &witness, &mosaic, &options, stats, &policy,
            )
            .unwrap()
            .unwrap();
            assert_eq!(view.prepared.source_segments.len(), 12);
            assert_eq!(view.slots.len(), 24);
            assert_eq!(view.active_aliases.len(), 32);
            assert_eq!(view.raw_payloads.len(), 16);
            let mut aliases = BTreeSet::new();
            for bound in &view.active_aliases {
                assert!(aliases.insert(bound.alias.global_dir_edge_id));
                assert!(std::ptr::eq(
                    bound.native_edge,
                    graph
                        .local_face_graphs()
                        .iter()
                        .find(|local| (local.partition_id, local.component_id)
                            == (bound.alias.partition_id, bound.alias.component_id))
                        .unwrap()
                        .directed_edges
                        .iter()
                        .find(|edge| edge.local_dir_edge_id == bound.alias.local_dir_edge_id)
                        .unwrap()
                ));
                assert_eq!(
                    &bound.input.segment.source_line_ids,
                    &bound.native_edge.source_line_ids
                );
            }
            assert_eq!(aliases, (0..32).collect());
            let mut records = BTreeSet::new();
            for raw in &view.raw_payloads {
                assert!(records.insert((
                    raw.payload.partition_id,
                    raw.payload.kind,
                    raw.family_local_record_index
                )));
                assert_eq!(raw.segments.len(), raw.payload.coordinate_bits.len() - 1);
                assert!(raw
                    .segments
                    .iter()
                    .all(|bound| bound.local_segment_alias[0] == raw.payload.partition_id));
            }
            assert_eq!(records.len(), 16);
        }
        assert_eq!(mosaic.fingerprint_sha256(), before);
        assert_eq!(format!("{graph:?}"), graph_before);
        assert!(classify_non_polygon_evidence(prepare(&mosaic), &policy)
            .unwrap()
            .is_none());

        // Stale context and raw family/record identity cannot reuse an old preparation.
        for change in 0..8 {
            let mut stale = mosaic.clone();
            let snapshot = stale.partitions.get_mut(&0).unwrap();
            match change {
                0 => snapshot.generation.retry_attempt += 1,
                1 => snapshot.tile_max.x = "0x4008000000000000".to_string(),
                2 => snapshot.ownership_domain_max.y = "0x4008000000000000".to_string(),
                3 => snapshot.non_polygon.dangles.reverse(),
                4 => {
                    snapshot.non_polygon.dangles.pop();
                }
                5 => snapshot
                    .non_polygon
                    .cut_edges
                    .push(snapshot.non_polygon.dangles.pop().unwrap()),
                6 => {
                    snapshot.local_noded_segments.pop();
                }
                _ => {
                    stale.partitions.remove(&3);
                }
            }
            let changed_graph = rebuild(locals.clone(), &stale);
            let changed_stats = changed_graph
                .clone()
                .materialize_global_non_polygon_extraction_payloads(&policy)
                .unwrap();
            assert!(
                input_bound_physical_alias_view(
                    &prepared,
                    &changed_graph,
                    &witness,
                    &stale,
                    &options,
                    changed_stats,
                    &policy,
                )
                .unwrap()
                .is_none(),
                "stale context/raw case {change}"
            );
        }
        let mut changed_options = options.clone();
        changed_options.node_input = !options.node_input;
        assert!(input_bound_physical_alias_view(
            &prepared,
            &graph,
            &witness,
            &mosaic,
            &changed_options,
            stats,
            &policy,
        )
        .unwrap()
        .is_none());
        for change in 0..5 {
            let mut changed_witness = witness.clone();
            let edge = &mut changed_witness.physical_edges[0];
            match change {
                0 => edge.next = usize::MAX,
                1 => edge.symmetric = 0,
                2 => edge.from_z_bits = 7.0f64.to_bits(),
                3 => edge.source_line_ids = vec![99],
                _ => edge.face_id = usize::MAX,
            }
            assert!(
                input_bound_physical_alias_view(
                    &prepared,
                    &graph,
                    &changed_witness,
                    &mosaic,
                    &options,
                    stats,
                    &policy,
                )
                .unwrap()
                .is_none(),
                "physical payload/link {change}"
            );
        }
        let mut bad_representative = mosaic.clone();
        bad_representative
            .partitions
            .get_mut(&0)
            .unwrap()
            .local_noded_segments[0]
            .representative_line_id = Some(99);
        assert!(prepare_partition_input_evidence(
            &bad_representative,
            &tiles,
            &refs,
            domain,
            &options,
            &policy,
        )
        .unwrap()
        .is_none());

        // Updating both graph and snapshot cannot replace original source/noded authority.
        for change in 0..7 {
            let mut changed_locals = locals.clone();
            for local in &mut changed_locals {
                for edge in &mut local.directed_edges {
                    match change {
                        0 => edge.source_line_ids = vec![99],
                        1 => {
                            edge.from_z_bits = 7.0f64.to_bits();
                            edge.to_z_bits = 7.0f64.to_bits();
                        }
                        2 => {
                            let shift =
                                |key: crate::graph::partition_border::PartitionBorderNodeKey| {
                                    let bits = key.xy_bits();
                                    crate::graph::partition_border::PartitionBorderNodeKey::from_coord(
                                        Coord3D::new(f64::from_bits(bits[0]) + 0.125,
                                            f64::from_bits(bits[1]), 0.0),
                                    )
                                };
                            edge.from = shift(edge.from);
                            edge.to = shift(edge.to);
                            edge.edge_key =
                                PartitionBorderEdgeKey::new(edge.from, edge.to).unwrap();
                        }
                        3 => edge.local_face_successor = Some(edge.local_dir_edge_id),
                        4 => edge.face_ref.as_mut().unwrap().component_id = 99,
                        5 => {}
                        _ => edge.local_face_successor = None,
                    }
                }
            }
            if change == 5 {
                changed_locals.retain(|local| local.partition_id != 0);
            }
            let mut changed_mosaic = mosaic.clone();
            for snapshot in changed_mosaic.partitions.values_mut() {
                snapshot.local_face_graphs = partition_local_face_graphs(
                    &changed_locals
                        .iter()
                        .filter(|local| local.partition_id == snapshot.partition_id)
                        .cloned()
                        .collect::<Vec<_>>(),
                );
            }
            let changed_graph = rebuild(changed_locals, &changed_mosaic);
            for snapshot in changed_mosaic.partitions.values_mut() {
                snapshot.atomic_observations = crate::tiling::partition_atomic_observations(
                    changed_graph
                        .border_observations()
                        .filter(|observation| observation.partition_id == snapshot.partition_id),
                );
            }
            let changed_prepared = prepare(&changed_mosaic);
            assert!(crate::tiling::private_physical_snapshots_bound(
                &changed_graph,
                &changed_mosaic,
                &options,
                &policy,
            )
            .unwrap());
            let changed_witness = changed_graph
                .global_arrangement_witness(&policy)
                .unwrap()
                .unwrap();
            assert!(
                input_bound_physical_alias_view(
                    &changed_prepared,
                    &changed_graph,
                    &changed_witness,
                    &changed_mosaic,
                    &options,
                    stats,
                    &policy,
                )
                .unwrap()
                .is_none(),
                "coherent native forgery {change}"
            );
        }
        let mut missing_raw = mosaic.clone();
        missing_raw
            .partitions
            .get_mut(&0)
            .unwrap()
            .non_polygon
            .dangles
            .pop();
        let missing_prepared = prepare(&missing_raw);
        let missing_graph = rebuild(locals.clone(), &missing_raw);
        let missing_stats = missing_graph
            .clone()
            .materialize_global_non_polygon_extraction_payloads(&policy)
            .unwrap();
        assert!(
            input_bound_physical_alias_view(
                &missing_prepared,
                &missing_graph,
                &witness,
                &missing_raw,
                &options,
                missing_stats,
                &policy,
            )
            .unwrap()
            .is_none(),
            "missing raw-only prepared local copy"
        );

        for limited in [
            ExecutionPolicy {
                max_graph_edges: Some(0),
                ..Default::default()
            },
            ExecutionPolicy {
                max_output_coordinates: Some(0),
                ..Default::default()
            },
            ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                input_bound_physical_alias_view(
                    &prepared, &graph, &witness, &mosaic, &options, stats, &limited,
                ),
                Err(PolygonizeError::ResourceLimitExceeded { .. })
            ));
        }
        let token = CancellationToken::new();
        token.cancel();
        let cancelled = ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        };
        assert!(matches!(
            input_bound_physical_alias_view(
                &prepared, &graph, &witness, &mosaic, &options, stats, &cancelled,
            ),
            Err(PolygonizeError::Cancelled { .. })
        ));
        let token = CancellationToken::new();
        let midflight = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            cancel_at_work_item: Some((token, 256)),
            ..Default::default()
        };
        assert!(matches!(
            input_bound_physical_alias_view(
                &prepared, &graph, &witness, &mosaic, &options, stats, &midflight,
            ),
            Err(PolygonizeError::Cancelled { .. })
        ));

        // Every retained vector must be bounded before either entry point hashes it.
        let limit = crate::options::MAX_UNCANCELLABLE_SORT_ITEMS;
        let bounded = ExecutionPolicy {
            cancellation_token: Some(CancellationToken::new()),
            ..Default::default()
        };
        for inventory in 0..20 {
            let mut oversized = mosaic.clone();
            let snapshot = oversized.partitions.get_mut(&0).unwrap();
            let empty_coordinate = crate::fingerprint::CoordinateFingerprintV1 {
                x: String::new(),
                y: String::new(),
                z: String::new(),
            };
            let empty_ring = crate::fingerprint::RingFingerprintV1 {
                coordinates: Vec::new(),
                edge_ids: Vec::new(),
            };
            match inventory {
                0 => snapshot.boundary_nodes[0].source_line_ids = vec![0; limit + 1],
                1 => {
                    snapshot.declared_adjacencies =
                        vec![snapshot.declared_adjacencies[0].clone(); limit + 1]
                }
                2 => {
                    let mut node = snapshot.boundary_nodes[0].clone();
                    node.z_bits.clear();
                    node.source_line_ids.clear();
                    node.representative_line_ids.clear();
                    node.face_refs.clear();
                    snapshot.boundary_nodes = vec![node; limit + 1];
                }
                3 => snapshot.boundary_nodes[0].z_bits = vec![0; limit + 1],
                4 => snapshot.boundary_nodes[0].representative_line_ids = vec![0; limit + 1],
                5 => snapshot.boundary_nodes[0].face_refs = vec![[0; 3]; limit + 1],
                6 => {
                    snapshot.topology.polygons = vec![
                        crate::fingerprint::PolygonFingerprintV1 {
                            exterior: Vec::new(),
                            interiors: Vec::new(),
                            exterior_edge_ids: Vec::new(),
                            provenance: None,
                        };
                        limit + 1
                    ]
                }
                7 => snapshot.topology.polygons[0].exterior = vec![empty_coordinate; limit + 1],
                8 => {
                    snapshot.topology.polygons[0].exterior_edge_ids = vec![String::new(); limit + 1]
                }
                9 => snapshot.topology.polygons[0].interiors = vec![empty_ring; limit + 1],
                10 | 11 => {
                    let polygon = &mut snapshot.topology.polygons[0];
                    polygon.interiors = vec![empty_ring];
                    if inventory == 10 {
                        polygon.interiors[0].coordinates = vec![empty_coordinate; limit + 1];
                    } else {
                        polygon.interiors[0].edge_ids = vec![String::new(); limit + 1];
                    }
                }
                12 => {
                    snapshot.topology.polygons[0].provenance =
                        Some(crate::fingerprint::ProvenanceFingerprintV1 {
                            boundary_line_ids: vec![String::new(); limit + 1],
                            input_profile_id: None,
                        })
                }
                13 => snapshot.topology.dangles = vec![Vec::new(); limit + 1],
                14 => snapshot.topology.cut_edges = vec![Vec::new(); limit + 1],
                15 => snapshot.topology.invalid_rings = vec![Vec::new(); limit + 1],
                16 => {
                    snapshot.topology.diagnostics =
                        Some(crate::fingerprint::TopologyDiagnosticsFingerprintV1 {
                            dangle_count: 0,
                            cut_edge_count: 0,
                            ring_count: 0,
                            shell_count: 0,
                            hole_count: 0,
                            unassigned_hole_count: 0,
                            unassigned_hole_area: String::new(),
                            invalid_ring_count: 0,
                            z_conflict_node_count: 0,
                            z_conflicting_line_ids: vec![String::new(); limit + 1],
                        })
                }
                17 => snapshot.topology.dangles = vec![vec![empty_coordinate; limit + 1]],
                18 => snapshot.topology.cut_edges = vec![vec![empty_coordinate; limit + 1]],
                _ => snapshot.topology.invalid_rings = vec![vec![empty_coordinate; limit + 1]],
            }
            if inventory == 0 {
                crate::tiling::preflight_partition_input_snapshots(&oversized, &policy).unwrap();
            }
            for result in [
                prepare_partition_input_evidence(
                    &oversized, &tiles, &refs, domain, &options, &bounded,
                )
                .map(|value| value.is_some()),
                input_bound_physical_alias_view(
                    &prepared, &graph, &witness, &oversized, &options, stats, &bounded,
                )
                .map(|value| value.is_some()),
            ] {
                assert!(
                    matches!(&result, Err(PolygonizeError::ResourceLimitExceeded { stage, .. })
                    if stage == "partition_input_binding_preflight"),
                    "inventory {inventory}: {result:?}"
                );
            }
        }

        // Each individual source/raw buffer fits; the aggregate must reject before hashing.
        let mut oversized = mosaic.clone();
        for snapshot in oversized.partitions.values_mut() {
            for edge in snapshot
                .local_face_graphs
                .iter_mut()
                .flat_map(|local| &mut local.directed_edges)
            {
                edge.source_line_ids = vec![0; 16_000];
            }
            for payload in &mut snapshot.non_polygon.dangles {
                *payload = vec![[0; 3]; 32_000];
            }
        }
        let bounded = ExecutionPolicy {
            cancellation_token: Some(CancellationToken::new()),
            ..Default::default()
        };
        for result in [
            prepare_partition_input_evidence(&oversized, &tiles, &refs, domain, &options, &bounded)
                .map(|value| value.is_some()),
            input_bound_physical_alias_view(
                &prepared, &graph, &witness, &oversized, &options, stats, &bounded,
            )
            .map(|value| value.is_some()),
        ] {
            assert!(
                matches!(result, Err(PolygonizeError::ResourceLimitExceeded { stage, .. })
                if stage == "partition_input_binding_preflight")
            );
        }
        assert_eq!(mosaic.fingerprint_sha256(), before);
        assert_eq!(format!("{graph:?}"), graph_before);
    }

    #[test]
    fn interior_ring_adopts_physical_candidate_without_shared_claims() {
        let geometry = Geometry::LineString(LineString::from(vec![
            (1.0, 1.0),
            (3.0, 1.0),
            (3.0, 3.0),
            (1.0, 3.0),
            (1.0, 1.0),
        ]));
        let mut tiled = TiledPolygonizer::new(
            Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 4.0 }),
            4.0,
        )
        .with_buffer(0.0)
        .with_untiled_equivalence_check();
        tiled.add_geometry(&geometry);
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let result = &traced.result;
        assert!(
            result
                .stitching_report
                .partition_border_global_arrangement_adopted
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_arrangement_alias_edge_count,
            0
        );
        assert_eq!(
            result
                .stitching_report
                .partition_border_global_arrangement_blocked_span_count,
            0
        );
        let construction = &traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_arrangement_candidate")
            .unwrap()
            .payload;
        assert_eq!(construction["constructed"], true);
        assert_eq!(
            construction["candidate"]["slots"].as_array().unwrap().len(),
            8
        );
        assert_eq!(
            construction["candidate"]["cycle_start_physical_dir_edge_ids"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let faces = &traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_arrangement_faces")
            .unwrap()
            .payload;
        assert_eq!(faces["reconciled"], true);
        assert_eq!(faces["components"].as_array().unwrap().len(), 1);
        assert_eq!(faces["faces"].as_array().unwrap().len(), 2);
        assert_eq!(
            faces["face_id_by_local_dir_edge"].as_array().unwrap().len(),
            8
        );
        assert_eq!(
            faces["faces"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|face| face["unbounded"] == true)
                .count(),
            1
        );
        let extraction = &traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_arrangement_extraction")
            .unwrap()
            .payload;
        assert_eq!(extraction["materialized"], true);
        assert_eq!(extraction["rings"].as_array().unwrap().len(), 2);
        assert_eq!(extraction["local_aliases"].as_array().unwrap().len(), 8);
        assert!(extraction["non_polygon_payloads"]
            .as_array()
            .unwrap()
            .is_empty());
        let conformance = &traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_arrangement_conformance")
            .unwrap()
            .payload;
        assert_eq!(conformance["candidate_built"], true);
        assert_eq!(conformance["polygon_count"], 1);
        assert_eq!(conformance["equivalence_checked"], true);
        assert_eq!(conformance["equivalence_ready"], true);
        assert_eq!(conformance["equivalence_mismatch_count"], 0);
        assert_eq!(conformance["non_polygon_ownership_proven"], true);
        assert_eq!(conformance["output_selectable"], false);
    }

    #[test]
    fn physical_table_extraction_retains_tiled_dangles_without_promoting_output() {
        let geometry = Geometry::LineString(LineString::from(vec![
            (1.0, 1.0),
            (3.0, 1.0),
            (3.0, 3.0),
            (1.0, 3.0),
            (1.0, 1.0),
        ]));
        let dangle = Geometry::LineString(LineString::from(vec![(3.25, 3.25), (3.5, 3.25)]));
        let mut tiled = TiledPolygonizer::new(
            Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 4.0 }),
            4.0,
        )
        .with_buffer(0.0)
        .with_untiled_equivalence_check();
        tiled.add_geometry(&geometry);
        tiled.add_geometry(&dangle);
        let ordinary = tiled.polygonize().unwrap();
        assert_eq!(
            ordinary
                .stitching_report
                .partition_border_global_non_polygon_extraction_dangle_count,
            1
        );
        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let report = &traced.result.stitching_report;
        assert_eq!(
            report.partition_border_global_non_polygon_extraction_dangle_count,
            1
        );
        assert_eq!(traced.result.polygons.len(), 1);
        assert!(!report.partition_border_global_stitched_output_ready);
        assert!(!report.partition_border_global_untiled_equivalence_checked);
        assert!(!report.partition_border_global_untiled_equivalence_ready);
        let extraction = &traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_arrangement_extraction")
            .unwrap()
            .payload;
        assert_eq!(extraction["materialized"], true);
        let payloads = extraction["non_polygon_payloads"].as_array().unwrap();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0]["kind"], "dangle");
        assert_eq!(
            payloads[0]["coordinate_bits"],
            serde_json::json!([
                [3.25f64.to_bits(), 3.25f64.to_bits(), 0.0f64.to_bits()],
                [3.5f64.to_bits(), 3.25f64.to_bits(), 0.0f64.to_bits()],
            ])
        );
        let conformance = &traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_arrangement_conformance")
            .unwrap()
            .payload;
        assert_eq!(conformance["candidate_built"], true);
        assert_eq!(conformance["dangle_count"], 1);
        assert_eq!(conformance["equivalence_checked"], true);
        assert_eq!(conformance["equivalence_ready"], true);
        assert_eq!(conformance["equivalence_mismatch_count"], 0);
        assert_eq!(conformance["non_polygon_ownership_proven"], true);
        assert_eq!(conformance["output_selectable"], false);
    }

    #[test]
    fn physical_output_conformance_uses_same_call_options_under_permutation_and_reversal() {
        let ring = LineString::from(vec![
            (1.0, 1.0),
            (3.0, 1.0),
            (3.0, 3.0),
            (1.0, 3.0),
            (1.0, 1.0),
        ]);
        let dangle = LineString::from(vec![(5.0, 5.0), (5.5, 5.0)]);
        for tile_size in [4.0, 8.0] {
            for reverse in [false, true] {
                for extract_only_polygonal in [false, true] {
                    for minimum_face_area in [None, Some(5.0)] {
                        for touch_policy in [
                            crate::TouchPolicy::AllowPointTouchDisallowEdgeShare,
                            crate::TouchPolicy::TreatAnyTouchAsDisjoint,
                            crate::TouchPolicy::AllowEdgeShare,
                        ] {
                            let mut geometries = vec![ring.clone(), dangle.clone()];
                            if reverse {
                                geometries.reverse();
                                for geometry in &mut geometries {
                                    geometry.0.reverse();
                                }
                            }
                            let geometries = geometries
                                .into_iter()
                                .map(Geometry::LineString)
                                .collect::<Vec<_>>();
                            let mut tiled = TiledPolygonizer::new(
                                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 8.0 }),
                                tile_size,
                            )
                            .with_buffer(0.0)
                            .with_options(PolygonizerOptions {
                                extract_only_polygonal,
                                containment: crate::ContainmentOptions { touch_policy },
                                output_filter: crate::OutputFilterOptions { minimum_face_area },
                                provenance: ProvenanceOptions {
                                    enabled: true,
                                    include_boundary_line_ids: true,
                                },
                                input_profile_id: Some("physical-output-conformance".to_string()),
                                ..Default::default()
                            })
                            .with_untiled_equivalence_check();
                            for geometry in &geometries {
                                tiled.add_geometry(geometry);
                            }
                            let ordinary = tiled.polygonize().unwrap();
                            let traced = tiled
                                .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                                .unwrap();
                            assert_eq!(
                                crate::tiling::canonical_polygon_output_keys(
                                    &ordinary.polygons,
                                    &ExecutionPolicy::default()
                                )
                                .unwrap(),
                                crate::tiling::canonical_polygon_output_keys(
                                    &traced.result.polygons,
                                    &ExecutionPolicy::default()
                                )
                                .unwrap()
                            );
                            let conformance = &traced
                                .trace
                                .events
                                .iter()
                                .find(|event| {
                                    event.kind == "partition_border_global_arrangement_conformance"
                                })
                                .unwrap()
                                .payload;
                            assert_eq!(conformance["candidate_built"], true, "tile_size={tile_size} reverse={reverse} polygonal={extract_only_polygonal} minimum={minimum_face_area:?} conformance={conformance}");
                            assert_eq!(conformance["candidate_materialized"], true);
                            assert_eq!(conformance["consumer_bound"], true);
                            assert_eq!(conformance["physical_extraction_bound"], true);
                            assert_eq!(
                                conformance["polygon_count"],
                                usize::from(minimum_face_area.is_none())
                            );
                            assert_eq!(conformance["dangle_count"], 1);
                            assert_eq!(conformance["equivalence_checked"], true);
                            assert_eq!(conformance["equivalence_ready"], true, "tile_size={tile_size} reverse={reverse} polygonal={extract_only_polygonal} minimum={minimum_face_area:?} conformance={conformance}");
                            assert_eq!(conformance["equivalence_mismatch_count"], 0);
                            assert_eq!(conformance["non_polygon_ownership_proven"], true);
                            assert_eq!(conformance["output_selectable"], false);
                            assert!(ordinary.stitched_output.is_none());
                            assert!(traced.result.stitched_output.is_none());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cross_core_complete_private_candidate_conforms_without_accepting_raw_duplicates() {
        let originals = vec![
            Geometry::LineString(LineString::from(vec![
                (0.25, 0.25),
                (0.5, 0.25),
                (0.5, 0.5),
                (0.25, 0.5),
                (0.25, 0.25),
            ])),
            Geometry::LineString(LineString::from(vec![(3.0, 3.5), (9.0, 3.5)])),
            Geometry::LineString(LineString::from(vec![(0.75, 0.75), (1.0, 0.75)])),
        ];
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        for tile_size in [4.0, 6.0] {
            for reverse in [false, true] {
                for node_input in [false, true] {
                    for minimum_face_area in [None, Some(1.0)] {
                        let mut geometries = originals.clone();
                        if reverse {
                            geometries.reverse();
                            for geometry in &mut geometries {
                                if let Geometry::LineString(line) = geometry {
                                    line.0.reverse();
                                }
                            }
                        }
                        let mut tiled = TiledPolygonizer::new(domain, tile_size)
                            .with_buffer(0.0)
                            .with_options(PolygonizerOptions {
                                node_input,
                                input_profile_id: Some("cross-candidate".to_string()),
                                provenance: ProvenanceOptions {
                                    enabled: true,
                                    include_boundary_line_ids: true,
                                },
                                output_filter: crate::OutputFilterOptions { minimum_face_area },
                                ..Default::default()
                            })
                            .with_untiled_equivalence_check();
                        for geometry in &geometries {
                            tiled.add_geometry(geometry);
                        }
                        let ordinary = tiled.polygonize().unwrap();
                        let traced = tiled
                            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                            .unwrap();
                        let raw_count = traced
                            .result
                            .partition_snapshots
                            .iter()
                            .map(|snapshot| snapshot.non_polygon.dangles.len())
                            .sum::<usize>();
                        let conformance = &traced
                            .trace
                            .events
                            .iter()
                            .find(|event| {
                                event.kind == "partition_border_global_arrangement_conformance"
                            })
                            .unwrap()
                            .payload;
                        assert_eq!(conformance["candidate_built"], true, "tile={tile_size} reverse={reverse} noded={node_input} minimum={minimum_face_area:?} conformance={conformance}");
                        assert_eq!(
                            conformance["polygon_count"],
                            usize::from(minimum_face_area.is_none())
                        );
                        assert_eq!(conformance["dangle_count"], 2);
                        assert_eq!(conformance["candidate_materialized"], true);
                        assert_eq!(conformance["consumer_bound"], true);
                        assert_eq!(conformance["physical_extraction_bound"], true);
                        assert_eq!(conformance["cross_core_assembly_bound"], true);
                        assert_eq!(conformance["raw_non_polygon_ledger_bound"], true);
                        assert_eq!(conformance["equivalence_checked"], true);
                        assert_eq!(conformance["equivalence_ready"], true);
                        assert_eq!(conformance["equivalence_mismatch_count"], 0);
                        assert_eq!(conformance["output_selectable"], false);
                        let consumer = &traced
                            .trace
                            .events
                            .iter()
                            .find(|event| {
                                event.kind == "partition_border_global_non_polygon_consumer"
                            })
                            .unwrap()
                            .payload;
                        let payloads = consumer["consumer"]["payloads"].as_array().unwrap();
                        assert_eq!(payloads.len(), 2);
                        assert_eq!(
                            payloads
                                .iter()
                                .map(|payload| payload["local_payload_aliases"]
                                    .as_array()
                                    .unwrap()
                                    .len())
                                .sum::<usize>(),
                            raw_count
                        );
                        assert_eq!(
                            payloads
                                .iter()
                                .map(|payload| payload["global_payload_count"].as_u64().unwrap())
                                .sum::<u64>(),
                            2
                        );
                        assert!(payloads
                            .iter()
                            .any(|payload| payload["proof"]["scope"] == "cross_core"));
                        assert!(payloads
                            .iter()
                            .any(|payload| payload["proof"]["scope"] == "whole_core"));
                        let raw_extraction = &traced
                            .trace
                            .events
                            .iter()
                            .find(|event| {
                                event.kind == "partition_border_global_arrangement_extraction"
                            })
                            .unwrap()
                            .payload;
                        assert_eq!(raw_extraction["materialized"], false);
                        let polygon_extraction = &traced
                            .trace
                            .events
                            .iter()
                            .find(|event| {
                                event.kind
                                    == "partition_border_global_arrangement_polygon_extraction"
                            })
                            .unwrap()
                            .payload;
                        assert_eq!(polygon_extraction["materialized"], true);
                        for result in [&ordinary, &traced.result] {
                            assert!(
                                !result
                                    .stitching_report
                                    .partition_border_global_non_polygon_extraction_ready
                            );
                            assert_eq!(result.stitching_report.partition_border_global_non_polygon_extraction_duplicate_payload_count, raw_count - 2);
                            assert!(
                                !result
                                    .stitching_report
                                    .partition_border_global_untiled_equivalence_ready
                            );
                            assert!(result.stitched_output.is_none());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn cross_core_cut_and_invalid_ring_candidates_require_complete_physical_proofs() {
        let square = |left: f64, right: f64| {
            Geometry::LineString(LineString::from(vec![
                (left, 0.25),
                (right, 0.25),
                (right, 0.375),
                (right, 0.5),
                (left, 0.5),
                (left, 0.375),
                (left, 0.25),
            ]))
        };
        let cut = vec![
            square(0.25, 0.5),
            square(10.0, 10.25),
            Geometry::LineString(LineString::from(vec![(0.5, 0.375), (10.0, 0.375)])),
        ];
        let invalid = Geometry::LineString(LineString::from(vec![
            (1.0, 1.0),
            (9.0, 3.0),
            (9.0, 1.0),
            (1.0, 3.0),
            (1.0, 1.0),
        ]));
        let mixed = [
            cut.clone(),
            vec![
                invalid.clone(),
                Geometry::LineString(LineString::from(vec![(3.0, 3.5), (9.0, 3.5)])),
            ],
        ]
        .concat();
        let separate_input_lines = mixed
            .iter()
            .flat_map(|geometry| {
                let Geometry::LineString(line) = geometry else {
                    unreachable!()
                };
                line.0
                    .windows(2)
                    .map(|pair| Geometry::LineString(LineString(pair.to_vec())))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        let mut checked = 0;
        for (originals, buffer, node_inputs, polygons, cuts, dangles, invalids, fragmented) in [
            (cut.clone(), 12.0, vec![false, true], 2, 1, 0, 0, false),
            (
                vec![square(0.25, 0.5), invalid.clone()],
                0.0,
                vec![false],
                1,
                0,
                0,
                2,
                false,
            ),
            (mixed, 12.0, vec![false], 2, 1, 1, 2, false),
            (separate_input_lines, 12.0, vec![false], 2, 1, 1, 2, true),
        ] {
            for (representation, project) in PHYSICAL_PROOF_REPRESENTATIONS {
                let originals = originals
                    .iter()
                    .map(|geometry| geometry.map_coords(project))
                    .collect::<Vec<_>>();
                let domain = Rect::new(project(domain.min()), project(domain.max()));
                for tile_size in [4.0, 6.0] {
                    for reverse in [false, true] {
                        for &node_input in &node_inputs {
                            for minimum_face_area in [None, Some(1.0)] {
                                // Keep the option matrix on the identity; transformed cases isolate coordinate handling.
                                if representation != "identity"
                                    && (tile_size != 4.0
                                        || reverse
                                        || node_input
                                        || minimum_face_area.is_some())
                                {
                                    continue;
                                }
                                let mut geometries = originals.clone();
                                if reverse {
                                    geometries.reverse();
                                    for geometry in &mut geometries {
                                        if let Geometry::LineString(line) = geometry {
                                            line.0.reverse();
                                        }
                                    }
                                }
                                let options = PolygonizerOptions {
                                    node_input,
                                    input_profile_id: Some("physical-components".to_string()),
                                    provenance: ProvenanceOptions {
                                        enabled: true,
                                        include_boundary_line_ids: true,
                                    },
                                    output_filter: crate::OutputFilterOptions { minimum_face_area },
                                    ..Default::default()
                                };
                                let mut tiled = TiledPolygonizer::new(domain, tile_size)
                                    .with_buffer(buffer)
                                    .with_options(options)
                                    .with_untiled_equivalence_check();
                                for geometry in &geometries {
                                    tiled.add_geometry(geometry);
                                }
                                let ordinary = tiled.polygonize().unwrap();
                                let traced = tiled
                                    .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                                    .unwrap();
                                let conformance = &traced
                                    .trace
                                    .events
                                    .iter()
                                    .find(|event| {
                                        event.kind
                                            == "partition_border_global_arrangement_conformance"
                                    })
                                    .unwrap()
                                    .payload;
                                assert_eq!(conformance["candidate_built"], true, "representation={representation} fragmented={fragmented} cuts={cuts} invalids={invalids} tiles={tile_size} node={node_input} event={conformance} proofs={:?}", traced.trace.events.iter().filter(|event| event.kind == "partition_border_cross_core_non_polygon_assembly" || event.kind == "partition_border_global_non_polygon_consumer" || event.kind == "partition_border_cross_core_physical_proof").map(|event| (&event.kind, &event.payload["constructed"], &event.payload["proven"])).collect::<Vec<_>>());
                                assert_eq!(conformance["equivalence_ready"], true);
                                assert_eq!(conformance["equivalence_mismatch_count"], 0);
                                assert_eq!(
                                    conformance["polygon_count"],
                                    if minimum_face_area.is_some() {
                                        0
                                    } else {
                                        polygons
                                    }
                                );
                                assert_eq!(conformance["cut_edge_count"], cuts);
                                assert_eq!(conformance["dangle_count"], dangles);
                                assert_eq!(conformance["invalid_ring_count"], invalids);
                                assert_eq!(conformance["raw_non_polygon_ledger_bound"], true);
                                assert_eq!(conformance["output_selectable"], false);
                                let proof = &traced
                                    .trace
                                    .events
                                    .iter()
                                    .find(|event| {
                                        event.kind == "partition_border_cross_core_physical_proof"
                                    })
                                    .unwrap()
                                    .payload;
                                assert_eq!(proof["proven"], true);
                                assert_eq!(
                                    proof["proof"]["components"].as_array().unwrap().len(),
                                    polygons as usize
                                );
                                assert_eq!(proof["output_selectable"], false);
                                let components = proof["proof"]["components"].as_array().unwrap();
                                let last_partition =
                                    (traced.result.partition_snapshots.len() - 1) as u64;
                                let expected_owners = if polygons == 1 {
                                    vec![if representation == "reflected" {
                                        last_partition
                                    } else {
                                        0
                                    }]
                                } else {
                                    vec![0, last_partition]
                                }
                                .into_iter()
                                .collect::<BTreeSet<_>>();
                                assert_eq!(
                                    components
                                        .iter()
                                        .map(|component| component["owner_partition_id"]
                                            .as_u64()
                                            .unwrap())
                                        .collect::<BTreeSet<_>>(),
                                    expected_owners,
                                    "representation={representation}"
                                );
                                for component in components {
                                    assert_eq!(
                                        component["copies"].as_array().unwrap().len(),
                                        if buffer == 0.0 {
                                            1
                                        } else {
                                            traced.result.partition_snapshots.len()
                                        }
                                    );
                                }
                                if fragmented {
                                    for snapshot in &traced.result.partition_snapshots {
                                        assert_eq!(
                                            snapshot.selected_input_geometry_indices.len(),
                                            geometries.len()
                                        );
                                    }
                                }
                                checked += 1;
                                for result in [&ordinary, &traced.result] {
                                    assert!(
                                        !result
                                            .stitching_report
                                            .partition_border_global_non_polygon_extraction_ready
                                    );
                                    assert!(result.stitching_report.partition_border_global_non_polygon_extraction_duplicate_payload_count > 0);
                                    assert!(result.stitched_output.is_none());
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(checked, 56);
    }

    #[test]
    fn cross_core_family_options_preserve_multiplicity_and_exact_area_boundaries() {
        let square = |left, right| {
            Geometry::LineString(LineString::from(vec![
                (left, 0.25),
                (right, 0.25),
                (right, 0.375),
                (right, 0.5),
                (left, 0.5),
                (left, 0.375),
                (left, 0.25),
            ]))
        };
        let geometries = [
            square(0.25, 0.5),
            square(10.0, 10.25),
            Geometry::LineString(LineString::from(vec![(0.5, 0.375), (10.0, 0.375)])),
            Geometry::LineString(LineString::from(vec![
                (1.0, 1.0),
                (9.0, 3.0),
                (9.0, 1.0),
                (1.0, 3.0),
                (1.0, 1.0),
            ])),
            Geometry::LineString(LineString::from(vec![(3.0, 3.5), (9.0, 3.5)])),
        ];
        let area = 0.0625f64;
        let above_area = f64::from_bits(area.to_bits() + 1);
        let mut checked = 0;
        for (name, enabled, include_ids, profile, minimum_face_area, polygonal, z, polygons) in [
            (
                "disabled",
                false,
                false,
                None,
                None,
                false,
                crate::ZPolicy::InterpolateAlongEdge,
                2,
            ),
            (
                "disabled_requested_ids",
                false,
                true,
                Some("ignored-profile"),
                None,
                false,
                crate::ZPolicy::InterpolateAlongEdge,
                2,
            ),
            (
                "enabled_without_ids",
                true,
                false,
                None,
                None,
                false,
                crate::ZPolicy::InterpolateAlongEdge,
                2,
            ),
            (
                "enabled_with_ids",
                true,
                true,
                Some("family-options"),
                None,
                false,
                crate::ZPolicy::InterpolateAlongEdge,
                2,
            ),
            (
                "exact_area",
                true,
                true,
                Some("family-options"),
                Some(area),
                false,
                crate::ZPolicy::InterpolateAlongEdge,
                2,
            ),
            (
                "above_area",
                true,
                true,
                Some("family-options"),
                Some(above_area),
                false,
                crate::ZPolicy::InterpolateAlongEdge,
                0,
            ),
            (
                "polygonal",
                true,
                true,
                Some("family-options"),
                None,
                true,
                crate::ZPolicy::InterpolateAlongEdge,
                2,
            ),
            (
                "zero_z_conflict_policy",
                true,
                true,
                Some("family-options"),
                None,
                false,
                crate::ZPolicy::ErrorOnConflict,
                2,
            ),
        ] {
            for tile_size in [4.0, 6.0] {
                for dedup in [DedupPolicy::KeepAll, DedupPolicy::CanonicalRingHash] {
                    let mut tiled =
                        TiledPolygonizer::new(Rect::new((0.0, 0.0), (12.0, 4.0)), tile_size)
                            .with_buffer(12.0)
                            .with_dedup_policy(dedup)
                            .with_options(PolygonizerOptions {
                                node_input: false,
                                extract_only_polygonal: polygonal,
                                provenance: ProvenanceOptions {
                                    enabled,
                                    include_boundary_line_ids: include_ids,
                                },
                                input_profile_id: profile.map(str::to_owned),
                                output_filter: crate::OutputFilterOptions { minimum_face_area },
                                z: ZOptions {
                                    policy: z,
                                    ..Default::default()
                                },
                                ..Default::default()
                            })
                            .with_untiled_equivalence_check();
                    for geometry in &geometries {
                        tiled.add_geometry(geometry);
                    }
                    let ordinary = tiled.polygonize().unwrap();
                    let traced = tiled
                        .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                        .unwrap();
                    let conformance = &traced
                        .trace
                        .events
                        .iter()
                        .find(|event| {
                            event.kind == "partition_border_global_arrangement_conformance"
                        })
                        .unwrap()
                        .payload;
                    assert_eq!(
                        conformance["candidate_built"], true,
                        "case={name} tile={tile_size} event={conformance}"
                    );
                    assert_eq!(conformance["equivalence_ready"], true);
                    assert_eq!(conformance["equivalence_mismatch_count"], 0);
                    assert_eq!(conformance["polygon_count"], polygons);
                    assert_eq!(conformance["dangle_count"], 1);
                    assert_eq!(conformance["cut_edge_count"], 1);
                    assert_eq!(conformance["invalid_ring_count"], 2);
                    assert_eq!(conformance["raw_non_polygon_ledger_bound"], true);
                    assert_eq!(conformance["output_selectable"], false);
                    for result in [&ordinary, &traced.result] {
                        assert!(
                            !result
                                .stitching_report
                                .partition_border_global_non_polygon_extraction_ready
                        );
                        assert!(result.stitching_report.partition_border_global_non_polygon_extraction_duplicate_payload_count > 0);
                        assert!(result.stitched_output.is_none());
                        for polygon in &result.polygons {
                            assert_eq!(polygon.provenance.is_some(), enabled);
                            if let Some(provenance) = &polygon.provenance {
                                assert_eq!(provenance.input_profile_id.as_deref(), profile);
                            }
                        }
                    }
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 32);
    }

    #[test]
    fn cross_core_cut_proofs_conform_under_fixed_and_certified_noding() {
        let square = |left, right| {
            Geometry::LineString(LineString::from(vec![
                (left, 0.25),
                (right, 0.25),
                (right, 0.375),
                (right, 0.5),
                (left, 0.5),
                (left, 0.375),
                (left, 0.25),
            ]))
        };
        let originals = vec![
            square(0.25, 0.5),
            square(10.0, 10.25),
            Geometry::LineString(LineString::from(vec![(0.5, 0.375), (10.0, 0.375)])),
        ];
        let fixed = PrecisionModel::FixedGrid { grid_size: 0.125 };
        let mut checked = 0;
        for (name, precision_model, snap_strategy, guarantee) in [
            (
                "floating_validated",
                PrecisionModel::Floating,
                crate::SnapStrategy::Grid,
                NodingGuarantee::Validate,
            ),
            (
                "fixed_unchecked",
                fixed,
                crate::SnapStrategy::Grid,
                NodingGuarantee::Unchecked,
            ),
            (
                "fixed_validated",
                fixed,
                crate::SnapStrategy::Grid,
                NodingGuarantee::Validate,
            ),
            (
                "fixed_certified",
                fixed,
                crate::SnapStrategy::Grid,
                NodingGuarantee::CertifiedFixedPrecision,
            ),
            (
                "fixed_geos_compat",
                fixed,
                crate::SnapStrategy::GeosCompat,
                NodingGuarantee::Validate,
            ),
        ] {
            for tile_size in [4.0, 6.0] {
                for reverse in [false, true] {
                    let mut geometries = originals.clone();
                    if reverse {
                        geometries.reverse();
                        for geometry in &mut geometries {
                            let Geometry::LineString(line) = geometry else {
                                unreachable!()
                            };
                            line.0.reverse();
                        }
                    }
                    let mut tiled =
                        TiledPolygonizer::new(Rect::new((0.0, 0.0), (12.0, 4.0)), tile_size)
                            .with_buffer(12.0)
                            .with_options(PolygonizerOptions {
                                node_input: true,
                                precision_model,
                                snap_strategy: snap_strategy.clone(),
                                noding: NodingOptions {
                                    guarantee,
                                    ..Default::default()
                                },
                                provenance: ProvenanceOptions {
                                    enabled: true,
                                    include_boundary_line_ids: true,
                                },
                                input_profile_id: Some("fixed-grid-cut".to_string()),
                                ..Default::default()
                            })
                            .with_untiled_equivalence_check();
                    for geometry in &geometries {
                        tiled.add_geometry(geometry);
                    }
                    let ordinary = tiled.polygonize().unwrap();
                    let traced = tiled
                        .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                        .unwrap();
                    let conformance = &traced
                        .trace
                        .events
                        .iter()
                        .find(|event| {
                            event.kind == "partition_border_global_arrangement_conformance"
                        })
                        .unwrap()
                        .payload;
                    assert_eq!(
                        conformance["candidate_built"], true,
                        "case={name} tile={tile_size} reverse={reverse} event={conformance}"
                    );
                    assert_eq!(conformance["candidate_materialized"], true);
                    assert_eq!(conformance["equivalence_ready"], true);
                    assert_eq!(conformance["equivalence_mismatch_count"], 0);
                    assert_eq!(conformance["polygon_count"], 2);
                    assert_eq!(conformance["cut_edge_count"], 1);
                    assert_eq!(conformance["dangle_count"], 0);
                    assert_eq!(conformance["invalid_ring_count"], 0);
                    assert_eq!(conformance["raw_non_polygon_ledger_bound"], true);
                    assert_eq!(conformance["output_selectable"], false);
                    for result in [&ordinary, &traced.result] {
                        assert!(
                            !result
                                .stitching_report
                                .partition_border_global_non_polygon_extraction_ready
                        );
                        assert!(result.stitching_report.partition_border_global_non_polygon_extraction_duplicate_payload_count > 0);
                        assert!(result.stitched_output.is_none());
                    }
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 20);
    }

    #[test]
    fn cross_core_nested_containers_retain_exact_input_and_halo_inventories() {
        let square = |left, right| {
            LineString::from(vec![
                (left, 0.25),
                (right, 0.25),
                (right, 0.375),
                (right, 0.5),
                (left, 0.5),
                (left, 0.375),
                (left, 0.25),
            ])
        };
        let originals = vec![
            square(0.25, 0.5),
            square(10.0, 10.25),
            LineString::from(vec![(0.5, 0.375), (10.0, 0.375)]),
            LineString::from(vec![
                (1.0, 1.0),
                (9.0, 3.0),
                (9.0, 1.0),
                (1.0, 3.0),
                (1.0, 1.0),
            ]),
            LineString::from(vec![(3.0, 3.5), (9.0, 3.5)]),
        ];
        let empty = || Geometry::GeometryCollection(geo::GeometryCollection(Vec::new()));
        let mut checked = 0;
        for reverse in [false, true] {
            let mut lines = originals.clone();
            if reverse {
                lines.reverse();
                for line in &mut lines {
                    line.0.reverse();
                }
            }
            let counts = lines
                .iter()
                .map(|line| line.0.len() - 1)
                .collect::<Vec<_>>();
            let total = counts.iter().sum::<usize>();
            let first_group = counts[..3].iter().sum::<usize>();
            for (name, geometries, selected, source_counts) in [
                (
                    "separate",
                    lines.iter().cloned().map(Geometry::LineString).collect(),
                    (0..lines.len()).collect::<Vec<_>>(),
                    counts.iter().copied().enumerate().collect::<Vec<_>>(),
                ),
                (
                    "multiline",
                    vec![Geometry::MultiLineString(MultiLineString(lines.clone()))],
                    vec![0],
                    vec![(0, total)],
                ),
                (
                    "nested",
                    vec![
                        empty(),
                        Geometry::GeometryCollection(geo::GeometryCollection(vec![
                            Geometry::MultiLineString(MultiLineString(lines[..3].to_vec())),
                            Geometry::GeometryCollection(geo::GeometryCollection(vec![
                                empty(),
                                Geometry::MultiLineString(MultiLineString(lines[3..].to_vec())),
                            ])),
                        ])),
                        empty(),
                    ],
                    vec![1],
                    vec![(1, total)],
                ),
                (
                    "split_with_empty_and_point",
                    vec![
                        empty(),
                        Geometry::MultiLineString(MultiLineString(lines[..3].to_vec())),
                        Geometry::LineString(LineString(Vec::new())),
                        Geometry::GeometryCollection(geo::GeometryCollection(vec![
                            empty(),
                            Geometry::MultiLineString(MultiLineString(lines[3..].to_vec())),
                        ])),
                        Geometry::MultiLineString(MultiLineString(Vec::new())),
                        Geometry::Point(geo::Point::new(0.75, 3.75)),
                    ],
                    vec![1, 3, 5],
                    vec![(1, first_group), (3, total - first_group)],
                ),
            ] {
                let expected_sources = source_counts
                    .iter()
                    .flat_map(|&(geometry, count)| {
                        (0..count).map(move |segment| (geometry, segment))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(expected_sources.len(), 18);
                for tile_size in [4.0, 6.0] {
                    let mut tiled =
                        TiledPolygonizer::new(Rect::new((0.0, 0.0), (12.0, 4.0)), tile_size)
                            .with_buffer(12.0)
                            .with_options(PolygonizerOptions {
                                provenance: ProvenanceOptions {
                                    enabled: true,
                                    include_boundary_line_ids: true,
                                },
                                input_profile_id: Some("container-inventory".to_string()),
                                ..Default::default()
                            })
                            .with_untiled_equivalence_check();
                    for geometry in &geometries {
                        tiled.add_geometry(geometry);
                    }
                    let ordinary = tiled.polygonize().unwrap();
                    let traced = tiled
                        .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                        .unwrap();
                    let conformance = &traced
                        .trace
                        .events
                        .iter()
                        .find(|event| {
                            event.kind == "partition_border_global_arrangement_conformance"
                        })
                        .unwrap()
                        .payload;
                    assert_eq!(
                        conformance["candidate_built"], true,
                        "case={name} reverse={reverse} tile={tile_size} event={conformance}"
                    );
                    assert_eq!(conformance["equivalence_ready"], true);
                    assert_eq!(conformance["equivalence_mismatch_count"], 0);
                    assert_eq!(conformance["polygon_count"], 2);
                    assert_eq!(conformance["cut_edge_count"], 1);
                    assert_eq!(conformance["dangle_count"], 1);
                    assert_eq!(conformance["invalid_ring_count"], 2);
                    assert_eq!(conformance["raw_non_polygon_ledger_bound"], true);
                    assert_eq!(conformance["output_selectable"], false);
                    let proof = &traced
                        .trace
                        .events
                        .iter()
                        .find(|event| event.kind == "partition_border_cross_core_physical_proof")
                        .unwrap()
                        .payload;
                    assert_eq!(proof["proven"], true);
                    let components = proof["proof"]["components"].as_array().unwrap();
                    assert_eq!(components.len(), 2);
                    let partitions = (0..traced.result.partition_snapshots.len() as u64)
                        .collect::<BTreeSet<_>>();
                    for component in components {
                        assert_eq!(
                            component["copies"].as_array().unwrap().len(),
                            partitions.len()
                        );
                        assert_eq!(
                            component["local_component_aliases"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .map(|alias| alias[0].as_u64().unwrap())
                                .collect::<BTreeSet<_>>(),
                            partitions
                        );
                    }
                    for result in [&ordinary, &traced.result] {
                        for snapshot in &result.partition_snapshots {
                            assert_eq!(snapshot.selected_input_geometry_indices, selected);
                            assert_eq!(
                                snapshot
                                    .selected_source_segments
                                    .iter()
                                    .map(|segment| (segment.geometry_index, segment.segment_index))
                                    .collect::<Vec<_>>(),
                                expected_sources
                            );
                            assert!(snapshot.local_noded_segments.iter().all(|segment| {
                                segment.source_line_ids == [0]
                                    && segment.representative_line_id == Some(0)
                            }));
                        }
                        assert!(
                            !result
                                .stitching_report
                                .partition_border_global_non_polygon_extraction_ready
                        );
                        assert!(result.stitching_report.partition_border_global_non_polygon_extraction_duplicate_payload_count > 0);
                        assert!(result.stitched_output.is_none());
                    }
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 16);
    }

    #[test]
    fn cross_core_partial_halos_retain_exact_polygon_copy_owners_and_sources() {
        let square = |left, right| {
            Geometry::LineString(LineString::from(vec![
                (left, 0.25),
                (right, 0.25),
                (right, 0.5),
                (left, 0.5),
                (left, 0.25),
            ]))
        };
        let originals = vec![
            square(0.25, 0.5),
            square(10.0, 10.25),
            Geometry::LineString(LineString::from(vec![
                (1.0, 1.0),
                (9.0, 3.0),
                (9.0, 1.0),
                (1.0, 3.0),
                (1.0, 1.0),
            ])),
            Geometry::LineString(LineString::from(vec![(3.0, 3.5), (9.0, 3.5)])),
        ];
        let mut checked = 0;
        // Explicit partition sets check both sides of each inclusive halo-selection threshold.
        for (buffer, left, right) in [
            (0.0, &[0][..], &[2][..]),
            (1.875, &[0][..], &[2][..]),
            (2.0, &[0][..], &[1, 2][..]),
            (3.375, &[0][..], &[1, 2][..]),
            (3.5, &[0, 1][..], &[1, 2][..]),
            (5.875, &[0, 1][..], &[1, 2][..]),
            (6.0, &[0, 1][..], &[0, 1, 2][..]),
            (7.375, &[0, 1][..], &[0, 1, 2][..]),
            (7.5, &[0, 1, 2][..], &[0, 1, 2][..]),
        ] {
            for reverse in [false, true] {
                let mut geometries = originals.clone();
                if reverse {
                    geometries.reverse();
                    for geometry in &mut geometries {
                        let Geometry::LineString(line) = geometry else {
                            unreachable!()
                        };
                        line.0.reverse();
                    }
                }
                let mut tiled = TiledPolygonizer::new(Rect::new((0.0, 0.0), (12.0, 4.0)), 4.0)
                    .with_buffer(buffer)
                    .with_options(PolygonizerOptions {
                        provenance: ProvenanceOptions {
                            enabled: true,
                            include_boundary_line_ids: true,
                        },
                        input_profile_id: Some("partial-halo-inventory".to_string()),
                        ..Default::default()
                    })
                    .with_untiled_equivalence_check();
                for geometry in &geometries {
                    tiled.add_geometry(geometry);
                }
                let ordinary = tiled.polygonize().unwrap();
                let traced = tiled
                    .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                    .unwrap();
                let conformance = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_global_arrangement_conformance")
                    .unwrap()
                    .payload;
                assert_eq!(
                    conformance["candidate_built"], true,
                    "buffer={buffer} reverse={reverse} event={conformance}"
                );
                assert_eq!(conformance["equivalence_ready"], true);
                assert_eq!(conformance["equivalence_mismatch_count"], 0);
                assert_eq!(conformance["polygon_count"], 2);
                assert_eq!(conformance["cut_edge_count"], 0);
                assert_eq!(conformance["dangle_count"], 1);
                assert_eq!(conformance["invalid_ring_count"], 2);
                assert_eq!(conformance["raw_non_polygon_ledger_bound"], true);
                assert_eq!(conformance["output_selectable"], false);
                let proof = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_cross_core_physical_proof")
                    .unwrap()
                    .payload;
                assert_eq!(proof["proven"], true);
                let components = proof["proof"]["components"].as_array().unwrap();
                assert_eq!(components.len(), 2);
                assert_eq!(
                    components
                        .iter()
                        .map(|component| component["owner_partition_id"].as_u64().unwrap())
                        .collect::<BTreeSet<_>>(),
                    [0, 2].into_iter().collect()
                );
                for component in components {
                    let owner = component["owner_partition_id"].as_u64().unwrap() as usize;
                    let expected = match owner {
                        0 => left,
                        2 => right,
                        _ => panic!("unexpected polygon owner {owner}"),
                    };
                    let aliases = component["local_component_aliases"].as_array().unwrap();
                    assert_eq!(aliases.len(), expected.len());
                    assert_eq!(
                        component["copies"].as_array().unwrap().len(),
                        expected.len()
                    );
                    assert_eq!(
                        aliases
                            .iter()
                            .map(|alias| alias[0].as_u64().unwrap() as usize)
                            .collect::<BTreeSet<_>>(),
                        expected.iter().copied().collect::<BTreeSet<_>>()
                    );
                    let owner_copy = component["owner_copy_index"].as_u64().unwrap() as usize;
                    assert_eq!(aliases[owner_copy][0].as_u64().unwrap() as usize, owner);
                }
                for result in [&ordinary, &traced.result] {
                    assert_eq!(result.partition_snapshots.len(), 3);
                    for snapshot in &result.partition_snapshots {
                        assert_eq!(snapshot.generation.buffer_bits, buffer.to_bits());
                        assert_eq!(snapshot.generation.retry_attempt, 0);
                        let mut counts = vec![(2, 4), (3, 1)];
                        if left.contains(&snapshot.partition_id) {
                            counts.push((0, 4));
                        }
                        if right.contains(&snapshot.partition_id) {
                            counts.push((1, 4));
                        }
                        if reverse {
                            for (geometry, _) in &mut counts {
                                *geometry = geometries.len() - 1 - *geometry;
                            }
                        }
                        counts.sort_unstable();
                        assert_eq!(
                            snapshot.selected_input_geometry_indices,
                            counts
                                .iter()
                                .map(|&(geometry, _)| geometry)
                                .collect::<Vec<_>>()
                        );
                        assert_eq!(
                            snapshot
                                .selected_source_segments
                                .iter()
                                .map(|segment| (segment.geometry_index, segment.segment_index))
                                .collect::<Vec<_>>(),
                            counts
                                .iter()
                                .flat_map(|&(geometry, count)| {
                                    (0..count).map(move |segment| (geometry, segment))
                                })
                                .collect::<Vec<_>>()
                        );
                    }
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_ready
                    );
                    assert!(
                        result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_duplicate_payload_count
                            > 0
                    );
                    assert!(result.stitched_output.is_none());
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 18);
    }

    #[test]
    fn cross_core_partial_cut_halos_decline_inconsistent_local_families() {
        let square = |left, right| {
            Geometry::LineString(LineString::from(vec![
                (left, 0.25),
                (right, 0.25),
                (right, 0.375),
                (right, 0.5),
                (left, 0.5),
                (left, 0.375),
                (left, 0.25),
            ]))
        };
        let originals = vec![
            square(0.25, 0.5),
            square(10.0, 10.25),
            Geometry::LineString(LineString::from(vec![(0.5, 0.375), (10.0, 0.375)])),
        ];
        let mut checked = 0;
        for (buffer, dangles, cuts, ready) in [
            (0.0, 3, 0, false),
            (3.5, 2, 1, false),
            (6.0, 1, 2, false),
            (7.375, 1, 2, false),
            (7.5, 0, 3, true),
        ] {
            for node_input in [false, true] {
                for reverse in [false, true] {
                    let mut geometries = originals.clone();
                    if reverse {
                        geometries.reverse();
                        for geometry in &mut geometries {
                            let Geometry::LineString(line) = geometry else {
                                unreachable!()
                            };
                            line.0.reverse();
                        }
                    }
                    let options = PolygonizerOptions {
                        node_input,
                        provenance: ProvenanceOptions {
                            enabled: true,
                            include_boundary_line_ids: true,
                        },
                        input_profile_id: Some("partial-cut-halos".to_string()),
                        ..Default::default()
                    };
                    let mut reference = Polygonizer::with_options(options.clone());
                    let mut tiled = TiledPolygonizer::new(Rect::new((0.0, 0.0), (12.0, 4.0)), 4.0)
                        .with_buffer(buffer)
                        .with_options(options)
                        .with_untiled_equivalence_check();
                    for geometry in &geometries {
                        reference.add_borrowed_geometry(geometry);
                        tiled.add_geometry(geometry);
                    }
                    let reference = reference.polygonize().unwrap();
                    assert_eq!(reference.polygons.len(), 2);
                    assert_eq!(reference.cut_edges.len(), 1);
                    assert!(reference.dangles.is_empty());
                    assert!(reference.invalid_rings.is_empty());
                    let ordinary = tiled.polygonize().unwrap();
                    let traced = tiled
                        .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                        .unwrap();
                    for result in [&ordinary, &traced.result] {
                        assert_eq!(result.partition_snapshots.len(), 3);
                        let mut families = (0, 0, 0);
                        for snapshot in &result.partition_snapshots {
                            assert_eq!(snapshot.generation.buffer_bits, buffer.to_bits());
                            assert_eq!(snapshot.generation.retry_attempt, 0);
                            families.0 += snapshot.non_polygon.dangles.len();
                            families.1 += snapshot.non_polygon.cut_edges.len();
                            families.2 += snapshot.non_polygon.invalid_rings.len();
                        }
                        assert_eq!(
                            families,
                            (dangles, cuts, 0),
                            "buffer={buffer} node={node_input} reverse={reverse}"
                        );
                        assert!(
                            !result
                                .stitching_report
                                .partition_border_global_non_polygon_extraction_ready
                        );
                        assert!(result.stitching_report.partition_border_global_non_polygon_extraction_duplicate_payload_count > 0);
                        assert!(result.stitched_output.is_none());
                    }
                    let conformance = &traced
                        .trace
                        .events
                        .iter()
                        .find(|event| {
                            event.kind == "partition_border_global_arrangement_conformance"
                        })
                        .unwrap()
                        .payload;
                    assert_eq!(
                        conformance["candidate_built"], ready,
                        "buffer={buffer} node={node_input} reverse={reverse} event={conformance}"
                    );
                    assert_eq!(conformance["candidate_materialized"], ready);
                    assert_eq!(conformance["equivalence_checked"], ready);
                    assert_eq!(conformance["equivalence_ready"], ready);
                    assert_eq!(conformance["equivalence_mismatch_count"], 0);
                    assert_eq!(conformance["raw_non_polygon_ledger_bound"], ready);
                    assert_eq!(conformance["output_selectable"], false);
                    if ready {
                        assert_eq!(conformance["polygon_count"], 2);
                        assert_eq!(conformance["cut_edge_count"], 1);
                        assert_eq!(conformance["dangle_count"], 0);
                        assert_eq!(conformance["invalid_ring_count"], 0);
                    }
                    let consumer = &traced
                        .trace
                        .events
                        .iter()
                        .find(|event| event.kind == "partition_border_global_non_polygon_consumer")
                        .unwrap()
                        .payload;
                    assert_eq!(consumer["constructed"], ready);
                    let proof = &traced
                        .trace
                        .events
                        .iter()
                        .find(|event| event.kind == "partition_border_cross_core_physical_proof")
                        .unwrap()
                        .payload;
                    assert_eq!(proof["proven"], ready);
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 20);
    }

    #[test]
    fn cross_core_polygon_containers_preserve_rings_and_decline_hole_assembly() {
        let mut checked = 0;
        for reverse in [false, true] {
            let ring = |left, bottom, right, top| {
                let mut line = LineString::from(vec![
                    (left, bottom),
                    (right, bottom),
                    (right, top),
                    (left, top),
                    (left, bottom),
                ]);
                if reverse {
                    line.0.reverse();
                }
                line
            };
            let mut shells = vec![
                geo::Polygon::new(ring(0.25, 0.25, 0.5, 0.5), Vec::new()),
                geo::Polygon::new(ring(10.0, 0.25, 10.25, 0.5), Vec::new()),
            ];
            if reverse {
                shells.reverse();
            }
            let holed =
                geo::Polygon::new(ring(0.25, 0.25, 1.75, 1.75), vec![ring(0.5, 0.5, 1.5, 1.5)]);
            let dangle = Geometry::LineString(LineString::from(if reverse {
                vec![(9.0, 3.5), (3.0, 3.5)]
            } else {
                vec![(3.0, 3.5), (9.0, 3.5)]
            }));
            for (name, mut geometries, mut source_counts, ready) in [
                (
                    "polygons",
                    vec![
                        Geometry::Polygon(shells[0].clone()),
                        Geometry::Polygon(shells[1].clone()),
                        dangle.clone(),
                    ],
                    vec![4, 4, 1],
                    true,
                ),
                (
                    "multipolygon",
                    vec![
                        Geometry::MultiPolygon(geo::MultiPolygon(shells.clone())),
                        dangle.clone(),
                    ],
                    vec![8, 1],
                    true,
                ),
                (
                    "nested_multipolygon",
                    vec![
                        Geometry::GeometryCollection(geo::GeometryCollection(vec![
                            Geometry::MultiPolygon(geo::MultiPolygon(shells.clone())),
                        ])),
                        dangle.clone(),
                    ],
                    vec![8, 1],
                    true,
                ),
                (
                    "polygon_with_hole",
                    vec![Geometry::Polygon(holed.clone()), dangle.clone()],
                    vec![8, 1],
                    false,
                ),
                (
                    "multipolygon_with_hole",
                    vec![
                        Geometry::MultiPolygon(geo::MultiPolygon(vec![holed.clone()])),
                        dangle.clone(),
                    ],
                    vec![8, 1],
                    false,
                ),
                (
                    "nested_multipolygon_with_hole",
                    vec![
                        Geometry::GeometryCollection(geo::GeometryCollection(vec![
                            Geometry::MultiPolygon(geo::MultiPolygon(vec![holed.clone()])),
                        ])),
                        dangle.clone(),
                    ],
                    vec![8, 1],
                    false,
                ),
            ] {
                if reverse {
                    geometries.reverse();
                    source_counts.reverse();
                }
                let expected_sources = source_counts
                    .iter()
                    .enumerate()
                    .flat_map(|(geometry, &count)| {
                        (0..count).map(move |segment| (geometry, segment))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(expected_sources.len(), 9);
                let options = PolygonizerOptions {
                    provenance: ProvenanceOptions {
                        enabled: true,
                        include_boundary_line_ids: true,
                    },
                    input_profile_id: Some("polygon-containers".to_string()),
                    ..Default::default()
                };
                let mut reference = Polygonizer::with_options(options.clone());
                let mut tiled = TiledPolygonizer::new(Rect::new((0.0, 0.0), (12.0, 4.0)), 4.0)
                    .with_buffer(12.0)
                    .with_options(options)
                    .with_untiled_equivalence_check();
                for geometry in &geometries {
                    reference.add_borrowed_geometry(geometry);
                    tiled.add_geometry(geometry);
                }
                let reference = reference.polygonize().unwrap();
                // Polygon inputs contribute linework; default extraction retains both bounded faces.
                assert_eq!(reference.polygons.len(), 2);
                assert_eq!(
                    reference
                        .polygons
                        .iter()
                        .map(|polygon| polygon.interiors.len())
                        .sum::<usize>(),
                    usize::from(!ready)
                );
                assert_eq!(reference.dangles.len(), 1);
                assert!(reference.cut_edges.is_empty());
                assert!(reference.invalid_rings.is_empty());
                let ordinary = tiled.polygonize().unwrap();
                let traced = tiled
                    .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                    .unwrap();
                let conformance = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_global_arrangement_conformance")
                    .unwrap()
                    .payload;
                assert_eq!(
                    conformance["candidate_built"], ready,
                    "case={name} reverse={reverse} event={conformance}"
                );
                assert_eq!(conformance["candidate_materialized"], ready);
                assert_eq!(conformance["equivalence_checked"], ready);
                assert_eq!(conformance["equivalence_ready"], ready);
                assert_eq!(conformance["equivalence_mismatch_count"], 0);
                assert_eq!(conformance["output_selectable"], false);
                let consumer = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_global_non_polygon_consumer")
                    .unwrap()
                    .payload;
                assert_eq!(consumer["constructed"], true);
                let proof = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_cross_core_physical_proof")
                    .unwrap()
                    .payload;
                assert_eq!(proof["proven"], ready);
                for result in [&ordinary, &traced.result] {
                    assert_eq!(result.partition_snapshots.len(), 3);
                    for snapshot in &result.partition_snapshots {
                        assert_eq!(
                            snapshot.selected_input_geometry_indices,
                            (0..geometries.len()).collect::<Vec<_>>()
                        );
                        assert_eq!(
                            snapshot
                                .selected_source_segments
                                .iter()
                                .map(|segment| (segment.geometry_index, segment.segment_index))
                                .collect::<Vec<_>>(),
                            expected_sources
                        );
                    }
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_ready
                    );
                    assert!(
                        result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_duplicate_payload_count
                            > 0
                    );
                    assert!(result.stitched_output.is_none());
                }
                checked += 1;
            }
        }
        assert_eq!(checked, 12);
    }

    #[test]
    fn cross_core_physical_proofs_reject_nesting_contact_and_boundary_ambiguity() {
        let square = |left, bottom, right, top| {
            Geometry::LineString(LineString::from(vec![
                (left, bottom),
                (right, bottom),
                (right, top),
                (left, top),
                (left, bottom),
            ]))
        };
        let invalid = Geometry::LineString(LineString::from(vec![
            (1.0, 1.0),
            (9.0, 3.0),
            (9.0, 1.0),
            (1.0, 3.0),
            (1.0, 1.0),
        ]));
        let boundary_crossing = Geometry::LineString(LineString::from(vec![
            (2.0, 1.0),
            (10.0, 3.0),
            (10.0, 1.0),
            (2.0, 3.0),
            (2.0, 1.0),
        ]));
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        let mut checked = 0;
        for (originals, tile_size) in [
            (
                vec![square(0.25, 0.25, 1.5, 1.5), square(0.5, 0.5, 1.0, 1.0)],
                4.0,
            ),
            (
                vec![square(0.25, 0.25, 0.5, 0.5), square(0.5, 0.5, 0.75, 0.75)],
                4.0,
            ),
            (vec![square(3.0, 0.25, 5.0, 0.5)], 4.0),
            (vec![square(4.0, 0.25, 4.5, 0.5)], 4.0),
            (vec![square(0.25, 0.25, 0.5, 0.5), boundary_crossing], 6.0),
            (vec![square(1.5, 1.75, 1.75, 2.0), invalid], 4.0),
        ] {
            for (representation, project) in PHYSICAL_PROOF_REPRESENTATIONS {
                let geometries = originals
                    .iter()
                    .map(|geometry| geometry.map_coords(project))
                    .collect::<Vec<_>>();
                let domain = Rect::new(project(domain.min()), project(domain.max()));
                for (touch_name, touch_policy) in [
                    (
                        "point",
                        crate::TouchPolicy::AllowPointTouchDisallowEdgeShare,
                    ),
                    ("disjoint", crate::TouchPolicy::TreatAnyTouchAsDisjoint),
                    ("edge", crate::TouchPolicy::AllowEdgeShare),
                ] {
                    for minimum_face_area in [None, Some(100.0)] {
                        // The identity isolates output options; retain the original coordinate matrix.
                        if representation != "identity"
                            && (touch_name != "point" || minimum_face_area.is_some())
                        {
                            continue;
                        }
                        let mut tiled = TiledPolygonizer::new(domain, tile_size)
                            .with_buffer(12.0)
                            .with_options(PolygonizerOptions {
                                node_input: false,
                                containment: crate::ContainmentOptions {
                                    touch_policy: touch_policy.clone(),
                                },
                                output_filter: crate::OutputFilterOptions { minimum_face_area },
                                ..Default::default()
                            })
                            .with_untiled_equivalence_check();
                        for geometry in &geometries {
                            tiled.add_geometry(geometry);
                        }
                        let dangle =
                            Geometry::LineString(LineString::from(vec![(3.0, 3.5), (9.0, 3.5)]))
                                .map_coords(project);
                        tiled.add_geometry(&dangle);
                        let ordinary = tiled.polygonize().unwrap();
                        let traced = tiled
                            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                            .unwrap();
                        let proof = &traced
                            .trace
                            .events
                            .iter()
                            .find(|event| {
                                event.kind == "partition_border_cross_core_physical_proof"
                            })
                            .unwrap()
                            .payload;
                        assert_eq!(
                            proof["proven"], false,
                            "representation={representation} touch={touch_name} minimum={minimum_face_area:?} geometries={geometries:?} proof={proof}"
                        );
                        let conformance = &traced
                            .trace
                            .events
                            .iter()
                            .find(|event| {
                                event.kind == "partition_border_global_arrangement_conformance"
                            })
                            .unwrap()
                            .payload;
                        assert_eq!(
                            conformance["candidate_built"], false,
                            "representation={representation} touch={touch_name} minimum={minimum_face_area:?} geometries={geometries:?} conformance={conformance}"
                        );
                        assert_eq!(conformance["candidate_materialized"], false);
                        assert_eq!(conformance["output_selectable"], false);
                        for result in [&ordinary, &traced.result] {
                            if minimum_face_area.is_some() {
                                assert!(result.polygons.is_empty());
                            }
                            assert!(result.stitched_output.is_none());
                        }
                        checked += 1;
                    }
                }
            }
        }
        assert_eq!(checked, 60);
    }

    #[test]
    fn cross_core_physical_proofs_bind_every_copy_and_decline_changed_evidence_atomically() {
        use crate::graph::partition_border::{
            PartitionBorderGlobalNonPolygonExtractionPayload, PartitionBorderGraph,
            PartitionBorderHalfEdge, PartitionBorderLocalFaceGraph,
        };
        use crate::tiling::{
            assemble_cross_core_non_polygon, compare_stitched_output_with_reference,
            consume_global_non_polygon, integrate_cross_core_component_candidate,
            prepare_non_polygon_evidence, prove_cross_core_physical_polygons,
            PartitionCrossCorePhysicalInputs, PartitionMosaic,
        };
        let square = |left, right| {
            Geometry::LineString(LineString::from(vec![
                (left, 0.25),
                (right, 0.25),
                (right, 0.375),
                (right, 0.5),
                (left, 0.5),
                (left, 0.375),
                (left, 0.25),
            ]))
        };
        let geometries = [
            square(0.25, 0.5),
            square(10.0, 10.25),
            Geometry::LineString(LineString::from(vec![(0.5, 0.375), (10.0, 0.375)])),
            Geometry::LineString(LineString::from(vec![
                (1.0, 1.0),
                (9.0, 3.0),
                (9.0, 1.0),
                (1.0, 3.0),
                (1.0, 1.0),
            ])),
            Geometry::LineString(LineString::from(vec![(3.0, 3.5), (9.0, 3.5)])),
        ];
        let refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        let options = PolygonizerOptions {
            node_input: false,
            input_profile_id: Some("physical-proof".to_string()),
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 4.0)
            .with_buffer(12.0)
            .with_options(options.clone());
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let tiles = tiled.generate_tiles().unwrap();
        let mut graph = PartitionBorderGraph::default();
        let mut mosaic = PartitionMosaic::default();
        for (partition, &tile) in tiles.iter().enumerate() {
            let (_, _, _, observations, locals, _, _, _, _, _, _, _, snapshot) = tiled
                .process_tile(partition, tile, &tiles, &[], 12.0, 0, None)
                .unwrap();
            for observation in observations {
                graph.insert(observation).unwrap();
            }
            for local in locals {
                graph.insert_local_face_graph(local).unwrap();
            }
            mosaic.replace_partition(snapshot, &policy).unwrap();
        }
        let decode = |family: &[Vec<[u64; 3]>]| {
            family
                .iter()
                .map(|bits| {
                    bits.iter()
                        .copied()
                        .map(crate::tiling::coord3d_from_bits)
                        .collect()
                })
                .collect()
        };
        for snapshot in mosaic.partitions.values() {
            graph
                .retain_global_non_polygon_extraction_payloads(
                    &policy,
                    snapshot.partition_id,
                    decode(&snapshot.non_polygon.dangles),
                    decode(&snapshot.non_polygon.cut_edges),
                    decode(&snapshot.non_polygon.invalid_rings),
                )
                .unwrap();
        }
        let stats = graph
            .materialize_global_non_polygon_extraction_payloads(&policy)
            .unwrap();
        assert!(!stats.payload_ready);
        let evidence =
            prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                .unwrap()
                .unwrap();
        let assembly = assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
            .unwrap()
            .unwrap();
        let consumer = consume_global_non_polygon(
            &evidence,
            None,
            Some(&assembly),
            &mosaic,
            &tiles,
            domain,
            &policy,
        )
        .unwrap()
        .unwrap();
        let inputs = PartitionCrossCorePhysicalInputs {
            graph: &graph,
            non_polygon_stats: stats,
            evidence: &evidence,
            assembly: &assembly,
            mosaic: &mosaic,
            tiles: &tiles,
            domain,
            options: &options,
        };
        let before = graph.clone();
        let before_mosaic = mosaic.fingerprint_sha256();
        let proof = prove_cross_core_physical_polygons(&inputs, &consumer, &policy)
            .unwrap()
            .unwrap();
        assert_eq!(proof.components.len(), 2);
        assert!(!proof.non_polygon_component_aliases.is_empty());
        let retained_polygon_aliases = proof
            .components
            .iter()
            .map(|component| {
                assert_eq!(component.copies.len(), 3);
                assert_eq!(component.local_component_aliases.len(), 3);
                assert_eq!(
                    component.local_component_aliases[component.owner_copy_index][0],
                    component.owner_partition_id
                );
                component
                    .copies
                    .iter()
                    .map(|copy| copy.local_aliases.len())
                    .sum::<usize>()
            })
            .sum::<usize>();
        assert_eq!(
            retained_polygon_aliases
                + proof
                    .non_polygon_component_aliases
                    .iter()
                    .map(|(_, ids)| ids.len())
                    .sum::<usize>(),
            graph
                .local_face_graphs()
                .iter()
                .map(|local| local.directed_edges.len())
                .sum::<usize>()
        );
        let output = integrate_cross_core_component_candidate(&inputs, &proof, &consumer, &policy)
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                output.polygons.len(),
                output.dangles.len(),
                output.cut_edges.len(),
                output.invalid_rings.len()
            ),
            (2, 1, 1, 2)
        );
        assert!(
            compare_stitched_output_with_reference(&output, &evidence.reference, &policy)
                .unwrap()
                .ready
        );
        for mutation in 0..14 {
            let mut changed = proof.clone();
            match mutation {
                0 => changed.input_fingerprint_sha256.clear(),
                1 => changed.snapshot_fingerprints[0].1.clear(),
                2 => {
                    changed.components.pop();
                }
                3 => changed.components[0].owner_partition_id = usize::MAX,
                4 => changed.components[0].owner_copy_index = usize::MAX,
                5 => {
                    changed.components[0].local_component_aliases.pop();
                }
                6 => {
                    changed.components[0].copies.pop();
                }
                7 => {
                    changed.components[0].copies[0].ring_payloads[0].coordinate_bits[0][2] =
                        1.0f64.to_bits()
                }
                8 => changed.components[0].copies[0].ring_payloads[0]
                    .source_line_ids
                    .push(99),
                9 => {
                    changed.components[0].copies[0].ring_payloads[0].representative_line_ids[0] = 99
                }
                10 => {
                    changed.components[0].copies[0].local_aliases[0].from_z_bits = 1.0f64.to_bits()
                }
                11 => {
                    changed.components[0].copies[0].local_aliases[0]
                        .face_ref
                        .as_mut()
                        .unwrap()
                        .component_id += 1
                }
                12 => {
                    changed.non_polygon_component_aliases.pop();
                }
                _ => changed.non_polygon_component_aliases[0].1.clear(),
            }
            assert!(
                integrate_cross_core_component_candidate(&inputs, &changed, &consumer, &policy)
                    .unwrap()
                    .is_none(),
                "proof mutation {mutation}"
            );
        }
        let rebuild =
            |locals: Vec<PartitionBorderLocalFaceGraph>,
             observations: Vec<PartitionBorderHalfEdge>,
             rows: Vec<PartitionBorderGlobalNonPolygonExtractionPayload>| {
                let mut changed = PartitionBorderGraph::default();
                for observation in observations {
                    changed.insert(observation).unwrap();
                }
                for local in locals {
                    changed.insert_local_face_graph(local).unwrap();
                }
                for row in rows {
                    let coords = row
                        .coordinate_bits
                        .into_iter()
                        .map(crate::tiling::coord3d_from_bits)
                        .collect();
                    let (mut dangles, mut cuts, mut invalids) =
                        (Vec::new(), Vec::new(), Vec::new());
                    match row.kind { crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind::Dangle => dangles.push(coords), crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind::CutEdge => cuts.push(coords), crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind::InvalidRing => invalids.push(coords) }
                    changed
                        .retain_global_non_polygon_extraction_payloads(
                            &policy,
                            row.partition_id,
                            dangles,
                            cuts,
                            invalids,
                        )
                        .unwrap();
                }
                changed
            };
        // A complete proof must retain halo copies named by the prepared source inventory.
        // Remove only one polygon cycle: two cycles share the original component ID.
        let polygon = &proof.components[0];
        let removed_copy_index = polygon
            .local_component_aliases
            .iter()
            .position(|alias| alias[0] != polygon.owner_partition_id)
            .unwrap();
        let removed_alias = polygon.local_component_aliases[removed_copy_index];
        let removed_edge_ids = polygon.copies[removed_copy_index]
            .local_aliases
            .iter()
            .map(|alias| alias.local_dir_edge_id)
            .collect::<std::collections::BTreeSet<_>>();
        let mut missing_locals = graph.local_face_graphs().to_vec();
        let local = missing_locals
            .iter_mut()
            .find(|local| [local.partition_id, local.component_id] == removed_alias)
            .unwrap();
        let original_edge_count = local.directed_edges.len();
        local
            .directed_edges
            .retain(|edge| !removed_edge_ids.contains(&edge.local_dir_edge_id));
        assert_eq!(
            original_edge_count - local.directed_edges.len(),
            removed_edge_ids.len()
        );
        assert!(!local.directed_edges.is_empty());
        let mut missing_mosaic = mosaic.clone();
        missing_mosaic
            .partitions
            .get_mut(&removed_alias[0])
            .unwrap()
            .local_face_graphs = crate::tiling::partition_local_face_graphs(&missing_locals)
            .into_iter()
            .filter(|local| local.partition_id == removed_alias[0])
            .collect();
        let original_snapshot = &mosaic.partitions[&removed_alias[0]];
        let missing_snapshot = &missing_mosaic.partitions[&removed_alias[0]];
        assert_eq!(
            missing_snapshot.selected_source_segments,
            original_snapshot.selected_source_segments
        );
        assert_eq!(
            missing_snapshot.local_noded_segments,
            original_snapshot.local_noded_segments
        );
        assert_eq!(
            missing_snapshot.boundary_noded_segments,
            original_snapshot.boundary_noded_segments
        );
        missing_mosaic.physical_spans =
            PartitionMosaic::physical_spans(&missing_mosaic.partitions, &policy).unwrap();
        let missing_graph = rebuild(
            missing_locals,
            graph.border_observations().cloned().collect(),
            graph
                .global_non_polygon_extraction_payload_candidates()
                .to_vec(),
        );
        let missing_evidence =
            prepare_non_polygon_evidence(&missing_mosaic, &tiles, &refs, domain, &options, &policy)
                .unwrap()
                .unwrap();
        assert_eq!(missing_evidence.aliases, evidence.aliases);
        let missing_assembly = assemble_cross_core_non_polygon(
            &missing_evidence,
            &missing_mosaic,
            &tiles,
            domain,
            &policy,
        )
        .unwrap()
        .unwrap();
        let missing_consumer = consume_global_non_polygon(
            &missing_evidence,
            None,
            Some(&missing_assembly),
            &missing_mosaic,
            &tiles,
            domain,
            &policy,
        )
        .unwrap()
        .unwrap();
        let missing_inputs = PartitionCrossCorePhysicalInputs {
            graph: &missing_graph,
            mosaic: &missing_mosaic,
            evidence: &missing_evidence,
            assembly: &missing_assembly,
            ..inputs
        };
        let missing_proof =
            prove_cross_core_physical_polygons(&missing_inputs, &missing_consumer, &policy)
                .unwrap();
        assert!(
            missing_proof.is_none(),
            "missing non-owner polygon halo copy must reject complete physical proof"
        );
        for claim_count in [2, 4] {
            let mut forged = mosaic.clone();
            let mut observations = graph.border_observations().cloned().collect::<Vec<_>>();
            for partition in [0, 1] {
                for reverse in [false, true] {
                    if claim_count == 2 && reverse != (partition == 1) {
                        continue;
                    }
                    let (from, to) = (Coord3D::new(4.0, 1.0, 0.0), Coord3D::new(4.0, 2.0, 0.0));
                    observations.push(
                        PartitionBorderHalfEdge::new(
                            partition,
                            10_000 + usize::from(reverse),
                            None,
                            if partition == 0 {
                                crate::graph::partition_border::PartitionBorderSide::MaxX
                            } else {
                                crate::graph::partition_border::PartitionBorderSide::MinX
                            },
                            if reverse { to } else { from },
                            if reverse { from } else { to },
                            [99],
                        )
                        .unwrap(),
                    );
                }
                forged
                    .partitions
                    .get_mut(&partition)
                    .unwrap()
                    .atomic_observations = crate::tiling::partition_atomic_observations(
                    observations
                        .iter()
                        .filter(|observation| observation.partition_id == partition),
                );
            }
            let forged_graph = rebuild(
                graph.local_face_graphs().to_vec(),
                observations,
                graph
                    .global_non_polygon_extraction_payload_candidates()
                    .to_vec(),
            );
            let forged_evidence =
                prepare_non_polygon_evidence(&forged, &tiles, &refs, domain, &options, &policy)
                    .unwrap()
                    .unwrap();
            let forged_assembly =
                assemble_cross_core_non_polygon(&forged_evidence, &forged, &tiles, domain, &policy)
                    .unwrap()
                    .unwrap();
            let forged_consumer = consume_global_non_polygon(
                &forged_evidence,
                None,
                Some(&forged_assembly),
                &forged,
                &tiles,
                domain,
                &policy,
            )
            .unwrap()
            .unwrap();
            for refresh_cache in [false, true] {
                if refresh_cache {
                    forged.physical_spans =
                        PartitionMosaic::physical_spans(&forged.partitions, &policy).unwrap();
                }
                assert!(
                    prove_cross_core_physical_polygons(
                        &PartitionCrossCorePhysicalInputs {
                            graph: &forged_graph,
                            mosaic: &forged,
                            evidence: &forged_evidence,
                            assembly: &forged_assembly,
                            ..inputs
                        },
                        &forged_consumer,
                        &policy
                    )
                    .unwrap()
                    .is_none(),
                    "claims={claim_count} refreshed_cache={refresh_cache}"
                );
            }
        }
        for (first_partition, second_partition, first_side, second_side) in [
            (
                0,
                1,
                crate::graph::partition_border::PartitionBorderSide::MaxX,
                crate::graph::partition_border::PartitionBorderSide::MinX,
            ),
            (
                0,
                1,
                crate::graph::partition_border::PartitionBorderSide::MaxY,
                crate::graph::partition_border::PartitionBorderSide::MinY,
            ),
            (
                0,
                2,
                crate::graph::partition_border::PartitionBorderSide::MaxX,
                crate::graph::partition_border::PartitionBorderSide::MinX,
            ),
        ] {
            let mut forged = mosaic.clone();
            let mut observations = graph.border_observations().cloned().collect::<Vec<_>>();
            for (partition, side, reverse) in [
                (first_partition, first_side, false),
                (second_partition, second_side, true),
            ] {
                let (local, edge) = graph
                    .local_face_graphs()
                    .iter()
                    .filter(|local| local.partition_id == partition)
                    .flat_map(|local| local.directed_edges.iter().map(move |edge| (local, edge)))
                    .find(|(_, edge)| {
                        let from = edge.from.xy_bits().map(f64::from_bits);
                        let to = edge.to.xy_bits().map(f64::from_bits);
                        from[0] == 9.0 && to[0] == 9.0 && ((from[1] < to[1]) != reverse)
                    })
                    .unwrap();
                let xyz = |key: crate::graph::partition_border::PartitionBorderNodeKey, z| {
                    let [x, y] = key.xy_bits().map(f64::from_bits);
                    Coord3D::new(x, y, f64::from_bits(z))
                };
                let mut observation = PartitionBorderHalfEdge::new_with_face_ref(
                    partition,
                    edge.local_dir_edge_id,
                    edge.face_ref,
                    side,
                    xyz(edge.from, edge.from_z_bits),
                    xyz(edge.to, edge.to_z_bits),
                    edge.source_line_ids.iter().copied(),
                )
                .unwrap();
                observation.component_id = local.component_id;
                observation.local_face_successor = edge.local_face_successor;
                observation.local_face_is_unbounded = edge.local_face_is_unbounded;
                observations.push(observation);
                forged
                    .partitions
                    .get_mut(&partition)
                    .unwrap()
                    .atomic_observations = crate::tiling::partition_atomic_observations(
                    observations
                        .iter()
                        .filter(|observation| observation.partition_id == partition),
                );
            }
            forged.physical_spans =
                PartitionMosaic::physical_spans(&forged.partitions, &policy).unwrap();
            assert!(forged
                .physical_span_evidence()
                .iter()
                .all(|span| span.status == crate::tiling::PartitionPhysicalSpanStatusV1::Valid));
            let forged_graph = rebuild(
                graph.local_face_graphs().to_vec(),
                observations,
                graph
                    .global_non_polygon_extraction_payload_candidates()
                    .to_vec(),
            );
            let forged_evidence =
                prepare_non_polygon_evidence(&forged, &tiles, &refs, domain, &options, &policy)
                    .unwrap()
                    .unwrap();
            let forged_assembly =
                assemble_cross_core_non_polygon(&forged_evidence, &forged, &tiles, domain, &policy)
                    .unwrap()
                    .unwrap();
            let forged_consumer = consume_global_non_polygon(
                &forged_evidence,
                None,
                Some(&forged_assembly),
                &forged,
                &tiles,
                domain,
                &policy,
            )
            .unwrap()
            .unwrap();
            let forged_inputs = PartitionCrossCorePhysicalInputs {
                graph: &forged_graph,
                mosaic: &forged,
                evidence: &forged_evidence,
                assembly: &forged_assembly,
                ..inputs
            };
            assert!(prove_cross_core_physical_polygons(&forged_inputs, &forged_consumer, &policy).unwrap().is_none(), "native/source-correct false side {first_partition}:{first_side:?} {second_partition}:{second_side:?}");
        }
        for mutation in 0..9 {
            let mut locals = graph.local_face_graphs().to_vec();
            let mut observations = graph.border_observations().cloned().collect::<Vec<_>>();
            let mut rows = graph
                .global_non_polygon_extraction_payload_candidates()
                .to_vec();
            match mutation {
                0 => {
                    locals.pop();
                }
                1 => locals[0].directed_edges[0].source_line_ids.push(99),
                2 => locals[0].directed_edges[0].from_z_bits = 1.0f64.to_bits(),
                3 => locals[0].directed_edges[0].symmetric_local_dir_edge_id = usize::MAX,
                4 => locals[0].directed_edges[0].local_face_successor = Some(usize::MAX),
                5 => observations.push(
                    PartitionBorderHalfEdge::new(
                        0,
                        usize::MAX,
                        None,
                        crate::graph::partition_border::PartitionBorderSide::MaxX,
                        Coord3D::new(4.0, 1.0, 0.0),
                        Coord3D::new(4.0, 2.0, 0.0),
                        [99],
                    )
                    .unwrap(),
                ),
                6 => {
                    rows.pop();
                }
                7 => rows[0].coordinate_bits[0][2] = 1.0f64.to_bits(),
                _ => {
                    rows.swap(0, 1);
                }
            }
            let changed = rebuild(locals, observations, rows);
            assert!(
                prove_cross_core_physical_polygons(
                    &PartitionCrossCorePhysicalInputs {
                        graph: &changed,
                        ..inputs
                    },
                    &consumer,
                    &policy
                )
                .unwrap()
                .is_none(),
                "native mutation {mutation}"
            );
        }
        let mut changed_consumer = consumer.clone();
        changed_consumer.output.invalid_rings.pop();
        assert!(
            prove_cross_core_physical_polygons(&inputs, &changed_consumer, &policy)
                .unwrap()
                .is_none()
        );
        let mut changed_assembly = assembly.clone();
        changed_assembly.payloads[0].spans.pop();
        assert!(prove_cross_core_physical_polygons(
            &PartitionCrossCorePhysicalInputs {
                assembly: &changed_assembly,
                ..inputs
            },
            &consumer,
            &policy
        )
        .unwrap()
        .is_none());
        let mut stale = mosaic.clone();
        stale
            .partitions
            .get_mut(&0)
            .unwrap()
            .generation
            .retry_attempt += 1;
        let changed_options = PolygonizerOptions {
            input_profile_id: Some("other".to_string()),
            ..options.clone()
        };
        for equivalent_options in [
            PolygonizerOptions {
                precision_model: PrecisionModel::FixedGrid { grid_size: 0.125 },
                ..options.clone()
            },
            PolygonizerOptions {
                extract_only_polygonal: true,
                ..options.clone()
            },
            PolygonizerOptions {
                output_filter: crate::OutputFilterOptions {
                    minimum_face_area: Some(0.0625),
                },
                ..options.clone()
            },
        ] {
            let mut reference = Polygonizer::with_options(equivalent_options.clone());
            for geometry in &geometries {
                reference.add_borrowed_geometry(geometry);
            }
            let equivalent_output = reference.polygonize().unwrap();
            assert!(
                compare_stitched_output_with_reference(&output, &equivalent_output, &policy)
                    .unwrap()
                    .ready
            );
            assert!(prove_cross_core_physical_polygons(
                &PartitionCrossCorePhysicalInputs {
                    options: &equivalent_options,
                    ..inputs
                },
                &consumer,
                &policy
            )
            .unwrap()
            .is_none());
        }
        let mut changed_stats = stats;
        changed_stats.coordinate_count += 1;
        for changed in [
            PartitionCrossCorePhysicalInputs {
                mosaic: &stale,
                ..inputs
            },
            PartitionCrossCorePhysicalInputs {
                options: &changed_options,
                ..inputs
            },
            PartitionCrossCorePhysicalInputs {
                non_polygon_stats: changed_stats,
                ..inputs
            },
        ] {
            assert!(
                prove_cross_core_physical_polygons(&changed, &consumer, &policy)
                    .unwrap()
                    .is_none()
            );
        }
        for limited in [
            ExecutionPolicy {
                max_graph_edges: Some(0),
                ..Default::default()
            },
            ExecutionPolicy {
                max_rings: Some(2),
                ..Default::default()
            },
            ExecutionPolicy {
                max_output_polygons: Some(1),
                ..Default::default()
            },
            ExecutionPolicy {
                max_output_coordinates: Some(1),
                ..Default::default()
            },
            ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                prove_cross_core_physical_polygons(&inputs, &consumer, &limited),
                Err(PolygonizeError::ResourceLimitExceeded { .. })
            ));
        }
        for cancel_at in [0, 1, 24] {
            let token = CancellationToken::new();
            let cancelled = ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, cancel_at)),
                ..Default::default()
            };
            assert!(matches!(
                prove_cross_core_physical_polygons(&inputs, &consumer, &cancelled),
                Err(PolygonizeError::Cancelled { .. })
            ));
        }
        let mut aggregate = graph.clone();
        aggregate
            .retain_global_non_polygon_extraction_payloads(
                &policy,
                0,
                vec![vec![Coord3D::new(1.0, 1.0, 0.0); 1_000]; 1_001],
                Vec::new(),
                Vec::new(),
            )
            .unwrap();
        let cancellable = ExecutionPolicy {
            cancellation_token: Some(CancellationToken::new()),
            ..Default::default()
        };
        assert!(matches!(
            prove_cross_core_physical_polygons(
                &PartitionCrossCorePhysicalInputs {
                    graph: &aggregate,
                    ..inputs
                },
                &consumer,
                &cancellable
            ),
            Err(PolygonizeError::ResourceLimitExceeded { .. })
        ));
        assert_eq!(graph, before);
        assert_eq!(mosaic.fingerprint_sha256(), before_mosaic);
        assert_eq!(
            prove_cross_core_physical_polygons(&inputs, &consumer, &policy)
                .unwrap()
                .as_ref(),
            Some(&proof)
        );
        let repeated =
            integrate_cross_core_component_candidate(&inputs, &proof, &consumer, &policy)
                .unwrap()
                .unwrap();
        assert_eq!(
            serde_json::to_value((
                &repeated.polygons,
                &repeated.dangles,
                &repeated.cut_edges,
                &repeated.invalid_rings
            ))
            .unwrap(),
            serde_json::to_value((
                &output.polygons,
                &output.dangles,
                &output.cut_edges,
                &output.invalid_rings
            ))
            .unwrap(),
        );
    }

    #[test]
    fn cross_core_complete_private_candidate_rejects_any_changed_component_atomically() {
        use crate::graph::partition_border::PartitionBorderGraph;
        use crate::tiling::{
            assemble_cross_core_non_polygon, consume_global_non_polygon,
            integrate_private_global_candidate, prepare_non_polygon_evidence,
            PartitionGlobalCandidateExtraction, PartitionGlobalCandidateInputs, PartitionMosaic,
        };
        let geometries = [
            Geometry::LineString(LineString::from(vec![
                (0.25, 0.25),
                (0.5, 0.25),
                (0.5, 0.5),
                (0.25, 0.5),
                (0.25, 0.25),
            ])),
            Geometry::LineString(LineString::from(vec![(3.0, 3.5), (9.0, 3.5)])),
            Geometry::LineString(LineString::from(vec![(0.75, 0.75), (1.0, 0.75)])),
        ];
        let refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        let options = PolygonizerOptions {
            node_input: false,
            input_profile_id: Some("cross-candidate".to_string()),
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            output_filter: crate::OutputFilterOptions {
                minimum_face_area: Some(1.0),
            },
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 4.0)
            .with_buffer(0.0)
            .with_options(options.clone());
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let tiles = tiled.generate_tiles().unwrap();
        let mut graph = PartitionBorderGraph::default();
        let mut mosaic = PartitionMosaic::default();
        for (partition_id, &tile) in tiles.iter().enumerate() {
            let (_, _, _, observations, local_graphs, _, _, _, _, _, _, _, snapshot) = tiled
                .process_tile(partition_id, tile, &tiles, &[], 0.0, 0, None)
                .unwrap();
            for observation in observations {
                graph.insert(observation).unwrap();
            }
            for local in local_graphs {
                graph.insert_local_face_graph(local).unwrap();
            }
            mosaic.replace_partition(snapshot, &policy).unwrap();
        }
        let eligible = mosaic.topology_ready_observation_ids();
        graph
            .apply_topology_ready_face_twins(&eligible, &policy)
            .unwrap();
        graph.reconcile_global_face_edge_map(&policy).unwrap();
        graph
            .reconcile_global_face_nodes(options.z, &policy)
            .unwrap();
        let witness = graph.global_arrangement_witness(&policy).unwrap().unwrap();
        let construction = graph
            .construct_global_arrangement_candidate(&witness, &eligible, true, &policy)
            .unwrap()
            .unwrap();
        let decode = |family: &[Vec<[u64; 3]>]| {
            family
                .iter()
                .map(|bits| {
                    bits.iter()
                        .copied()
                        .map(crate::tiling::coord3d_from_bits)
                        .collect()
                })
                .collect()
        };
        for snapshot in mosaic.partitions.values() {
            graph
                .retain_global_non_polygon_extraction_payloads(
                    &policy,
                    snapshot.partition_id,
                    decode(&snapshot.non_polygon.dangles),
                    decode(&snapshot.non_polygon.cut_edges),
                    decode(&snapshot.non_polygon.invalid_rings),
                )
                .unwrap();
        }
        let stats = graph
            .materialize_global_non_polygon_extraction_payloads(&policy)
            .unwrap();
        assert!(!stats.payload_ready);
        assert_eq!(stats.duplicate_payload_count, 2);
        assert!(graph
            .extract_physical_table_payloads(
                &witness,
                &construction,
                &eligible,
                true,
                stats,
                &policy
            )
            .unwrap()
            .is_none());
        let extraction = graph
            .extract_physical_polygon_payloads(&witness, &construction, &eligible, true, &policy)
            .unwrap()
            .unwrap();
        let evidence =
            prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                .unwrap()
                .unwrap();
        let assembly = assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
            .unwrap()
            .unwrap();
        let consumer = consume_global_non_polygon(
            &evidence,
            None,
            Some(&assembly),
            &mosaic,
            &tiles,
            domain,
            &policy,
        )
        .unwrap()
        .unwrap();
        let inputs = PartitionGlobalCandidateInputs {
            graph: &graph,
            witness: &witness,
            construction: &construction,
            eligible_observation_ids: &eligible,
            span_evidence_ready: true,
            non_polygon_stats: stats,
            evidence: &evidence,
            ownership: None,
            assembly: Some(&assembly),
            mosaic: &mosaic,
            tiles: &tiles,
            domain,
            options: &options,
        };
        let materialize = |inputs: &PartitionGlobalCandidateInputs<'_>,
                           extraction: &crate::graph::partition_border::PartitionBorderPhysicalPolygonExtraction,
                           consumer: &crate::tiling::PartitionGlobalNonPolygonConsumer<'_>,
                           policy: &ExecutionPolicy| {
            integrate_private_global_candidate(
                inputs,
                PartitionGlobalCandidateExtraction::CrossCore(extraction),
                consumer,
                policy,
            )
        };
        let expected = materialize(&inputs, &extraction, &consumer, &policy)
            .unwrap()
            .unwrap();
        assert_eq!(expected.polygons.len(), 0);
        assert_eq!(expected.dangles.len(), 2);
        assert!(
            crate::tiling::compare_stitched_output_with_reference(
                &expected,
                &evidence.reference,
                &policy
            )
            .unwrap()
            .ready
        );
        let graph_before = graph.clone();
        let mosaic_before = mosaic.fingerprint_sha256();
        let mut foreign_graph = PartitionBorderGraph::default();
        for local in graph.local_face_graphs() {
            let mut changed = local.clone();
            for edge in &mut changed.directed_edges {
                edge.source_line_ids = vec![99];
            }
            foreign_graph.insert_local_face_graph(changed).unwrap();
        }
        foreign_graph
            .apply_topology_ready_face_twins(&eligible, &policy)
            .unwrap();
        foreign_graph
            .reconcile_global_face_edge_map(&policy)
            .unwrap();
        foreign_graph
            .reconcile_global_face_nodes(options.z, &policy)
            .unwrap();
        for snapshot in mosaic.partitions.values() {
            foreign_graph
                .retain_global_non_polygon_extraction_payloads(
                    &policy,
                    snapshot.partition_id,
                    decode(&snapshot.non_polygon.dangles),
                    decode(&snapshot.non_polygon.cut_edges),
                    decode(&snapshot.non_polygon.invalid_rings),
                )
                .unwrap();
        }
        let foreign_witness = foreign_graph
            .global_arrangement_witness(&policy)
            .unwrap()
            .unwrap();
        let foreign_construction = foreign_graph
            .construct_global_arrangement_candidate(&foreign_witness, &eligible, true, &policy)
            .unwrap()
            .unwrap();
        let foreign_extraction = foreign_graph
            .extract_physical_polygon_payloads(
                &foreign_witness,
                &foreign_construction,
                &eligible,
                true,
                &policy,
            )
            .unwrap()
            .unwrap();
        let foreign_inputs = PartitionGlobalCandidateInputs {
            graph: &foreign_graph,
            witness: &foreign_witness,
            construction: &foreign_construction,
            ..inputs
        };
        assert!(
            materialize(&foreign_inputs, &foreign_extraction, &consumer, &policy)
                .unwrap()
                .is_none()
        );
        for mutation in 0..7 {
            let mut changed = extraction.clone();
            match mutation {
                0 => {
                    changed.local_aliases.pop();
                }
                1 => changed.ring_payloads[0].coordinate_bits[0][2] = 1.0f64.to_bits(),
                2 => changed.ring_payloads[0].representative_line_ids[0] = 99,
                3 => changed.ring_payloads[0].source_line_ids.push(99),
                4 => changed.ring_payloads[0].local_dir_edge_ids_by_edge[0].clear(),
                5 => changed.local_aliases[0].from_z_bits = 1.0f64.to_bits(),
                _ => {
                    changed.ring_payloads.pop();
                }
            }
            assert!(
                materialize(&inputs, &changed, &consumer, &policy)
                    .unwrap()
                    .is_none(),
                "physical mutation {mutation}"
            );
        }
        for mutation in 0..5 {
            let mut changed = consumer.clone();
            match mutation {
                0 => changed.input_fingerprint_sha256 = "stale",
                1 => changed.payloads.clear(),
                2 => changed.payloads[0].local_payload_aliases = &[],
                3 => changed.payloads[0].global_payload_count += 1,
                _ => changed.output.dangles[0][0].z = 1.0,
            }
            assert!(
                materialize(&inputs, &extraction, &changed, &policy)
                    .unwrap()
                    .is_none(),
                "consumer mutation {mutation}"
            );
        }
        for mutation in 0..4 {
            let mut changed = assembly.clone();
            match mutation {
                0 => {
                    changed.payloads[0].spans.pop();
                }
                1 => changed.payloads[0].spans[0].owner_partition_id = 2,
                2 => changed.payloads[0].spans[0]
                    .boundary_segment_aliases
                    .clear(),
                _ => changed.whole_core_payloads.clear(),
            }
            let changed_inputs = PartitionGlobalCandidateInputs {
                assembly: Some(&changed),
                ..inputs
            };
            assert!(
                materialize(&changed_inputs, &extraction, &consumer, &policy)
                    .unwrap()
                    .is_none(),
                "assembly mutation {mutation}"
            );
        }
        for mutation in 0..3 {
            let mut changed_graph = graph.clone();
            if mutation == 0 {
                changed_graph = PartitionBorderGraph::default();
                for local in graph.local_face_graphs() {
                    changed_graph
                        .insert_local_face_graph(local.clone())
                        .unwrap();
                }
                changed_graph
                    .apply_topology_ready_face_twins(&eligible, &policy)
                    .unwrap();
                changed_graph
                    .reconcile_global_face_edge_map(&policy)
                    .unwrap();
                changed_graph
                    .reconcile_global_face_nodes(options.z, &policy)
                    .unwrap();
                assert!(changed_graph
                    .global_non_polygon_extraction_payload_candidates()
                    .is_empty());
            } else {
                let mut dangle = decode(&mosaic.partitions[&0].non_polygon.dangles);
                if mutation == 2 {
                    dangle[0][0].z = 1.0;
                }
                changed_graph
                    .retain_global_non_polygon_extraction_payloads(
                        &policy,
                        0,
                        dangle,
                        Vec::new(),
                        Vec::new(),
                    )
                    .unwrap();
            }
            let changed_inputs = PartitionGlobalCandidateInputs {
                graph: &changed_graph,
                ..inputs
            };
            assert!(
                materialize(&changed_inputs, &extraction, &consumer, &policy)
                    .unwrap()
                    .is_none(),
                "raw mutation {mutation}"
            );
        }
        for mutation in 0..4 {
            let mut changed_stats = stats;
            match mutation {
                0 => changed_stats.payload_ready = true,
                1 => changed_stats.duplicate_payload_count = 0,
                2 => changed_stats.coordinate_count += 1,
                _ => changed_stats.evidence_mismatch_count = 0,
            }
            let changed_inputs = PartitionGlobalCandidateInputs {
                non_polygon_stats: changed_stats,
                ..inputs
            };
            assert!(
                materialize(&changed_inputs, &extraction, &consumer, &policy)
                    .unwrap()
                    .is_none()
            );
        }
        let mut stale = mosaic.clone();
        stale
            .partitions
            .get_mut(&1)
            .unwrap()
            .generation
            .retry_attempt += 1;
        let mut changed_options = options.clone();
        changed_options.input_profile_id = Some("other-profile".to_string());
        let mut changed_construction = construction.clone();
        changed_construction
            .faces
            .as_mut()
            .unwrap()
            .unbounded_face_id = usize::MAX;
        let mut changed_tiles = tiles.clone();
        changed_tiles.swap(0, 1);
        for changed_inputs in [
            PartitionGlobalCandidateInputs {
                mosaic: &stale,
                ..inputs
            },
            PartitionGlobalCandidateInputs {
                options: &changed_options,
                ..inputs
            },
            PartitionGlobalCandidateInputs {
                construction: &changed_construction,
                ..inputs
            },
            PartitionGlobalCandidateInputs {
                tiles: &changed_tiles,
                ..inputs
            },
            PartitionGlobalCandidateInputs {
                span_evidence_ready: false,
                ..inputs
            },
            PartitionGlobalCandidateInputs {
                assembly: None,
                ..inputs
            },
        ] {
            assert!(
                materialize(&changed_inputs, &extraction, &consumer, &policy)
                    .unwrap()
                    .is_none()
            );
        }
        for limited in [
            ExecutionPolicy {
                max_output_coordinates: Some(8),
                ..Default::default()
            },
            ExecutionPolicy {
                max_rings: Some(1),
                ..Default::default()
            },
            ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            },
            ExecutionPolicy {
                max_graph_edges: Some(0),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                materialize(&inputs, &extraction, &consumer, &limited),
                Err(PolygonizeError::ResourceLimitExceeded { .. })
            ));
        }
        for cancel_at in [0, 1] {
            let token = CancellationToken::new();
            let cancelled = ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, cancel_at)),
                ..Default::default()
            };
            assert!(matches!(
                materialize(&inputs, &extraction, &consumer, &cancelled),
                Err(PolygonizeError::Cancelled { .. })
            ));
        }
        assert_eq!(graph, graph_before);
        assert_eq!(mosaic.fingerprint_sha256(), mosaic_before);
        let repeated = materialize(&inputs, &extraction, &consumer, &policy)
            .unwrap()
            .unwrap();
        assert_eq!(repeated.dangles, expected.dangles);
        assert!(
            crate::tiling::compare_stitched_output_with_reference(
                &repeated,
                &evidence.reference,
                &policy
            )
            .unwrap()
            .ready
        );
    }

    #[test]
    fn complete_private_candidate_revalidates_both_components_and_rejects_a_bad_member() {
        use crate::graph::partition_border::PartitionBorderGraph;
        use crate::tiling::{
            consume_global_non_polygon, integrate_private_global_candidate,
            prepare_non_polygon_evidence, prove_whole_non_polygon_ownership,
            PartitionGlobalCandidateExtraction, PartitionGlobalCandidateInputs, PartitionMosaic,
        };
        let geometries = [
            Geometry::LineString(LineString::from(vec![
                (1.0, 1.0),
                (3.0, 1.0),
                (3.0, 3.0),
                (1.0, 3.0),
                (1.0, 1.0),
            ])),
            Geometry::LineString(LineString::from(vec![(3.25, 3.25), (3.5, 3.25)])),
        ];
        let refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 4.0 });
        let options = PolygonizerOptions {
            input_profile_id: Some("complete-candidate".to_string()),
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 4.0).with_options(options.clone());
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let tiles = tiled.generate_tiles().unwrap();
        let (_, _, _, observations, local_graphs, _, _, _, _, _, _, _, snapshot) = tiled
            .process_tile(0, domain, &tiles, &[], 0.0, 0, None)
            .unwrap();
        let mut graph = PartitionBorderGraph::default();
        for observation in observations {
            graph.insert(observation).unwrap();
        }
        for local in local_graphs {
            graph.insert_local_face_graph(local).unwrap();
        }
        let mut mosaic = PartitionMosaic::default();
        mosaic.replace_partition(snapshot.clone(), &policy).unwrap();
        let eligible = mosaic.topology_ready_observation_ids();
        graph
            .apply_topology_ready_face_twins(&eligible, &policy)
            .unwrap();
        graph.reconcile_global_face_edge_map(&policy).unwrap();
        graph
            .reconcile_global_face_nodes(options.z, &policy)
            .unwrap();
        let witness = graph.global_arrangement_witness(&policy).unwrap().unwrap();
        let construction = graph
            .construct_global_arrangement_candidate(&witness, &eligible, true, &policy)
            .unwrap()
            .unwrap();
        let decode = |family: &[Vec<[u64; 3]>]| {
            family
                .iter()
                .map(|bits| {
                    bits.iter()
                        .copied()
                        .map(crate::tiling::coord3d_from_bits)
                        .collect()
                })
                .collect()
        };
        graph
            .retain_global_non_polygon_extraction_payloads(
                &policy,
                0,
                decode(&snapshot.non_polygon.dangles),
                decode(&snapshot.non_polygon.cut_edges),
                decode(&snapshot.non_polygon.invalid_rings),
            )
            .unwrap();
        let stats = graph
            .materialize_global_non_polygon_extraction_payloads(&policy)
            .unwrap();
        let extraction = graph
            .extract_physical_table_payloads(
                &witness,
                &construction,
                &eligible,
                true,
                stats,
                &policy,
            )
            .unwrap()
            .unwrap();
        let evidence =
            prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                .unwrap()
                .unwrap();
        let ownership = prove_whole_non_polygon_ownership(&evidence, &tiles, domain, &policy)
            .unwrap()
            .unwrap();
        let consumer = consume_global_non_polygon(
            &evidence,
            Some(&ownership),
            None,
            &mosaic,
            &tiles,
            domain,
            &policy,
        )
        .unwrap()
        .unwrap();
        let inputs = PartitionGlobalCandidateInputs {
            graph: &graph,
            witness: &witness,
            construction: &construction,
            eligible_observation_ids: &eligible,
            span_evidence_ready: true,
            non_polygon_stats: stats,
            evidence: &evidence,
            ownership: Some(&ownership),
            assembly: None,
            mosaic: &mosaic,
            tiles: &tiles,
            domain,
            options: &options,
        };
        let expected = integrate_private_global_candidate(
            &inputs,
            PartitionGlobalCandidateExtraction::WholeCore(&extraction),
            &consumer,
            &policy,
        )
        .unwrap()
        .unwrap();
        assert_eq!(expected.polygons.len(), 1);
        assert_eq!(expected.dangles.len(), 1);
        assert_eq!(
            expected.polygons[0]
                .provenance
                .as_ref()
                .unwrap()
                .input_profile_id
                .as_deref(),
            Some("complete-candidate")
        );
        assert!(
            crate::tiling::compare_stitched_output_with_reference(
                &expected,
                &evidence.reference,
                &policy
            )
            .unwrap()
            .ready
        );
        let before = graph.clone();
        let mosaic_before = mosaic.fingerprint_sha256();
        for mutation in 0..6 {
            let mut changed = extraction.clone();
            match mutation {
                0 => {
                    changed.local_aliases.pop();
                }
                1 => changed.ring_payloads[0].coordinate_bits[0][2] = 1.0f64.to_bits(),
                2 => changed.ring_payloads[0].representative_line_ids[0] = 999,
                3 => changed.ring_payloads[0].source_line_ids.push(999),
                4 => changed.ring_payloads[0].local_dir_edge_ids_by_edge[0].clear(),
                _ => {
                    changed.non_polygon_payloads.pop();
                }
            }
            assert!(
                integrate_private_global_candidate(
                    &inputs,
                    PartitionGlobalCandidateExtraction::WholeCore(&changed),
                    &consumer,
                    &policy
                )
                .unwrap()
                .is_none(),
                "physical mutation {mutation}"
            );
        }
        for mutation in 0..5 {
            let mut changed = consumer.clone();
            match mutation {
                0 => changed.input_fingerprint_sha256 = "stale",
                1 => changed.payloads.clear(),
                2 => changed.payloads[0].local_payload_aliases = &[],
                3 => changed.payloads[0].global_payload_count += 1,
                _ => changed.output.dangles[0][0].z = 1.0,
            }
            assert!(
                integrate_private_global_candidate(
                    &inputs,
                    PartitionGlobalCandidateExtraction::WholeCore(&extraction),
                    &changed,
                    &policy
                )
                .unwrap()
                .is_none(),
                "consumer mutation {mutation}"
            );
        }
        let mut blocked = PartitionGlobalCandidateInputs {
            span_evidence_ready: false,
            ..inputs
        };
        assert!(integrate_private_global_candidate(
            &blocked,
            PartitionGlobalCandidateExtraction::WholeCore(&extraction),
            &consumer,
            &policy
        )
        .unwrap()
        .is_none());
        blocked.span_evidence_ready = true;
        blocked.non_polygon_stats.duplicate_payload_count = 1;
        assert!(integrate_private_global_candidate(
            &blocked,
            PartitionGlobalCandidateExtraction::WholeCore(&extraction),
            &consumer,
            &policy
        )
        .unwrap()
        .is_none());
        let mut changed_options = options.clone();
        changed_options.input_profile_id = Some("other-profile".to_string());
        blocked.non_polygon_stats = stats;
        blocked.options = &changed_options;
        assert!(integrate_private_global_candidate(
            &blocked,
            PartitionGlobalCandidateExtraction::WholeCore(&extraction),
            &consumer,
            &policy
        )
        .unwrap()
        .is_none());
        let mut stale = mosaic.clone();
        stale
            .partitions
            .get_mut(&0)
            .unwrap()
            .generation
            .retry_attempt += 1;
        blocked.options = &options;
        blocked.mosaic = &stale;
        assert!(integrate_private_global_candidate(
            &blocked,
            PartitionGlobalCandidateExtraction::WholeCore(&extraction),
            &consumer,
            &policy
        )
        .unwrap()
        .is_none());
        blocked.mosaic = &mosaic;
        let mut stale_construction = construction.clone();
        stale_construction.faces.as_mut().unwrap().unbounded_face_id = usize::MAX;
        blocked.construction = &stale_construction;
        assert!(integrate_private_global_candidate(
            &blocked,
            PartitionGlobalCandidateExtraction::WholeCore(&extraction),
            &consumer,
            &policy
        )
        .unwrap()
        .is_none());
        blocked.construction = &construction;
        for limited in [
            ExecutionPolicy {
                max_output_coordinates: Some(6),
                ..Default::default()
            },
            ExecutionPolicy {
                max_rings: Some(1),
                ..Default::default()
            },
            ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                integrate_private_global_candidate(
                    &blocked,
                    PartitionGlobalCandidateExtraction::WholeCore(&extraction),
                    &consumer,
                    &limited
                ),
                Err(PolygonizeError::ResourceLimitExceeded { .. })
            ));
        }
        for cancel_at in [0, 1] {
            let token = CancellationToken::new();
            let cancelled = ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, cancel_at)),
                ..Default::default()
            };
            assert!(matches!(
                integrate_private_global_candidate(
                    &blocked,
                    PartitionGlobalCandidateExtraction::WholeCore(&extraction),
                    &consumer,
                    &cancelled
                ),
                Err(PolygonizeError::Cancelled { .. })
            ));
        }
        assert_eq!(graph, before);
        assert_eq!(mosaic.fingerprint_sha256(), mosaic_before);
        assert!(
            crate::tiling::compare_stitched_output_with_reference(
                &integrate_private_global_candidate(
                    &blocked,
                    PartitionGlobalCandidateExtraction::WholeCore(&extraction),
                    &consumer,
                    &policy
                )
                .unwrap()
                .unwrap(),
                &evidence.reference,
                &policy
            )
            .unwrap()
            .ready
        );
    }

    #[test]
    fn physical_output_conformance_declines_holes_before_filtering() {
        let geometries = [
            Geometry::LineString(LineString::from(vec![
                (1.0, 1.0),
                (7.0, 1.0),
                (7.0, 7.0),
                (1.0, 7.0),
                (1.0, 1.0),
            ])),
            Geometry::LineString(LineString::from(vec![
                (2.0, 2.0),
                (3.0, 2.0),
                (3.0, 3.0),
                (2.0, 3.0),
                (2.0, 2.0),
            ])),
        ];
        for extract_only_polygonal in [false, true] {
            let mut tiled = TiledPolygonizer::new(
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 8.0 }),
                8.0,
            )
            .with_buffer(0.0)
            .with_options(PolygonizerOptions {
                extract_only_polygonal,
                output_filter: crate::OutputFilterOptions {
                    minimum_face_area: Some(2.0),
                },
                ..Default::default()
            })
            .with_untiled_equivalence_check();
            for geometry in &geometries {
                tiled.add_geometry(geometry);
            }
            let ordinary = tiled.polygonize().unwrap();
            let traced = tiled
                .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                .unwrap();
            let conformance = &traced
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_conformance")
                .unwrap()
                .payload;
            assert_eq!(conformance["candidate_built"], false);
            assert_eq!(conformance["equivalence_checked"], false);
            assert_eq!(conformance["equivalence_ready"], false);
            assert_eq!(conformance["output_selectable"], false);
            assert!(ordinary.stitched_output.is_none());
            assert!(traced.result.stitched_output.is_none());
        }
    }

    #[test]
    fn partition_source_inventory_bounds_work_before_fingerprint_allocation() {
        use crate::options::MAX_UNCANCELLABLE_SORT_ITEMS;
        use crate::tiling::partition_source_segments;

        // Borrow the same small geometry repeatedly; source identities still
        // distinguish each original input index without a huge input fixture.
        let chunk = Geometry::LineString(LineString::from(vec![(0.0, 0.0); 1_001]));
        let last = Geometry::LineString(LineString::from(vec![(0.0, 0.0); 2]));
        let mut geometries = vec![(&chunk, None); MAX_UNCANCELLABLE_SORT_ITEMS / 1_000];
        geometries.push((&last, None));
        let indices = (0..geometries.len()).collect::<Vec<_>>();
        let output =
            partition_source_segments(&geometries, &indices, &ExecutionPolicy::default()).unwrap();
        assert_eq!(output.len(), MAX_UNCANCELLABLE_SORT_ITEMS + 1);
        assert_eq!(output.last().unwrap().geometry_index, geometries.len() - 1);
        drop(output);

        let invalid = Geometry::LineString(LineString::from(vec![(f64::NAN, 0.0); 1_001]));
        geometries[..indices.len() - 1].fill((&invalid, None));
        let policy = ExecutionPolicy {
            cancellation_token: Some(CancellationToken::new()),
            ..Default::default()
        };
        // If decoding runs before preflight, this returns InvalidGeometry at
        // the first fingerprint instead of rejecting the full source count.
        assert!(matches!(
            partition_source_segments(&geometries, &indices, &policy),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: MAX_UNCANCELLABLE_SORT_ITEMS,
                observed,
            }) if stage == "partition_source_preflight"
                && observed == MAX_UNCANCELLABLE_SORT_ITEMS + 1
        ));
    }

    #[test]
    fn partition_source_inventory_polls_preflight_copy_and_fingerprints() {
        use crate::tiling::{partition_source_segment_sink, partition_source_segments};
        let line = Geometry::LineString(LineString::from(vec![(0.0, 0.0); 513]));
        let short = Geometry::LineString(LineString::from(vec![(0.0, 0.0); 2]));
        let cancelled_policy = || {
            let token = CancellationToken::new();
            ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, 256)),
                ..Default::default()
            }
        };
        let geometries = vec![(&short, None); 300];
        let indices = (0..geometries.len()).collect::<Vec<_>>();
        assert!(matches!(
            partition_source_segments(&geometries, &indices, &cancelled_policy()),
            Err(PolygonizeError::Cancelled { ref stage })
                if stage == "partition_source_preflight"
        ));
        let empty = Geometry::GeometryCollection(geo::GeometryCollection(Vec::new()));
        let empty_geometries = vec![(&empty, None); 300];
        assert!(matches!(
            partition_source_segments(&empty_geometries, &indices, &cancelled_policy()),
            Err(PolygonizeError::Cancelled { ref stage })
                if stage == "partition_source_preflight"
        ));
        assert!(matches!(
            partition_source_segments(&[(&line, None)], &[0], &cancelled_policy()),
            Err(PolygonizeError::Cancelled { ref stage })
                if stage == "partition_source_segments"
        ));
        let sink =
            partition_source_segment_sink(&[(&line, None)], &[0], &ExecutionPolicy::default())
                .unwrap();
        assert!(matches!(
            sink.snapshot_segments(&cancelled_policy()),
            Err(PolygonizeError::Cancelled { ref stage })
                if stage == "partition_source_fingerprints"
        ));
        assert_eq!(
            sink.snapshot_segments(&ExecutionPolicy::default())
                .unwrap()
                .len(),
            512
        );
    }

    #[test]
    fn partition_source_preflight_preserves_supported_geometry_inventory() {
        use crate::tiling::partition_source_segments;
        let line = LineString::from(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]);
        let ring = LineString::from(vec![(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 0.0)]);
        let polygon = geo::Polygon::new(ring.clone(), vec![ring]);
        let geometry = Geometry::GeometryCollection(geo::GeometryCollection(vec![
            Geometry::LineString(LineString::new(Vec::new())),
            Geometry::LineString(line.clone()),
            Geometry::MultiLineString(MultiLineString(vec![line])),
            Geometry::Polygon(polygon.clone()),
            Geometry::MultiPolygon(geo::MultiPolygon(vec![polygon])),
            Geometry::GeometryCollection(geo::GeometryCollection(Vec::new())),
            Geometry::Point(geo::Point::new(3.0, 4.0)),
        ]));
        let input = [(&geometry, None)];
        let expected =
            partition_source_segments(&input, &[0], &ExecutionPolicy::default()).unwrap();
        let policy = ExecutionPolicy {
            cancellation_token: Some(CancellationToken::new()),
            ..Default::default()
        };
        assert_eq!(expected.len(), 16);
        assert_eq!(
            partition_source_segments(&input, &[0], &policy).unwrap(),
            expected
        );
        assert!(partition_source_segments(&input, &[], &policy)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn global_non_polygon_ownership_proves_complete_dangle_and_cut_ledgers() {
        use crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind;
        use crate::tiling::{prove_global_non_polygon_ownership, PartitionMosaic};
        let cases = [
            (
                vec![Geometry::LineString(LineString::from(vec![
                    (1.0, 1.0),
                    (2.0, 1.0),
                ]))],
                PartitionBorderGlobalNonPolygonPayloadKind::Dangle,
                4.0,
            ),
            (
                vec![
                    Geometry::LineString(LineString::from(vec![
                        (1.0, 1.0),
                        (2.0, 1.0),
                        (2.0, 1.5),
                        (2.0, 2.0),
                        (1.0, 2.0),
                        (1.0, 1.0),
                    ])),
                    Geometry::LineString(LineString::from(vec![
                        (4.0, 1.0),
                        (5.0, 1.0),
                        (5.0, 2.0),
                        (4.0, 2.0),
                        (4.0, 1.5),
                        (4.0, 1.0),
                    ])),
                    Geometry::LineString(LineString::from(vec![(2.0, 1.5), (4.0, 1.5)])),
                ],
                PartitionBorderGlobalNonPolygonPayloadKind::CutEdge,
                8.0,
            ),
        ];
        let policy = ExecutionPolicy::default();
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 8.0 });
        for (mut geometries, kind, tile_size) in cases {
            let mut signatures = Vec::new();
            for reverse in [false, true] {
                if reverse {
                    geometries.reverse();
                    for geometry in &mut geometries {
                        if let Geometry::LineString(line) = geometry {
                            line.0.reverse();
                        }
                    }
                }
                let geometry_refs = geometries
                    .iter()
                    .map(|geometry| (geometry, None))
                    .collect::<Vec<_>>();
                let mut tiled = TiledPolygonizer::new(domain, tile_size)
                    .with_buffer(0.0)
                    .with_untiled_equivalence_check();
                for geometry in &geometries {
                    tiled.add_geometry(geometry);
                }
                let traced = tiled
                    .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                    .unwrap();
                let mut mosaic = PartitionMosaic::default();
                for snapshot in traced.result.partition_snapshots.iter().rev() {
                    mosaic.replace_partition(snapshot.clone(), &policy).unwrap();
                }
                let before = mosaic.fingerprint_sha256();
                let (proof, reference) = prove_global_non_polygon_ownership(
                    &mosaic,
                    &tiled.generate_tiles().unwrap(),
                    &geometry_refs,
                    domain,
                    &options,
                    &policy,
                )
                .unwrap()
                .unwrap();
                assert_eq!(proof.payloads.len(), 1);
                assert_eq!(proof.payloads[0].payload.kind, kind);
                assert_eq!(proof.payloads[0].payload.partition_id, 0);
                assert!(!proof.payloads[0].edges.is_empty());
                for edge in &proof.payloads[0].edges {
                    assert!(!edge.local_segment_aliases.is_empty());
                    assert_eq!(edge.segment.source_line_ids, vec![0]);
                    for &[partition, local] in &edge.local_segment_aliases {
                        assert_eq!(
                            edge.segment,
                            mosaic.partitions[&partition].local_noded_segments[local]
                        );
                    }
                }
                assert_eq!(reference.dangles.len() + reference.cut_edges.len(), 1);
                let event = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_global_non_polygon_ownership")
                    .unwrap()
                    .payload;
                assert_eq!(event["proven"], true);
                assert_eq!(event["proof"], serde_json::to_value(&proof).unwrap());
                assert_eq!(event["output_selectable"], false);
                assert!(traced.result.stitched_output.is_none());
                assert_eq!(before, mosaic.fingerprint_sha256());
                signatures.push(proof.payloads);
            }
            assert_eq!(signatures[0], signatures[1]);
        }
    }

    #[test]
    fn global_non_polygon_ownership_rejects_missing_stale_duplicate_and_cross_core_evidence() {
        use crate::tiling::{prove_global_non_polygon_ownership, PartitionMosaic};
        let geometries = [Geometry::LineString(LineString::from(vec![
            (1.0, 1.0),
            (2.0, 1.0),
        ]))];
        let geometry_refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 8.0 });
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 4.0).with_buffer(0.0);
        tiled.add_geometry(&geometries[0]);
        let result = tiled.polygonize().unwrap();
        let tiles = tiled.generate_tiles().unwrap();
        let mut mosaic = PartitionMosaic::default();
        for snapshot in result.partition_snapshots {
            mosaic.replace_partition(snapshot, &policy).unwrap();
        }
        let (proof, _) = prove_global_non_polygon_ownership(
            &mosaic,
            &tiles,
            &geometry_refs,
            domain,
            &options,
            &policy,
        )
        .unwrap()
        .unwrap();
        let extraction = crate::graph::partition_border::PartitionBorderPhysicalTableExtraction {
            ring_payloads: vec![],
            local_aliases: vec![],
            non_polygon_payloads: proof
                .payloads
                .iter()
                .map(|owned| owned.payload.clone())
                .collect(),
        };
        assert!(proof.covers(&extraction, &mosaic, &policy).unwrap());
        let mut missing_payload = extraction.clone();
        missing_payload.non_polygon_payloads.clear();
        assert!(!proof.covers(&missing_payload, &mosaic, &policy).unwrap());
        for mutation in 0..10 {
            let mut changed = mosaic.clone();
            let owner = changed.partitions.get_mut(&0).unwrap();
            match mutation {
                0 => {
                    changed.partitions.remove(&3);
                }
                1 => owner.non_polygon.dangles.clear(),
                2 => owner
                    .non_polygon
                    .dangles
                    .push(owner.non_polygon.dangles[0].clone()),
                3 => owner.local_noded_segments.clear(),
                4 => owner.local_noded_segments[0].source_line_ids.push(999),
                5 => owner.selected_source_segments.clear(),
                6 => owner.topology.options = serde_json::json!({}),
                7 => owner.non_polygon.cut_edges = std::mem::take(&mut owner.non_polygon.dangles),
                8 => {
                    let payload = owner.non_polygon.dangles[0].clone();
                    changed
                        .partitions
                        .get_mut(&1)
                        .unwrap()
                        .non_polygon
                        .dangles
                        .push(payload);
                }
                _ => owner.non_polygon.dangles[0][0][2] = f64::NAN.to_bits(),
            }
            let before = changed.fingerprint_sha256();
            assert!(
                prove_global_non_polygon_ownership(
                    &changed,
                    &tiles,
                    &geometry_refs,
                    domain,
                    &options,
                    &policy
                )
                .unwrap()
                .is_none(),
                "mutation {mutation}"
            );
            assert!(!proof.covers(&extraction, &changed, &policy).unwrap());
            assert_eq!(changed.fingerprint_sha256(), before);
        }
        let other = [Geometry::LineString(LineString::from(vec![
            (1.0, 2.0),
            (2.0, 2.0),
        ]))];
        assert!(prove_global_non_polygon_ownership(
            &mosaic,
            &tiles,
            &[(&other[0], None)],
            domain,
            &options,
            &policy
        )
        .unwrap()
        .is_none());
        let cross_core = Geometry::LineString(LineString::from(vec![(3.0, 1.0), (5.0, 1.0)]));
        let mut crossing = TiledPolygonizer::new(domain, 4.0)
            .with_buffer(0.0)
            .with_untiled_equivalence_check();
        crossing.add_geometry(&cross_core);
        let traced = crossing
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let event = &traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "partition_border_global_non_polygon_ownership")
            .unwrap()
            .payload;
        assert_eq!(event["proven"], false);
        assert!(event["proof"].is_null());
        assert!(traced.result.stitched_output.is_none());
        for limited in [
            ExecutionPolicy {
                max_graph_edges: Some(0),
                ..Default::default()
            },
            ExecutionPolicy {
                max_output_coordinates: Some(1),
                ..Default::default()
            },
            ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                prove_global_non_polygon_ownership(
                    &mosaic,
                    &tiles,
                    &geometry_refs,
                    domain,
                    &options,
                    &limited
                ),
                Err(PolygonizeError::ResourceLimitExceeded { .. })
            ));
        }
        for cancel_at in [0, 1] {
            let token = CancellationToken::new();
            let cancelled = ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, cancel_at)),
                ..Default::default()
            };
            assert!(matches!(
                prove_global_non_polygon_ownership(
                    &mosaic,
                    &tiles,
                    &geometry_refs,
                    domain,
                    &options,
                    &cancelled
                ),
                Err(PolygonizeError::Cancelled { .. })
            ));
        }
    }

    #[test]
    fn global_non_polygon_ownership_keeps_invalid_ring_multiplicity_without_bypassing_duplicate_gates(
    ) {
        use crate::tiling::{prove_global_non_polygon_ownership, PartitionMosaic};
        let mut geometry = Geometry::LineString(LineString::from(vec![
            (1.0, 1.0),
            (3.0, 3.0),
            (1.0, 3.0),
            (3.0, 1.0),
            (1.0, 1.0),
        ]));
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 4.0 });
        let options = PolygonizerOptions::default();
        let policy = ExecutionPolicy::default();
        let mut signatures = Vec::new();
        for reverse in [false, true] {
            if reverse {
                if let Geometry::LineString(line) = &mut geometry {
                    line.0.reverse();
                }
            }
            let mut tiled = TiledPolygonizer::new(domain, 4.0)
                .with_options(options.clone())
                .with_untiled_equivalence_check();
            tiled.add_geometry(&geometry);
            let traced = tiled
                .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                .unwrap();
            let report = &traced.result.stitching_report;
            assert_eq!(
                report.partition_border_global_non_polygon_extraction_invalid_ring_count,
                2
            );
            assert_eq!(
                report.partition_border_global_non_polygon_extraction_duplicate_payload_count,
                1
            );
            assert!(!report.partition_border_global_non_polygon_extraction_ready);
            assert!(traced.result.stitched_output.is_none());
            let mut mosaic = PartitionMosaic::default();
            for snapshot in traced.result.partition_snapshots {
                mosaic.replace_partition(snapshot, &policy).unwrap();
            }
            let (proof, reference) = prove_global_non_polygon_ownership(
                &mosaic,
                &[domain],
                &[(&geometry, None)],
                domain,
                &options,
                &policy,
            )
            .unwrap()
            .unwrap();
            assert_eq!(reference.invalid_rings.len(), 2);
            assert_eq!(proof.payloads.len(), 1);
            let payload = &proof.payloads[0];
            assert_eq!(payload.payload.kind, crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind::InvalidRing);
            assert_eq!(payload.global_payload_count, 2);
            assert_eq!(payload.local_payload_indices, vec![0, 1]);
            assert_eq!(payload.edges.len(), 4);
            assert_eq!(payload.payload.partition_id, 0);
            let event = &traced
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_non_polygon_ownership")
                .unwrap()
                .payload;
            assert_eq!(event["proven"], true);
            assert_eq!(event["proof"], serde_json::to_value(&proof).unwrap());
            let conformance = &traced
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_global_arrangement_conformance")
                .unwrap()
                .payload;
            assert_eq!(conformance["candidate_built"], false);
            assert_eq!(conformance["equivalence_checked"], false);
            assert_eq!(conformance["output_selectable"], false);
            for missing in [true, false] {
                let mut changed = mosaic.clone();
                let rings = &mut changed
                    .partitions
                    .get_mut(&0)
                    .unwrap()
                    .non_polygon
                    .invalid_rings;
                if missing {
                    rings.pop();
                } else {
                    rings.push(rings[0].clone());
                }
                assert!(prove_global_non_polygon_ownership(
                    &changed,
                    &[domain],
                    &[(&geometry, None)],
                    domain,
                    &options,
                    &policy
                )
                .unwrap()
                .is_none());
            }
            signatures.push(proof.payloads);
        }
        assert_eq!(signatures[0], signatures[1]);
    }

    #[test]
    fn cross_core_non_polygon_assembly_retains_all_copies_and_unique_atomic_owners() {
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 4.0 });
        let cases = [
            (
                vec![Geometry::LineString(LineString::from(vec![
                    (3.0, 1.0),
                    (5.0, 1.0),
                ]))],
                "dangle",
                2.0,
                domain,
            ),
            (
                vec![
                    Geometry::LineString(LineString::from(vec![
                        (1.0, 1.0),
                        (2.0, 1.0),
                        (2.0, 1.5),
                        (2.0, 2.0),
                        (1.0, 2.0),
                        (1.0, 1.0),
                    ])),
                    Geometry::LineString(LineString::from(vec![
                        (6.0, 1.0),
                        (7.0, 1.0),
                        (7.0, 2.0),
                        (6.0, 2.0),
                        (6.0, 1.5),
                        (6.0, 1.0),
                    ])),
                    Geometry::LineString(LineString::from(vec![(2.0, 1.5), (6.0, 1.5)])),
                ],
                "cut_edge",
                4.0,
                domain,
            ),
            (
                vec![Geometry::LineString(LineString::from(vec![
                    (1.0, 3.0),
                    (1.0, 5.0),
                ]))],
                "dangle",
                2.0,
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 8.0 }),
            ),
            (
                vec![Geometry::LineString(LineString::from(vec![
                    (3.0, 1.0),
                    (5.0, 1.0),
                ]))],
                "dangle",
                0.0,
                domain,
            ),
            (
                vec![Geometry::LineString(LineString::from(vec![
                    (3.0, 1.0),
                    (9.0, 1.0),
                ]))],
                "dangle",
                8.0,
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 }),
            ),
            (
                vec![Geometry::LineString(LineString::from(vec![
                    (1.0, 1.0),
                    (15.0, 1.0),
                ]))],
                "dangle",
                0.0,
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 16.0, y: 4.0 }),
            ),
            (
                vec![Geometry::LineString(LineString::from(vec![
                    (1.0, 1.0),
                    (1.0, 15.0),
                ]))],
                "dangle",
                0.0,
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 16.0 }),
            ),
            (
                vec![Geometry::LineString(LineString::from(vec![
                    (-11.0, -3.0),
                    (-1.0, -3.0),
                ]))],
                "dangle",
                8.0,
                Rect::new(Coord { x: -12.0, y: -4.0 }, Coord { x: 0.0, y: 0.0 }),
            ),
            (
                vec![
                    Geometry::LineString(LineString::from(vec![
                        (1.0, 1.0),
                        (2.0, 1.0),
                        (2.0, 1.5),
                        (2.0, 2.0),
                        (1.0, 2.0),
                        (1.0, 1.0),
                    ])),
                    Geometry::LineString(LineString::from(vec![
                        (10.0, 1.0),
                        (11.0, 1.0),
                        (11.0, 2.0),
                        (10.0, 2.0),
                        (10.0, 1.5),
                        (10.0, 1.0),
                    ])),
                    Geometry::LineString(LineString::from(vec![(2.0, 1.5), (10.0, 1.5)])),
                ],
                "cut_edge",
                12.0,
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 }),
            ),
        ];
        for (mut geometries, kind, buffer, domain) in cases {
            let mut signatures = Vec::new();
            for reverse in [false, true] {
                if reverse {
                    geometries.reverse();
                    for geometry in &mut geometries {
                        if let Geometry::LineString(line) = geometry {
                            line.0.reverse();
                        }
                    }
                }
                let mut tiled = TiledPolygonizer::new(domain, 4.0)
                    .with_buffer(buffer)
                    .with_untiled_equivalence_check();
                for geometry in &geometries {
                    tiled.add_geometry(geometry);
                }
                let core_count = tiled.generate_tiles().unwrap().len();
                let ordinary = tiled.polygonize().unwrap();
                let traced = tiled
                    .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                    .unwrap();
                for result in [&ordinary, &traced.result] {
                    assert_eq!(
                        result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_duplicate_payload_count,
                        core_count - 1
                    );
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_ready
                    );
                    assert!(result.stitched_output.is_none());
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_untiled_equivalence_ready
                    );
                }
                let event = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_cross_core_non_polygon_assembly")
                    .unwrap()
                    .payload;
                assert_eq!(event["constructed"], true, "kind={kind} event={event}");
                assert_eq!(event["output_selectable"], false);
                let payloads = event["assembly"]["payloads"].as_array().unwrap();
                assert_eq!(payloads.len(), 1);
                let payload = &payloads[0];
                assert_eq!(payload["kind"], kind);
                assert_eq!(payload["global_payload_count"], 1);
                assert_eq!(
                    payload["local_payload_aliases"],
                    serde_json::json!((0..core_count).map(|part| [part, 0]).collect::<Vec<_>>())
                );
                let spans = payload["spans"].as_array().unwrap();
                assert_eq!(spans.len(), core_count);
                assert_eq!(
                    spans
                        .iter()
                        .map(|span| span["owner_partition_id"].as_u64().unwrap())
                        .collect::<BTreeSet<_>>(),
                    (0..core_count as u64).collect::<BTreeSet<_>>()
                );
                let coordinate = |bits: &serde_json::Value| {
                    serde_json::to_value(
                        crate::fingerprint::coordinate_fingerprint(Coord3D::new(
                            f64::from_bits(bits[0].as_u64().unwrap()),
                            f64::from_bits(bits[1].as_u64().unwrap()),
                            f64::from_bits(bits[2].as_u64().unwrap()),
                        ))
                        .unwrap(),
                    )
                    .unwrap()
                };
                let mut node = coordinate(&payload["coordinate_bits"][0]);
                let mut retained_boundary_aliases = BTreeSet::new();
                for span in spans {
                    let owner = span["owner_partition_id"].as_u64().unwrap();
                    assert!(span["boundary_segment_aliases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|alias| alias[0].as_u64() == Some(owner)));
                    assert_eq!(span["segment"]["source_line_ids"], serde_json::json!([0]));
                    assert_eq!(span["segment"]["representative_line_id"], 0);
                    let mut alias_partitions = BTreeSet::new();
                    for alias in span["boundary_segment_aliases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .chain(span["coarse_boundary_segment_aliases"].as_array().unwrap())
                    {
                        alias_partitions.insert(alias[0].as_u64().unwrap());
                        retained_boundary_aliases
                            .insert((alias[0].as_u64().unwrap(), alias[1].as_u64().unwrap()));
                    }
                    assert_eq!(
                        alias_partitions,
                        (0..core_count as u64).collect::<BTreeSet<_>>()
                    );
                    node = if span["segment"]["start"] == node {
                        span["segment"]["end"].clone()
                    } else {
                        assert_eq!(span["segment"]["end"], node);
                        span["segment"]["start"].clone()
                    };
                }
                assert_eq!(node, coordinate(&payload["coordinate_bits"][1]));
                if kind == "dangle" {
                    let all_rows = traced
                        .result
                        .partition_snapshots
                        .iter()
                        .flat_map(|snapshot| {
                            (0..snapshot.boundary_noded_segments.len())
                                .map(|local| (snapshot.partition_id as u64, local as u64))
                        })
                        .collect::<BTreeSet<_>>();
                    assert_eq!(retained_boundary_aliases, all_rows);
                }
                if core_count > 2 {
                    assert!(spans
                        .iter()
                        .any(|span| !span["coarse_boundary_segment_aliases"]
                            .as_array()
                            .unwrap()
                            .is_empty()));
                }
                let conformance = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_global_arrangement_conformance")
                    .unwrap()
                    .payload;
                assert_eq!(
                    conformance["candidate_built"], true,
                    "conformance={conformance}"
                );
                assert_eq!(conformance["equivalence_checked"], true);
                assert_eq!(conformance["equivalence_ready"], true);
                assert_eq!(conformance["equivalence_mismatch_count"], 0);
                assert_eq!(conformance["component_physical_proof_bound"], true);
                assert_eq!(conformance["output_selectable"], false);
                signatures.push(payload.clone());
            }
            assert_eq!(signatures[0], signatures[1]);
        }
    }

    #[test]
    fn cross_core_non_polygon_assembly_rejects_missing_stale_and_conflicting_span_evidence() {
        use crate::tiling::{
            assemble_cross_core_non_polygon, prepare_non_polygon_evidence, PartitionMosaic,
        };
        let geometry = Geometry::LineString(LineString::from(vec![(3.0, 1.0), (5.0, 1.0)]));
        let geometries = [(&geometry, None)];
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 4.0 });
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 4.0).with_buffer(2.0);
        tiled.add_geometry(&geometry);
        let result = tiled.polygonize().unwrap();
        let tiles = tiled.generate_tiles().unwrap();
        let mut mosaic = PartitionMosaic::default();
        for snapshot in result.partition_snapshots.iter().rev() {
            mosaic.replace_partition(snapshot.clone(), &policy).unwrap();
        }
        let evidence =
            prepare_non_polygon_evidence(&mosaic, &tiles, &geometries, domain, &options, &policy)
                .unwrap()
                .unwrap();
        let expected = assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
            .unwrap()
            .unwrap();
        let before = mosaic.fingerprint_sha256();
        for mutation in 0..7 {
            let mut changed = mosaic.clone();
            let snapshot = changed.partitions.get_mut(&0).unwrap();
            match mutation {
                0 => {
                    snapshot.boundary_noded_segments.pop();
                }
                1 => snapshot.boundary_noded_segments[0]
                    .source_line_ids
                    .push(999),
                2 => snapshot
                    .boundary_noded_segments
                    .push(snapshot.boundary_noded_segments[0].clone()),
                3 => snapshot.boundary_noded_segments[0].representative_line_id = None,
                4 => {
                    for snapshot in changed.partitions.values_mut() {
                        snapshot.declared_adjacencies.clear();
                    }
                }
                5 => {
                    for snapshot in changed.partitions.values_mut() {
                        for adjacency in &mut snapshot.declared_adjacencies {
                            adjacency.coordinate_bits = 4.5f64.to_bits();
                        }
                    }
                }
                _ => {
                    let snapshot = changed.partitions.get_mut(&1).unwrap();
                    snapshot.local_noded_segments.clear();
                    snapshot.boundary_noded_segments.clear();
                }
            }
            assert!(
                assemble_cross_core_non_polygon(&evidence, &changed, &tiles, domain, &policy)
                    .unwrap()
                    .is_none()
            );
            let Some(prepared) = prepare_non_polygon_evidence(
                &changed,
                &tiles,
                &geometries,
                domain,
                &options,
                &policy,
            )
            .unwrap() else {
                continue;
            };
            assert!(
                assemble_cross_core_non_polygon(&prepared, &changed, &tiles, domain, &policy)
                    .unwrap()
                    .is_none(),
                "mutation {mutation}"
            );
        }
        for limited in [
            ExecutionPolicy {
                max_graph_nodes: Some(2),
                ..Default::default()
            },
            ExecutionPolicy {
                max_graph_edges: Some(1),
                ..Default::default()
            },
            ExecutionPolicy {
                max_candidate_pairs: Some(0),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &limited),
                Err(PolygonizeError::ResourceLimitExceeded { .. })
            ));
        }
        for cancel_at in [0, 1] {
            let token = CancellationToken::new();
            let cancelled = ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, cancel_at)),
                ..Default::default()
            };
            assert!(matches!(
                assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &cancelled),
                Err(PolygonizeError::Cancelled { .. })
            ));
        }
        assert_eq!(before, mosaic.fingerprint_sha256());
        assert_eq!(
            expected,
            assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
                .unwrap()
                .unwrap()
        );
    }

    #[test]
    fn multi_boundary_non_polygon_assembly_rejects_one_bad_member_as_a_whole() {
        use crate::tiling::{
            assemble_cross_core_non_polygon, prepare_non_polygon_evidence, PartitionMosaic,
        };
        let geometries = [
            Geometry::LineString(LineString::from(vec![(1.0, 1.0), (15.0, 1.0)])),
            Geometry::LineString(LineString::from(vec![(1.0, 2.0), (2.0, 2.0)])),
        ];
        let refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 16.0, y: 4.0 });
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 4.0);
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let result = tiled.polygonize().unwrap();
        let tiles = tiled.generate_tiles().unwrap();
        let mut mosaic = PartitionMosaic::default();
        for snapshot in result.partition_snapshots.iter().rev() {
            mosaic.replace_partition(snapshot.clone(), &policy).unwrap();
        }
        let evidence =
            prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                .unwrap()
                .unwrap();
        let expected = assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
            .unwrap()
            .unwrap();
        assert_eq!(expected.payloads.len(), 1);
        assert_eq!(expected.payloads[0].spans.len(), 4);
        assert_eq!(expected.whole_core_payloads.len(), 1);
        let coarse = expected.payloads[0]
            .spans
            .iter()
            .flat_map(|span| &span.coarse_boundary_segment_aliases)
            .find(|alias| alias[0] == 0)
            .copied()
            .unwrap();
        let before = mosaic.fingerprint_sha256();
        for mutation in 0..10 {
            let mut changed = mosaic.clone();
            match mutation {
                0 => changed
                    .partitions
                    .get_mut(&1)
                    .unwrap()
                    .boundary_noded_segments
                    .clear(),
                1 => {
                    changed
                        .partitions
                        .get_mut(&coarse[0])
                        .unwrap()
                        .boundary_noded_segments[coarse[1]]
                        .representative_line_id = None
                }
                2 => changed
                    .partitions
                    .get_mut(&coarse[0])
                    .unwrap()
                    .boundary_noded_segments[coarse[1]]
                    .source_line_ids
                    .push(999),
                3 => {
                    changed
                        .partitions
                        .get_mut(&coarse[0])
                        .unwrap()
                        .boundary_noded_segments[coarse[1]]
                        .start
                        .z = "0x3ff0000000000000".to_string()
                }
                4 => {
                    for (a, b) in [(1, 2), (2, 1)] {
                        changed
                            .partitions
                            .get_mut(&a)
                            .unwrap()
                            .declared_adjacencies
                            .retain(|adjacency| adjacency.neighbor_partition_id != b);
                    }
                }
                5 => {
                    for (a, b) in [(1, 2), (2, 1)] {
                        for adjacency in
                            &mut changed.partitions.get_mut(&a).unwrap().declared_adjacencies
                        {
                            if adjacency.neighbor_partition_id == b {
                                adjacency.coordinate_bits = 8.5f64.to_bits();
                            }
                        }
                    }
                }
                6 => changed
                    .partitions
                    .get_mut(&1)
                    .unwrap()
                    .non_polygon
                    .dangles
                    .clear(),
                7 => {
                    let snapshot = changed.partitions.get_mut(&2).unwrap();
                    snapshot
                        .non_polygon
                        .dangles
                        .push(snapshot.non_polygon.dangles[0].clone());
                }
                8 => {
                    let snapshot = changed.partitions.get_mut(&3).unwrap();
                    let payload = snapshot.non_polygon.dangles.pop().unwrap();
                    snapshot.non_polygon.cut_edges.push(payload);
                }
                _ => {
                    let snapshot = changed.partitions.get_mut(&0).unwrap();
                    snapshot
                        .non_polygon
                        .dangles
                        .retain(|bits| bits[0][1] != 2.0f64.to_bits());
                }
            }
            assert!(
                assemble_cross_core_non_polygon(&evidence, &changed, &tiles, domain, &policy)
                    .unwrap()
                    .is_none(),
                "stale mutation {mutation}"
            );
            if let Some(refreshed) =
                prepare_non_polygon_evidence(&changed, &tiles, &refs, domain, &options, &policy)
                    .unwrap()
            {
                assert!(
                    assemble_cross_core_non_polygon(&refreshed, &changed, &tiles, domain, &policy)
                        .unwrap()
                        .is_none(),
                    "current mutation {mutation}"
                );
            }
        }
        let limited = ExecutionPolicy {
            max_candidate_pairs: Some(67),
            ..Default::default()
        };
        assert!(matches!(
            assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &limited),
            Err(PolygonizeError::ResourceLimitExceeded { .. })
        ));
        for cancel_at in [0, 1, 3] {
            let token = CancellationToken::new();
            let cancelled = ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, cancel_at)),
                ..Default::default()
            };
            assert!(matches!(
                assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &cancelled),
                Err(PolygonizeError::Cancelled { .. })
            ));
        }
        assert_eq!(before, mosaic.fingerprint_sha256());
        assert_eq!(
            expected,
            assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
                .unwrap()
                .unwrap()
        );
    }

    #[test]
    fn cross_core_ring_assembly_retains_closed_lineage_and_all_record_multiplicity() {
        use crate::tiling::{
            assemble_cross_core_non_polygon, prepare_non_polygon_evidence, PartitionMosaic,
        };
        let cases = [
            (
                vec![(1.0, 1.0), (5.0, 3.0), (5.0, 1.0), (1.0, 3.0), (1.0, 1.0)],
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 4.0 }),
                false,
            ),
            (
                vec![(2.0, 1.0), (10.0, 3.0), (10.0, 1.0), (2.0, 3.0), (2.0, 1.0)],
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 }),
                false,
            ),
            (
                vec![(1.0, 2.0), (3.0, 10.0), (1.0, 10.0), (3.0, 2.0), (1.0, 2.0)],
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 12.0 }),
                false,
            ),
            (
                vec![
                    (-10.0, -3.0),
                    (-2.0, -1.0),
                    (-2.0, -3.0),
                    (-10.0, -1.0),
                    (-10.0, -3.0),
                ],
                Rect::new(Coord { x: -12.0, y: -4.0 }, Coord { x: 0.0, y: 0.0 }),
                false,
            ),
            (
                vec![(0.5, 4.0), (1.5, 6.0), (1.0, 2.0), (2.5, 2.0), (0.5, 4.0)],
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 8.0 }),
                true,
            ),
        ];
        let options = PolygonizerOptions {
            node_input: false,
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        for (coords, domain, closing_transition) in cases {
            let mut geometry = Geometry::LineString(LineString::from(coords));
            let mut signatures = Vec::new();
            for reverse in [false, true] {
                if reverse {
                    if let Geometry::LineString(line) = &mut geometry {
                        line.0.reverse();
                    }
                }
                let mut tiled = TiledPolygonizer::new(domain, 4.0)
                    .with_options(options.clone())
                    .with_untiled_equivalence_check();
                tiled.add_geometry(&geometry);
                let ordinary = tiled.polygonize().unwrap();
                let traced = tiled
                    .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                    .unwrap();
                let tiles = tiled.generate_tiles().unwrap();
                let mut mosaic = PartitionMosaic::default();
                for snapshot in traced.result.partition_snapshots.iter().rev() {
                    mosaic.replace_partition(snapshot.clone(), &policy).unwrap();
                }
                let evidence = prepare_non_polygon_evidence(
                    &mosaic,
                    &tiles,
                    &[(&geometry, None)],
                    domain,
                    &options,
                    &policy,
                )
                .unwrap()
                .unwrap();
                let assembly =
                    assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
                        .unwrap()
                        .unwrap_or_else(|| {
                            panic!("incomplete ring witness: domain={domain:?}, reverse={reverse}")
                        });
                assert_eq!(assembly.payloads.len(), 1);
                assert!(assembly.whole_core_payloads.is_empty());
                let payload = &assembly.payloads[0];
                assert_eq!(payload.kind, crate::graph::partition_border::PartitionBorderGlobalNonPolygonPayloadKind::InvalidRing);
                assert_eq!(payload.global_payload_count, 2);
                assert_eq!(
                    payload.local_payload_aliases,
                    (0..tiles.len())
                        .flat_map(|part| [[part, 0], [part, 1]])
                        .collect::<Vec<_>>()
                );
                assert_eq!(
                    payload.coordinate_bits.first(),
                    payload.coordinate_bits.last()
                );
                assert!(payload.spans.len() > payload.coordinate_bits.len() - 1);
                let coordinate = |bits: [u64; 3]| {
                    crate::fingerprint::coordinate_fingerprint(Coord3D::new(
                        f64::from_bits(bits[0]),
                        f64::from_bits(bits[1]),
                        f64::from_bits(bits[2]),
                    ))
                    .unwrap()
                };
                let mut visited = 0;
                let mut retained_aliases = BTreeSet::new();
                let mut owners = BTreeSet::new();
                for (edge_index, pair) in payload.coordinate_bits.windows(2).enumerate() {
                    let mut node = coordinate(pair[0]);
                    let edge_spans = payload
                        .spans
                        .iter()
                        .filter(|span| span.payload_edge_index == edge_index)
                        .collect::<Vec<_>>();
                    assert!(!edge_spans.is_empty());
                    for span in edge_spans {
                        assert_eq!(payload.spans[visited], *span);
                        visited += 1;
                        node = if span.segment.start == node {
                            span.segment.end.clone()
                        } else {
                            assert_eq!(span.segment.end, node);
                            span.segment.start.clone()
                        };
                        assert_eq!(span.segment.source_line_ids, vec![0]);
                        assert_eq!(span.segment.representative_line_id, Some(0));
                        owners.insert(span.owner_partition_id);
                        assert!(span
                            .boundary_segment_aliases
                            .iter()
                            .any(|alias| alias[0] == span.owner_partition_id));
                        for &alias in &span.boundary_segment_aliases {
                            assert_eq!(
                                mosaic.partitions[&alias[0]].boundary_noded_segments[alias[1]],
                                span.segment
                            );
                            retained_aliases.insert(alias);
                        }
                        for &alias in &span.coarse_boundary_segment_aliases {
                            let parent =
                                &mosaic.partitions[&alias[0]].boundary_noded_segments[alias[1]];
                            assert_ne!(*parent, span.segment);
                            assert_eq!(parent.source_line_ids, span.segment.source_line_ids);
                            assert_eq!(
                                parent.representative_line_id,
                                span.segment.representative_line_id
                            );
                            retained_aliases.insert(alias);
                        }
                    }
                    assert_eq!(node, coordinate(pair[1]));
                }
                assert_eq!(visited, payload.spans.len());
                assert_eq!(owners, (0..tiles.len()).collect::<BTreeSet<_>>());
                assert_eq!(
                    retained_aliases,
                    mosaic
                        .partitions
                        .values()
                        .flat_map(|snapshot| (0..snapshot.boundary_noded_segments.len())
                            .map(|local| [snapshot.partition_id, local]))
                        .collect::<BTreeSet<_>>()
                );
                if closing_transition {
                    assert_ne!(
                        payload.spans.first().unwrap().owner_partition_id,
                        payload.spans.last().unwrap().owner_partition_id
                    );
                }
                for result in [&ordinary, &traced.result] {
                    assert_eq!(
                        result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_invalid_ring_count,
                        2 * tiles.len()
                    );
                    assert_eq!(
                        result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_duplicate_payload_count,
                        2 * tiles.len() - 1
                    );
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_ready
                    );
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_untiled_equivalence_ready
                    );
                    assert!(result.stitched_output.is_none());
                }
                let event = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_cross_core_non_polygon_assembly")
                    .unwrap()
                    .payload;
                assert_eq!(event["constructed"], true);
                assert_eq!(event["output_selectable"], false);
                assert_eq!(event["assembly"], serde_json::to_value(&assembly).unwrap());
                let conformance = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_global_arrangement_conformance")
                    .unwrap()
                    .payload;
                assert_eq!(
                    conformance["candidate_built"], true,
                    "conformance={conformance}"
                );
                assert_eq!(conformance["equivalence_checked"], true);
                assert_eq!(conformance["equivalence_ready"], true);
                assert_eq!(conformance["equivalence_mismatch_count"], 0);
                assert_eq!(conformance["component_physical_proof_bound"], true);
                assert_eq!(conformance["output_selectable"], false);
                signatures.push(payload.clone());
            }
            assert_eq!(signatures[0], signatures[1]);
        }
    }

    #[test]
    fn cross_core_ring_assembly_rejects_bad_ring_or_companion_evidence_as_a_whole() {
        use crate::tiling::{
            assemble_cross_core_non_polygon, prepare_non_polygon_evidence, PartitionMosaic,
        };
        let geometries = [
            Geometry::LineString(LineString::from(vec![
                (2.0, 1.0),
                (10.0, 3.0),
                (10.0, 1.0),
                (2.0, 3.0),
                (2.0, 1.0),
            ])),
            Geometry::LineString(LineString::from(vec![(0.5, 0.5), (1.0, 0.5)])),
        ];
        let refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        let options = PolygonizerOptions {
            node_input: false,
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 4.0).with_options(options.clone());
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let tiles = tiled.generate_tiles().unwrap();
        let mut mosaic = PartitionMosaic::default();
        for snapshot in tiled.polygonize().unwrap().partition_snapshots {
            mosaic.replace_partition(snapshot, &policy).unwrap();
        }
        let evidence =
            prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                .unwrap()
                .unwrap();
        let expected = assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
            .unwrap()
            .unwrap();
        assert_eq!(expected.payloads.len(), 1);
        assert_eq!(expected.payloads[0].global_payload_count, 2);
        assert_eq!(expected.whole_core_payloads.len(), 1);
        let coarse = expected.payloads[0]
            .spans
            .iter()
            .flat_map(|span| &span.coarse_boundary_segment_aliases)
            .next()
            .copied()
            .unwrap();
        let before = mosaic.fingerprint_sha256();
        for mutation in 0..9 {
            let mut changed = mosaic.clone();
            match mutation {
                0 => {
                    changed
                        .partitions
                        .get_mut(&1)
                        .unwrap()
                        .non_polygon
                        .invalid_rings
                        .pop();
                }
                1 => {
                    let snapshot = changed.partitions.get_mut(&2).unwrap();
                    snapshot
                        .non_polygon
                        .invalid_rings
                        .push(snapshot.non_polygon.invalid_rings[0].clone());
                }
                2 => {
                    for snapshot in changed.partitions.values_mut() {
                        for ring in &mut snapshot.non_polygon.invalid_rings {
                            ring.swap(1, 2);
                        }
                    }
                }
                3 => {
                    changed
                        .partitions
                        .get_mut(&1)
                        .unwrap()
                        .boundary_noded_segments
                        .pop();
                }
                4 => {
                    changed
                        .partitions
                        .get_mut(&coarse[0])
                        .unwrap()
                        .boundary_noded_segments[coarse[1]]
                        .start
                        .z = "0x3ff0000000000000".to_string()
                }
                5 => changed
                    .partitions
                    .get_mut(&coarse[0])
                    .unwrap()
                    .boundary_noded_segments[coarse[1]]
                    .source_line_ids
                    .push(999),
                6 => {
                    changed
                        .partitions
                        .get_mut(&coarse[0])
                        .unwrap()
                        .boundary_noded_segments[coarse[1]]
                        .representative_line_id = None
                }
                7 => {
                    for (a, b) in [(1, 2), (2, 1)] {
                        changed
                            .partitions
                            .get_mut(&a)
                            .unwrap()
                            .declared_adjacencies
                            .retain(|adjacency| adjacency.neighbor_partition_id != b);
                    }
                }
                _ => changed
                    .partitions
                    .get_mut(&0)
                    .unwrap()
                    .non_polygon
                    .dangles
                    .clear(),
            }
            assert!(
                assemble_cross_core_non_polygon(&evidence, &changed, &tiles, domain, &policy)
                    .unwrap()
                    .is_none(),
                "stale mutation {mutation}"
            );
            if let Some(refreshed) =
                prepare_non_polygon_evidence(&changed, &tiles, &refs, domain, &options, &policy)
                    .unwrap()
            {
                assert!(
                    assemble_cross_core_non_polygon(&refreshed, &changed, &tiles, domain, &policy)
                        .unwrap()
                        .is_none(),
                    "current mutation {mutation}"
                );
            }
        }
        for limited in [
            ExecutionPolicy {
                max_candidate_pairs: Some(100),
                ..Default::default()
            },
            ExecutionPolicy {
                max_graph_nodes: Some(2),
                ..Default::default()
            },
            ExecutionPolicy {
                max_graph_edges: Some(1),
                ..Default::default()
            },
        ] {
            assert!(matches!(
                assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &limited),
                Err(PolygonizeError::ResourceLimitExceeded { .. })
            ));
        }
        for cancel_at in [0, 1, 4] {
            let token = CancellationToken::new();
            let cancelled = ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, cancel_at)),
                ..Default::default()
            };
            assert!(matches!(
                assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &cancelled),
                Err(PolygonizeError::Cancelled { .. })
            ));
        }
        assert_eq!(before, mosaic.fingerprint_sha256());
        assert_eq!(
            expected,
            assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
                .unwrap()
                .unwrap()
        );
    }

    #[test]
    fn cross_core_non_polygon_assembly_declines_corners_halo_only_claimants_and_branched_rings() {
        for (coords, domain, buffer, node_input) in [
            (
                vec![(3.0, 3.0), (5.0, 5.0)],
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 8.0 }),
                2.0,
                true,
            ),
            (
                vec![(3.0, 1.0), (5.0, 1.0)],
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 }),
                8.0,
                true,
            ),
            (
                vec![(3.0, 1.0), (5.0, 3.0), (5.0, 1.0), (3.0, 3.0), (3.0, 1.0)],
                Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 4.0 }),
                0.0,
                false,
            ),
        ] {
            let geometry = Geometry::LineString(LineString::from(coords));
            let mut tiled = TiledPolygonizer::new(domain, 4.0)
                .with_buffer(buffer)
                .with_options(PolygonizerOptions {
                    node_input,
                    ..Default::default()
                })
                .with_untiled_equivalence_check();
            tiled.add_geometry(&geometry);
            let traced = tiled
                .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                .unwrap();
            let event = &traced
                .trace
                .events
                .iter()
                .find(|event| event.kind == "partition_border_cross_core_non_polygon_assembly")
                .unwrap()
                .payload;
            assert_eq!(
                event["constructed"], false,
                "domain={domain:?} buffer={buffer} event={event}"
            );
            assert!(event["assembly"].is_null());
            assert_eq!(event["output_selectable"], false);
            assert!(traced.result.stitched_output.is_none());
        }
    }

    #[test]
    fn global_non_polygon_consumer_materializes_all_families_and_retains_the_raw_ledger() {
        use crate::tiling::{
            assemble_cross_core_non_polygon, consume_global_non_polygon,
            non_polygon_output_mismatch_count, prepare_non_polygon_evidence,
            prove_whole_non_polygon_ownership, PartitionMosaic,
        };
        let originals = vec![
            Geometry::LineString(LineString::from(vec![(3.0, 3.5), (9.0, 3.5)])),
            Geometry::LineString(LineString::from(vec![
                (2.0, 1.0),
                (10.0, 3.0),
                (10.0, 1.0),
                (2.0, 3.0),
                (2.0, 1.0),
            ])),
            Geometry::LineString(LineString::from(vec![
                (0.25, 0.25),
                (0.5, 0.25),
                (0.5, 0.375),
                (0.5, 0.5),
                (0.25, 0.5),
                (0.25, 0.25),
            ])),
            Geometry::LineString(LineString::from(vec![
                (1.0, 0.25),
                (1.25, 0.25),
                (1.25, 0.5),
                (1.0, 0.5),
                (1.0, 0.375),
                (1.0, 0.25),
            ])),
            Geometry::LineString(LineString::from(vec![(0.5, 0.375), (1.0, 0.375)])),
        ];
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        let options = PolygonizerOptions {
            node_input: false,
            input_profile_id: Some("consumer-profile".to_string()),
            provenance: ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            },
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        for tile_size in [12.0, 4.0] {
            let mut geometries = originals.clone();
            let mut signatures = Vec::new();
            for reverse in [false, true] {
                if reverse {
                    geometries.reverse();
                    for geometry in &mut geometries {
                        if let Geometry::LineString(line) = geometry {
                            line.0.reverse();
                        }
                    }
                }
                let refs = geometries
                    .iter()
                    .map(|geometry| (geometry, None))
                    .collect::<Vec<_>>();
                let mut tiled = TiledPolygonizer::new(domain, tile_size)
                    .with_options(options.clone())
                    .with_untiled_equivalence_check();
                for geometry in &geometries {
                    tiled.add_geometry(geometry);
                }
                let ordinary = tiled.polygonize().unwrap();
                let traced = tiled
                    .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                    .unwrap();
                let tiles = tiled.generate_tiles().unwrap();
                let mut mosaic = PartitionMosaic::default();
                for snapshot in traced.result.partition_snapshots.iter().rev() {
                    mosaic.replace_partition(snapshot.clone(), &policy).unwrap();
                }
                let evidence =
                    prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                        .unwrap()
                        .unwrap();
                let ownership =
                    prove_whole_non_polygon_ownership(&evidence, &tiles, domain, &policy).unwrap();
                let assembly = if ownership.is_none() {
                    assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
                        .unwrap()
                } else {
                    None
                };
                let consumer = consume_global_non_polygon(
                    &evidence,
                    ownership.as_ref(),
                    assembly.as_ref(),
                    &mosaic,
                    &tiles,
                    domain,
                    &policy,
                )
                .unwrap()
                .unwrap();
                assert_eq!(consumer.output.dangles.len(), 1);
                assert_eq!(consumer.output.cut_edges.len(), 1);
                assert_eq!(consumer.output.invalid_rings.len(), 2);
                assert_eq!(consumer.payloads.len(), 3);
                assert_eq!(
                    consumer.input_fingerprint_sha256,
                    evidence.input_fingerprint_sha256
                );
                assert_eq!(
                    consumer.snapshot_fingerprints,
                    evidence.snapshot_fingerprints
                );
                assert_eq!(
                    non_polygon_output_mismatch_count(
                        &consumer.output.dangles,
                        &consumer.output.cut_edges,
                        &consumer.output.invalid_rings,
                        &evidence.reference,
                        &policy
                    )
                    .unwrap(),
                    0
                );
                let raw_count = mosaic
                    .partitions
                    .values()
                    .map(|snapshot| {
                        snapshot.non_polygon.dangles.len()
                            + snapshot.non_polygon.cut_edges.len()
                            + snapshot.non_polygon.invalid_rings.len()
                    })
                    .sum::<usize>();
                assert_eq!(
                    consumer
                        .payloads
                        .iter()
                        .map(|payload| payload.local_payload_aliases.len())
                        .sum::<usize>(),
                    raw_count
                );
                assert_eq!(
                    consumer
                        .payloads
                        .iter()
                        .map(|payload| payload.global_payload_count)
                        .sum::<usize>(),
                    4
                );
                for payload in &consumer.payloads {
                    assert_eq!(
                        payload.local_payload_aliases,
                        evidence.local_payloads[&(payload.kind, payload.coordinate_bits.to_vec())]
                    );
                }
                for result in [&ordinary, &traced.result] {
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_ready
                    );
                    assert_eq!(
                        result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_duplicate_payload_count,
                        raw_count - 3
                    );
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_untiled_equivalence_ready
                    );
                    assert!(result.stitched_output.is_none());
                }
                let event = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_global_non_polygon_consumer")
                    .unwrap()
                    .payload;
                assert_eq!(event["constructed"], true);
                assert_eq!(event["non_polygon_equivalence_ready"], true);
                assert_eq!(event["output_selectable"], false);
                assert_eq!(event["consumer"], serde_json::to_value(&consumer).unwrap());
                signatures.push(serde_json::to_value(&consumer.payloads).unwrap());
            }
            assert_eq!(signatures[0], signatures[1]);
        }
    }

    #[test]
    fn global_non_polygon_consumer_rejects_forged_stale_or_mismatched_evidence_as_a_whole() {
        use crate::tiling::{
            assemble_cross_core_non_polygon, consume_global_non_polygon,
            prepare_non_polygon_evidence, prove_whole_non_polygon_ownership, PartitionMosaic,
        };
        let geometries = [
            Geometry::LineString(LineString::from(vec![
                (2.0, 1.0),
                (10.0, 3.0),
                (10.0, 1.0),
                (2.0, 3.0),
                (2.0, 1.0),
            ])),
            Geometry::LineString(LineString::from(vec![(0.5, 0.5), (1.0, 0.5)])),
        ];
        let refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        let options = PolygonizerOptions {
            node_input: false,
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        for tile_size in [12.0, 4.0] {
            let mut tiled = TiledPolygonizer::new(domain, tile_size).with_options(options.clone());
            for geometry in &geometries {
                tiled.add_geometry(geometry);
            }
            let tiles = tiled.generate_tiles().unwrap();
            let mut mosaic = PartitionMosaic::default();
            for snapshot in tiled.polygonize().unwrap().partition_snapshots {
                mosaic.replace_partition(snapshot, &policy).unwrap();
            }
            let evidence =
                prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                    .unwrap()
                    .unwrap();
            let ownership =
                prove_whole_non_polygon_ownership(&evidence, &tiles, domain, &policy).unwrap();
            let assembly = if ownership.is_none() {
                assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
                    .unwrap()
            } else {
                None
            };
            let expected = consume_global_non_polygon(
                &evidence,
                ownership.as_ref(),
                assembly.as_ref(),
                &mosaic,
                &tiles,
                domain,
                &policy,
            )
            .unwrap()
            .unwrap();
            let before = mosaic.fingerprint_sha256();
            if let Some(proof) = &ownership {
                for mutation in 0..12 {
                    let mut forged = proof.clone();
                    let ring = forged
                        .payloads
                        .iter_mut()
                        .find(|payload| payload.global_payload_count == 2)
                        .unwrap();
                    match mutation {
                        0 => forged.input_fingerprint_sha256.push('0'),
                        1 => forged.snapshot_fingerprints[0].1.push('0'),
                        2 => forged.source_segment_count += 1,
                        3 => forged.noded_segment_count += 1,
                        4 => {
                            forged.payloads.pop();
                        }
                        5 => ring.payload.partition_id = 1,
                        6 => ring.global_payload_count = 1,
                        7 => {
                            ring.local_payload_indices.pop();
                        }
                        8 => ring.edges[0].local_segment_aliases.clear(),
                        9 => ring.edges[0].segment.representative_line_id = None,
                        10 => ring.edges[0].segment.source_line_ids.push(999),
                        _ => ring.edges[0].segment.start.z = "0x3ff0000000000000".to_string(),
                    }
                    assert!(
                        consume_global_non_polygon(
                            &evidence,
                            Some(&forged),
                            None,
                            &mosaic,
                            &tiles,
                            domain,
                            &policy
                        )
                        .unwrap()
                        .is_none(),
                        "whole mutation {mutation}"
                    );
                }
            } else {
                let proof = assembly.as_ref().unwrap();
                for mutation in 0..15 {
                    let mut forged = proof.clone();
                    let ring = &mut forged.payloads[0];
                    match mutation {
                        0 => forged.input_fingerprint_sha256.push('0'),
                        1 => forged.snapshot_fingerprints[0].1.push('0'),
                        2 => forged.whole_core_payloads.clear(),
                        3 => {
                            ring.local_payload_aliases.pop();
                        }
                        4 => ring.global_payload_count = 1,
                        5 => {
                            ring.spans.pop();
                        }
                        6 => {
                            ring.spans[0].owner_partition_id =
                                (ring.spans[0].owner_partition_id + 1) % tiles.len()
                        }
                        7 => ring.spans[0].boundary_segment_aliases.clear(),
                        8 => ring
                            .spans
                            .iter_mut()
                            .find(|span| !span.coarse_boundary_segment_aliases.is_empty())
                            .unwrap()
                            .coarse_boundary_segment_aliases
                            .clear(),
                        9 => ring.spans[0].payload_edge_index += 1,
                        10 => ring.spans[0].segment.representative_line_id = None,
                        11 => ring.spans[0].segment.source_line_ids.push(999),
                        12 => ring.spans[0].segment.start.z = "0x3ff0000000000000".to_string(),
                        13 => ring.coordinate_bits.last_mut().unwrap()[2] = 1.0f64.to_bits(),
                        _ => ring.spans.swap(0, 1),
                    }
                    assert!(
                        consume_global_non_polygon(
                            &evidence,
                            None,
                            Some(&forged),
                            &mosaic,
                            &tiles,
                            domain,
                            &policy
                        )
                        .unwrap()
                        .is_none(),
                        "cross mutation {mutation}"
                    );
                }
            }
            assert!(consume_global_non_polygon(
                &evidence, None, None, &mosaic, &tiles, domain, &policy
            )
            .unwrap()
            .is_none());
            let mut stale = mosaic.clone();
            stale
                .partitions
                .get_mut(&0)
                .unwrap()
                .generation
                .retry_attempt += 1;
            assert!(consume_global_non_polygon(
                &evidence,
                ownership.as_ref(),
                assembly.as_ref(),
                &stale,
                &tiles,
                domain,
                &policy
            )
            .unwrap()
            .is_none());
            let changed_domain = Rect::new(domain.min(), Coord { x: 12.5, y: 4.0 });
            assert!(consume_global_non_polygon(
                &evidence,
                ownership.as_ref(),
                assembly.as_ref(),
                &mosaic,
                &tiles,
                changed_domain,
                &policy
            )
            .unwrap()
            .is_none());
            let mut changed_tiles = tiles.clone();
            changed_tiles[0] = Rect::new(Coord { x: 0.25, y: 0.0 }, changed_tiles[0].max());
            assert!(consume_global_non_polygon(
                &evidence,
                ownership.as_ref(),
                assembly.as_ref(),
                &mosaic,
                &changed_tiles,
                domain,
                &policy
            )
            .unwrap()
            .is_none());
            for mutation in 0..3 {
                let mut mismatched =
                    prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                        .unwrap()
                        .unwrap();
                match mutation {
                    0 => {
                        mismatched.reference.invalid_rings.pop();
                    }
                    1 => mismatched.reference.dangles.clear(),
                    _ => mismatched.reference.dangles[0][0].z = 1.0,
                }
                assert!(
                    consume_global_non_polygon(
                        &mismatched,
                        ownership.as_ref(),
                        assembly.as_ref(),
                        &mosaic,
                        &tiles,
                        domain,
                        &policy
                    )
                    .unwrap()
                    .is_none(),
                    "equivalence mutation {mutation}"
                );
            }
            for limited in [
                ExecutionPolicy {
                    max_output_coordinates: Some(11),
                    ..Default::default()
                },
                ExecutionPolicy {
                    max_candidate_pairs: Some(0),
                    ..Default::default()
                },
            ] {
                assert!(matches!(
                    consume_global_non_polygon(
                        &evidence,
                        ownership.as_ref(),
                        assembly.as_ref(),
                        &mosaic,
                        &tiles,
                        domain,
                        &limited
                    ),
                    Err(PolygonizeError::ResourceLimitExceeded { .. })
                ));
            }
            for cancel_at in [0, 1, 8] {
                let token = CancellationToken::new();
                let cancelled = ExecutionPolicy {
                    cancellation_token: Some(token.clone()),
                    cancel_at_work_item: Some((token, cancel_at)),
                    ..Default::default()
                };
                assert!(matches!(
                    consume_global_non_polygon(
                        &evidence,
                        ownership.as_ref(),
                        assembly.as_ref(),
                        &mosaic,
                        &tiles,
                        domain,
                        &cancelled
                    ),
                    Err(PolygonizeError::Cancelled { .. })
                ));
            }
            assert_eq!(before, mosaic.fingerprint_sha256());
            assert_eq!(
                serde_json::to_value(&expected).unwrap(),
                serde_json::to_value(
                    consume_global_non_polygon(
                        &evidence,
                        ownership.as_ref(),
                        assembly.as_ref(),
                        &mosaic,
                        &tiles,
                        domain,
                        &policy
                    )
                    .unwrap()
                    .unwrap()
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn mixed_non_polygon_assembly_keeps_whole_core_lineage_and_cross_core_copies() {
        let cross = Geometry::LineString(LineString::from(vec![(3.0, 2.0), (9.0, 2.0)]));
        let cases = [
            (
                vec![
                    Geometry::LineString(LineString::from(vec![(1.0, 1.0), (2.0, 1.0)])),
                    cross.clone(),
                ],
                "dangle",
                true,
                1,
            ),
            (
                vec![
                    Geometry::LineString(LineString::from(vec![
                        (0.5, 0.5),
                        (1.0, 0.5),
                        (1.0, 0.75),
                        (1.0, 1.0),
                        (0.5, 1.0),
                        (0.5, 0.5),
                    ])),
                    Geometry::LineString(LineString::from(vec![
                        (2.0, 0.5),
                        (2.5, 0.5),
                        (2.5, 1.0),
                        (2.0, 1.0),
                        (2.0, 0.75),
                        (2.0, 0.5),
                    ])),
                    Geometry::LineString(LineString::from(vec![(1.0, 0.75), (2.0, 0.75)])),
                    cross.clone(),
                ],
                "cut_edge",
                true,
                1,
            ),
            (
                vec![
                    Geometry::LineString(LineString::from(vec![
                        (0.5, 0.5),
                        (1.5, 1.5),
                        (0.5, 1.5),
                        (1.5, 0.5),
                        (0.5, 0.5),
                    ])),
                    cross,
                ],
                "invalid_ring",
                false,
                2,
            ),
        ];
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 12.0, y: 4.0 });
        for (mut geometries, whole_kind, node_input, multiplicity) in cases {
            let mut signatures = Vec::new();
            for reverse in [false, true] {
                if reverse {
                    geometries.reverse();
                    for geometry in &mut geometries {
                        if let Geometry::LineString(line) = geometry {
                            line.0.reverse();
                        }
                    }
                }
                let mut tiled = TiledPolygonizer::new(domain, 4.0)
                    .with_options(PolygonizerOptions {
                        node_input,
                        ..Default::default()
                    })
                    .with_untiled_equivalence_check();
                for geometry in &geometries {
                    tiled.add_geometry(geometry);
                }
                let ordinary = tiled.polygonize().unwrap();
                let traced = tiled
                    .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
                    .unwrap();
                for result in [&ordinary, &traced.result] {
                    assert!(
                        !result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_ready
                    );
                    assert_eq!(
                        result
                            .stitching_report
                            .partition_border_global_non_polygon_extraction_duplicate_payload_count,
                        multiplicity + 1
                    );
                    assert!(result.stitched_output.is_none());
                }
                let event = &traced
                    .trace
                    .events
                    .iter()
                    .find(|event| event.kind == "partition_border_cross_core_non_polygon_assembly")
                    .unwrap()
                    .payload;
                assert_eq!(
                    event["constructed"], true,
                    "kind={whole_kind} event={event}"
                );
                assert_eq!(event["output_selectable"], false);
                let assembly = &event["assembly"];
                assert_eq!(assembly["payloads"].as_array().unwrap().len(), 1);
                assert_eq!(assembly["whole_core_payloads"].as_array().unwrap().len(), 1);
                let whole = &assembly["whole_core_payloads"][0];
                assert_eq!(whole["payload"]["kind"], whole_kind);
                assert_eq!(whole["payload"]["partition_id"], 0);
                assert_eq!(whole["global_payload_count"], multiplicity);
                assert_eq!(
                    whole["local_payload_indices"].as_array().unwrap().len(),
                    multiplicity
                );
                for edge in whole["edges"].as_array().unwrap() {
                    assert_eq!(edge["segment"]["source_line_ids"], serde_json::json!([0]));
                    assert!(edge["local_segment_aliases"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|alias| alias[0] == 0));
                }
                let crossed = &assembly["payloads"][0];
                assert_eq!(crossed["global_payload_count"], 1);
                assert_eq!(
                    crossed["local_payload_aliases"],
                    serde_json::json!([[0, usize::from(whole_kind == "dangle")], [1, 0], [2, 0]]),
                    "kind={whole_kind}"
                );
                assert_eq!(crossed["spans"].as_array().unwrap().len(), 3);
                let retained_count = whole["local_payload_indices"].as_array().unwrap().len()
                    + crossed["local_payload_aliases"].as_array().unwrap().len();
                let raw_count = traced
                    .result
                    .partition_snapshots
                    .iter()
                    .map(|snapshot| {
                        snapshot.non_polygon.dangles.len()
                            + snapshot.non_polygon.cut_edges.len()
                            + snapshot.non_polygon.invalid_rings.len()
                    })
                    .sum::<usize>();
                assert_eq!(retained_count, raw_count);
                signatures.push((
                    assembly["payloads"].clone(),
                    assembly["whole_core_payloads"].clone(),
                ));
            }
            assert_eq!(signatures[0], signatures[1]);
        }
    }

    #[test]
    fn mixed_non_polygon_assembly_declines_one_bad_whole_core_member_without_partial_output() {
        use crate::tiling::{
            assemble_cross_core_non_polygon, prepare_non_polygon_evidence, PartitionMosaic,
        };
        let geometries = [
            Geometry::LineString(LineString::from(vec![(1.0, 1.0), (2.0, 1.0)])),
            Geometry::LineString(LineString::from(vec![(3.0, 2.0), (5.0, 2.0)])),
        ];
        let refs = geometries
            .iter()
            .map(|geometry| (geometry, None))
            .collect::<Vec<_>>();
        let domain = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 8.0, y: 4.0 });
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let policy = ExecutionPolicy::default();
        let mut tiled = TiledPolygonizer::new(domain, 4.0);
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }
        let result = tiled.polygonize().unwrap();
        let tiles = tiled.generate_tiles().unwrap();
        let mut mosaic = PartitionMosaic::default();
        for snapshot in result.partition_snapshots {
            mosaic.replace_partition(snapshot, &policy).unwrap();
        }
        let evidence =
            prepare_non_polygon_evidence(&mosaic, &tiles, &refs, domain, &options, &policy)
                .unwrap()
                .unwrap();
        let expected = assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
            .unwrap()
            .unwrap();
        assert_eq!(expected.whole_core_payloads.len(), 1);
        for mutation in 0..4 {
            let mut changed = mosaic.clone();
            let owner = changed.partitions.get_mut(&0).unwrap();
            match mutation {
                0 => {
                    owner.non_polygon.dangles.remove(0);
                }
                1 => owner
                    .non_polygon
                    .dangles
                    .push(owner.non_polygon.dangles[0].clone()),
                2 => {
                    let payload = owner.non_polygon.dangles.remove(0);
                    owner.non_polygon.cut_edges.push(payload);
                }
                _ => owner.local_noded_segments[0].representative_line_id = Some(999),
            }
            let before = changed.fingerprint_sha256();
            assert!(
                assemble_cross_core_non_polygon(&evidence, &changed, &tiles, domain, &policy)
                    .unwrap()
                    .is_none()
            );
            if let Some(prepared) =
                prepare_non_polygon_evidence(&changed, &tiles, &refs, domain, &options, &policy)
                    .unwrap()
            {
                assert!(assemble_cross_core_non_polygon(
                    &prepared, &changed, &tiles, domain, &policy
                )
                .unwrap()
                .is_none());
            }
            assert_eq!(changed.fingerprint_sha256(), before);
        }
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(
            assemble_cross_core_non_polygon(
                &evidence,
                &mosaic,
                &tiles,
                domain,
                &ExecutionPolicy {
                    cancellation_token: Some(token),
                    ..Default::default()
                }
            ),
            Err(PolygonizeError::Cancelled { .. })
        ));
        assert_eq!(
            expected,
            assemble_cross_core_non_polygon(&evidence, &mosaic, &tiles, domain, &policy)
                .unwrap()
                .unwrap()
        );
    }

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
        let sink = crate::tiling::partition_source_segment_sink(
            &geometries,
            &[0],
            &ExecutionPolicy::default(),
        )
        .unwrap();

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
        let source_segments = crate::tiling::partition_source_segment_sink(
            &geometries,
            &[0, 1],
            &ExecutionPolicy::default(),
        )
        .unwrap();
        let tiled = TiledPolygonizer::new(bbox, 10.0).with_buffer(1.0);
        let tiles = tiled.generate_tiles().unwrap();
        let (sinks, work) = tiled
            .stream_source_segments_to_partition_sinks(&tiles, &source_segments)
            .unwrap();

        assert_eq!(
            work,
            crate::tiling::PartitionRouterWorkV1 {
                source_segment_count: 2,
                direct_assignment_count: 1,
                slow_path_segment_count: 1,
                candidate_partition_visit_count: 3,
                exact_intersection_test_count: 2,
                emitted_assignment_count: 3,
            }
        );
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
    fn bounds_router_candidate_visits_and_polls_cancellation() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometry = Geometry::LineString(LineString::new(vec![
            Coord { x: 9.5, y: 2.0 },
            Coord { x: 10.5, y: 2.0 },
        ]));
        let geometries = vec![(&geometry, geometry.bounding_rect())];
        let source_segments = crate::tiling::partition_source_segment_sink(
            &geometries,
            &[0],
            &ExecutionPolicy::default(),
        )
        .unwrap();
        let tiles = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(1.0)
            .generate_tiles()
            .unwrap();

        let bounded = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(1.0)
            .with_tile_execution_policy(TileExecutionPolicy {
                max_partition_candidate_visits: Some(1),
                ..Default::default()
            });
        assert!(matches!(
            bounded.stream_source_segments_to_partition_sinks(&tiles, &source_segments),
            Err(PolygonizeError::ResourceLimitExceeded {
                ref stage,
                limit: 1,
                observed: 2,
            }) if stage == "partition_candidate_visits"
        ));

        let token = CancellationToken::new();
        token.cancel();
        let cancelled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(1.0)
            .with_execution_policy(ExecutionPolicy {
                cancellation_token: Some(token),
                ..Default::default()
            });
        assert!(matches!(
            cancelled.stream_source_segments_to_partition_sinks(&tiles, &source_segments),
            Err(PolygonizeError::Cancelled { ref stage })
                if stage == "partition_source_segment_routing"
        ));

        let large_bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 300.0, y: 300.0 });
        let large_geometry = Geometry::LineString(LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 300.0, y: 300.0 },
        ]));
        let large_geometries = vec![(&large_geometry, large_geometry.bounding_rect())];
        let large_source_segments = crate::tiling::partition_source_segment_sink(
            &large_geometries,
            &[0],
            &ExecutionPolicy::default(),
        )
        .unwrap();
        let large_tiled = TiledPolygonizer::new(large_bbox, 10.0).with_buffer(1.0);
        let large_tiles = large_tiled.generate_tiles().unwrap();
        let token = CancellationToken::new();
        let mid_scan = TiledPolygonizer::new(large_bbox, 10.0)
            .with_buffer(1.0)
            .with_execution_policy(ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, 256)),
                ..Default::default()
            });
        assert!(matches!(
            mid_scan.stream_source_segments_to_partition_sinks(
                &large_tiles,
                &large_source_segments
            ),
            Err(PolygonizeError::Cancelled { ref stage })
                if stage == "partition_source_segment_routing"
        ));
    }

    #[test]
    fn compares_router_assignments_with_independent_partition_oracle() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });
        let geometries = [
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 2.0 },
                Coord { x: 8.0, y: 2.0 },
                Coord { x: 18.0, y: 2.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 2.0, y: 18.0 },
                Coord { x: 18.0, y: 2.0 },
            ])),
        ];
        let mut tiled = TiledPolygonizer::new(bbox, 10.0).with_buffer(1.0);
        for geometry in &geometries {
            tiled.add_geometry(geometry);
        }

        let comparison = tiled.partition_router_comparison().unwrap();
        assert_eq!(comparison.schema_version, 1);
        assert_eq!(comparison.oracle_difference, None);
        assert_eq!(comparison.routed_assignment_oracle_difference, None);
        assert_eq!(comparison.routed_local_snapshot_difference, None);
        assert_eq!(comparison.routed_local_snapshot_checked_partition_count, 4);
        assert_eq!(
            comparison.router_work,
            crate::tiling::PartitionRouterWorkV1 {
                source_segment_count: 3,
                direct_assignment_count: 1,
                slow_path_segment_count: 2,
                candidate_partition_visit_count: 7,
                exact_intersection_test_count: 6,
                emitted_assignment_count: 7,
            }
        );
        assert_eq!(comparison.assignments.len(), 4);
        assert!(comparison
            .assignments
            .iter()
            .all(|assignment| assignment.independent_segment_count
                == assignment.routed_segment_count));
        assert_eq!(
            comparison.assignments[0],
            crate::tiling::PartitionRouterAssignmentEvidenceV1 {
                partition_id: 0,
                geometry_envelope_segment_count: 3,
                independent_segment_count: 3,
                routed_segment_count: 3,
                geometry_envelope_false_positive_count: 0,
            }
        );
        assert_eq!(
            comparison.assignments[1],
            crate::tiling::PartitionRouterAssignmentEvidenceV1 {
                partition_id: 1,
                geometry_envelope_segment_count: 3,
                independent_segment_count: 2,
                routed_segment_count: 2,
                geometry_envelope_false_positive_count: 1,
            }
        );
        assert_eq!(comparison.assignments[2].routed_segment_count, 1);
        assert_eq!(comparison.assignments[3].routed_segment_count, 1);
        assert_eq!(
            tiled.benchmark_partition_router().unwrap(),
            comparison.router_work
        );
        assert_eq!(
            serde_json::to_value(&comparison).unwrap()["router_work"]
                ["candidate_partition_visit_count"],
            7
        );
    }

    #[test]
    fn routed_source_segments_preserve_source_chain_metadata() {
        let geometry = Geometry::MultiLineString(MultiLineString(vec![
            LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 1.0, y: 0.0 },
                Coord { x: 2.0, y: 0.0 },
            ]),
            LineString::new(vec![Coord { x: 2.0, y: 0.0 }, Coord { x: 2.0, y: 1.0 }]),
        ]));
        let geometries = vec![(&geometry, geometry.bounding_rect())];
        let sink = crate::tiling::partition_source_segment_sink(
            &geometries,
            &[0],
            &ExecutionPolicy::default(),
        )
        .unwrap();
        let (lines, source_line_strings) = sink.polygonizer_input();
        let mut polygonizer = Polygonizer::new();
        polygonizer.add_source_segments(lines, source_line_strings);
        let traced = polygonizer
            .polygonize_with_trace(TraceLevelV1::Noding, usize::MAX)
            .unwrap();
        let input_events = traced
            .trace
            .events
            .iter()
            .filter(|event| event.kind == "normalized_input_segment")
            .collect::<Vec<_>>();

        assert_eq!(input_events.len(), 3);
        assert_eq!(
            input_events
                .iter()
                .map(|event| event.payload["source_chain_index"].as_u64())
                .collect::<Vec<_>>(),
            vec![Some(0), Some(0), Some(1)]
        );
        assert_eq!(
            input_events
                .iter()
                .map(|event| event.payload["chain_segment_index"].as_u64())
                .collect::<Vec<_>>(),
            vec![Some(0), Some(1), Some(0)]
        );
        assert_eq!(
            input_events
                .iter()
                .map(|event| event.payload["chain_segment_count"].as_u64())
                .collect::<Vec<_>>(),
            vec![Some(2), Some(2), Some(1)]
        );
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
        let mut mosaic = crate::tiling::PartitionMosaic::default();
        for snapshot in &result.partition_snapshots {
            mosaic
                .replace_partition(snapshot.clone(), &ExecutionPolicy::default())
                .unwrap();
        }
        let node_obligations = mosaic
            .boundary_node_obligation_evidence(&ExecutionPolicy::default())
            .unwrap();
        let node = |x: f64, y: f64| {
            node_obligations
                .iter()
                .find(|evidence| {
                    evidence.xy_bits
                        == [
                            crate::utils::canonical_coordinate_bits(x),
                            crate::utils::canonical_coordinate_bits(y),
                        ]
                })
                .unwrap()
        };
        assert_eq!(
            node(1.0, 0.0).status,
            crate::tiling::PartitionBoundaryNodeObligationStatusV1::Valid
        );
        assert_eq!(
            node(1.0, 0.0).obligation,
            Some(crate::tiling::PartitionCorroborationObligationV1 {
                owner_partition_id: 1,
                corroborating_partition_ids: vec![0],
            })
        );
        assert_eq!(
            node(1.0, 1.0).status,
            crate::tiling::PartitionBoundaryNodeObligationStatusV1::Incomplete
        );
        assert_eq!(node(1.0, 1.0).obligation, None);
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

        let (accepted, conformance) = crate::tiling::gate_private_candidate_conformance(
            Some(stitched_output.clone()),
            Some(&expected),
            &ExecutionPolicy::default(),
        )
        .unwrap();
        assert!(accepted.is_some());
        assert_eq!(conformance, stats);

        for mutation in 0..9 {
            let mut mismatch = stitched_output.clone();
            let polygon = &mut mismatch.polygons[0];
            match mutation {
                0 => polygon.exterior_ids[0] = 999,
                1 => polygon.exterior_ids.clear(),
                2 => polygon.interiors_ids.clear(),
                3 => polygon.interiors_ids[0][0] = 999,
                4 => polygon.boundary_source_line_ids.push(999),
                5 => polygon
                    .provenance
                    .as_mut()
                    .unwrap()
                    .boundary_line_ids
                    .push(999),
                6 => polygon.provenance.as_mut().unwrap().input_profile_id = None,
                7 => {
                    polygon.exterior.pop();
                }
                _ => polygon.exterior[1].z = 123.0,
            }
            let stats = crate::tiling::compare_stitched_output_with_untiled(
                Some(&mismatch),
                &geometry_refs,
                &options,
                &ExecutionPolicy::default(),
            )
            .unwrap();
            assert!(stats.checked);
            assert!(!stats.ready, "mutation {mutation}");
            assert_eq!(stats.mismatch_count, 1);
            let (accepted, conformance) = crate::tiling::gate_private_candidate_conformance(
                Some(mismatch),
                Some(&expected),
                &ExecutionPolicy::default(),
            )
            .unwrap();
            assert!(accepted.is_none());
            assert_eq!(conformance, stats);
        }

        for family in 0..3 {
            let mut mismatch = stitched_output.clone();
            let line = vec![Coord3D::new(20.0, 20.0, 1.0), Coord3D::new(21.0, 20.0, 2.0)];
            match family {
                0 => mismatch.dangles.push(line),
                1 => mismatch.cut_edges.push(line),
                _ => mismatch.invalid_rings.push(line),
            }
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
            let (accepted, conformance) = crate::tiling::gate_private_candidate_conformance(
                Some(mismatch),
                Some(&expected),
                &ExecutionPolicy::default(),
            )
            .unwrap();
            assert!(accepted.is_none());
            assert_eq!(conformance, stats);
        }

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
    fn private_candidate_conformance_requires_a_reference_and_preserves_cancellation_and_limits() {
        use crate::tiling::{gate_private_candidate_conformance, TiledUntiledEquivalenceStats};
        let reference = crate::PolygonizerResult {
            polygons: Vec::new(),
            dangles: Vec::new(),
            cut_edges: Vec::new(),
            invalid_rings: Vec::new(),
            diagnostics: None,
        };
        let candidate = TiledStitchedOutput {
            polygons: Vec::new(),
            dangles: Vec::new(),
            cut_edges: Vec::new(),
            invalid_rings: Vec::new(),
        };
        for (output, reference) in [(Some(candidate.clone()), None), (None, Some(&reference))] {
            let (accepted, conformance) =
                gate_private_candidate_conformance(output, reference, &ExecutionPolicy::default())
                    .unwrap();
            assert!(accepted.is_none());
            assert_eq!(conformance, TiledUntiledEquivalenceStats::default());
        }
        for cancel_at in [0, 1] {
            let token = CancellationToken::new();
            let policy = ExecutionPolicy {
                cancellation_token: Some(token.clone()),
                cancel_at_work_item: Some((token, cancel_at)),
                ..Default::default()
            };
            let mut output = candidate.clone();
            output.dangles = vec![vec![Coord3D::new(0.0, 0.0, 0.0)]; 2];
            assert!(matches!(
                gate_private_candidate_conformance(Some(output), Some(&reference), &policy),
                Err(PolygonizeError::Cancelled { .. })
            ));
        }
        let token = CancellationToken::new();
        token.cancel();
        assert!(
            matches!(gate_private_candidate_conformance(None, None, &ExecutionPolicy {
            cancellation_token: Some(token), ..Default::default()
        }), Err(PolygonizeError::Cancelled { ref stage }) if stage == "tiled_private_candidate_conformance")
        );
        let mut oversized = candidate;
        oversized.dangles.push(vec![
            Coord3D::new(0.0, 0.0, 0.0);
            crate::options::MAX_UNCANCELLABLE_SORT_ITEMS + 1
        ]);
        assert!(
            matches!(gate_private_candidate_conformance(Some(oversized), Some(&reference), &ExecutionPolicy {
            cancellation_token: Some(CancellationToken::new()), ..Default::default()
        }), Err(PolygonizeError::ResourceLimitExceeded { ref stage, .. }) if stage == "tiled_untiled_equivalence_dangles")
        );
    }

    #[test]
    fn conformance_canonicalizes_coordinates_and_edge_ids_together() {
        let options = PolygonizerOptions::default();
        let mut polygonizer = Polygonizer::with_options(options);
        for (base, side) in [(0.0, 10.0), (2.0, 6.0)] {
            let vertices = [
                Coord3D::new(base, base, 4.0),
                Coord3D::new(base + side, base, 4.0),
                Coord3D::new(base + side, base + side, 4.0),
                Coord3D::new(base, base + side, 4.0),
            ];
            polygonizer.add_lines(
                (0..4)
                    .map(|index| {
                        Line3D::new(
                            vertices[index],
                            vertices[(index + 1) % 4],
                            base as u32 * 10 + index as u32 + 1,
                        )
                    })
                    .collect(),
            );
        }
        let expected = polygonizer.polygonize().unwrap();
        let policy = ExecutionPolicy::default();
        let keys = crate::tiling::canonical_polygon_output_keys(&expected.polygons, &policy)
            .unwrap()
            .unwrap();
        let mut changed = expected.polygons.clone();
        for polygon in &mut changed {
            for (ring, ids) in std::iter::once((&mut polygon.exterior, &mut polygon.exterior_ids))
                .chain(polygon.interiors.iter_mut().zip(&mut polygon.interiors_ids))
            {
                ring.pop();
                ring.reverse();
                ids.reverse();
                ids.rotate_left(1);
                ring.rotate_left(2);
                ids.rotate_left(2);
                ring.push(ring[0]);
            }
        }
        assert_eq!(
            Some(keys.clone()),
            crate::tiling::canonical_polygon_output_keys(&changed, &policy).unwrap()
        );
        changed[0].exterior_ids.rotate_left(1);
        assert_ne!(
            Some(keys),
            crate::tiling::canonical_polygon_output_keys(&changed, &policy).unwrap()
        );
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

        assert_eq!(snapshot.schema_version, 15);
        snapshot.validate_for_mosaic().unwrap();
        assert_eq!(snapshot.partition_id, 0);
        assert_eq!(snapshot.ownership_domain_min.x, "0x0000000000000000");
        assert_eq!(snapshot.ownership_domain_max.x, "0x4000000000000000");
        assert_eq!(snapshot.generation.buffer_bits, 0.0f64.to_bits());
        assert_eq!(snapshot.generation.retry_attempt, 0);
        assert!(snapshot.declared_adjacencies.is_empty());
        assert_eq!(snapshot.execution.max_parallel_tiles, Some(1));
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
            .process_one_partition(0, bbox, &[bbox], serial.buffer, 0)
            .unwrap();
        assert_eq!(snapshot, &independent);
        assert_eq!(snapshot.diff(&independent), None);
        assert_eq!(serial.partition_oracle_first_difference().unwrap(), None);
        let mut mosaic = crate::tiling::PartitionMosaic::default();
        let commit = mosaic
            .replace_partition(independent.clone(), &ExecutionPolicy::default())
            .unwrap();
        assert_eq!(commit.partition_id, 0);
        assert!(!commit.replaced_existing);
        assert!(commit.changed);
        assert_eq!(commit.partition_count, 1);
        assert_eq!(
            commit.physical_span_claim_count,
            independent.atomic_observations.len()
        );
        assert_eq!(commit.physical_span_count, mosaic.physical_spans.len());
        assert_eq!(
            mosaic.physical_span_claim_count(),
            independent.atomic_observations.len()
        );
        assert_eq!(
            commit.mosaic_fingerprint_sha256,
            mosaic.fingerprint_sha256()
        );
        let committed_fingerprint = mosaic.fingerprint_sha256();
        let mut invalid_replacement = independent.clone();
        invalid_replacement.schema_version += 1;
        assert!(mosaic
            .replace_partition(invalid_replacement, &ExecutionPolicy::default())
            .is_err());
        assert_eq!(mosaic.fingerprint_sha256(), committed_fingerprint);
        let purge = mosaic
            .purge_partition(0, &ExecutionPolicy::default())
            .unwrap();
        assert!(purge.removed);
        assert_eq!(purge.partition_count, 0);
        assert_eq!(purge.physical_span_count, 0);
        assert_eq!(purge.physical_span_claim_count, 0);
        assert_eq!(purge.mosaic_fingerprint_sha256, mosaic.fingerprint_sha256());
        mosaic
            .replace_partition(independent.clone(), &ExecutionPolicy::default())
            .unwrap();
        let mut corroborating = independent.clone();
        corroborating.partition_id = 1;
        for observation in &mut corroborating.atomic_observations {
            observation.observation_id.partition_id = 1;
            if let Some(face_ref) = &mut observation.face_ref {
                face_ref[0] = 1;
            }
            if let Some(successor) = &mut observation.local_face_boundary_successor {
                successor.partition_id = 1;
            }
        }
        for graph in &mut corroborating.local_face_graphs {
            graph.partition_id = 1;
            for edge in &mut graph.directed_edges {
                if let Some(face_ref) = &mut edge.face_ref {
                    face_ref[0] = 1;
                }
            }
        }
        for node in &mut corroborating.boundary_nodes {
            for face_ref in &mut node.face_refs {
                face_ref[0] = 1;
            }
        }
        corroborating.validate_for_mosaic().unwrap();
        let before_conflict = mosaic.fingerprint_sha256();
        let conflict = mosaic
            .replace_partition(corroborating.clone(), &ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            conflict,
            crate::tiling::PartitionMosaicErrorV1::PhysicalConflict {
                witness: crate::tiling::PartitionPhysicalSpanWitnessV1 {
                    payload_class: crate::tiling::PartitionPhysicalSpanPayloadClassV1::Adjacency,
                    ..
                }
            }
        ));
        assert_eq!(mosaic.fingerprint_sha256(), before_conflict);
        for observation in &mut corroborating.atomic_observations {
            observation.side ^= 1;
            std::mem::swap(&mut observation.from_xy_bits, &mut observation.to_xy_bits);
            std::mem::swap(&mut observation.from_z_bits, &mut observation.to_z_bits);
            if let Some(successor) = &mut observation.local_face_boundary_successor {
                std::mem::swap(
                    &mut successor.edge_start_xy_bits,
                    &mut successor.edge_end_xy_bits,
                );
            }
        }
        mosaic
            .replace_partition(corroborating, &ExecutionPolicy::default())
            .unwrap();
        assert_eq!(
            mosaic.physical_span_claim_count(),
            independent.atomic_observations.len() * 2
        );
        assert!(mosaic.physical_spans.values().all(|claims| {
            claims
                .iter()
                .map(|claim| claim.partition_id)
                .collect::<BTreeSet<_>>()
                == BTreeSet::from([0, 1])
        }));
        assert!(mosaic.physical_span_evidence().iter().all(|evidence| {
            let obligation = evidence.obligation.as_ref().unwrap();
            let claims = &mosaic.physical_spans[&evidence.span];
            evidence.status == crate::tiling::PartitionPhysicalSpanStatusV1::Valid
                && claims.iter().any(|claim| {
                    claim.partition_id == obligation.owner_partition_id
                        && matches!(claim.observation.side, 0 | 2)
                })
                && obligation.corroborating_partition_ids.len() == 1
                && claims.iter().any(|claim| {
                    claim.partition_id == obligation.corroborating_partition_ids[0]
                        && matches!(claim.observation.side, 1 | 3)
                })
                && evidence.witness.is_none()
        }));
        assert!(mosaic
            .boundary_node_obligation_evidence(&ExecutionPolicy::default())
            .unwrap()
            .iter()
            .any(|evidence| {
                evidence.status == crate::tiling::PartitionBoundaryNodeObligationStatusV1::Conflict
            }));
        assert!(mosaic.topology_span_evidence().iter().all(|evidence| {
            evidence.status == crate::tiling::PartitionTopologySpanStatusV1::Ambiguous
                && evidence.ready_observation_ids.is_empty()
        }));

        let mut ready_snapshots = [independent.clone(), mosaic.partitions[&1].clone()];
        for snapshot in &mut ready_snapshots {
            let mut retained_spans = BTreeSet::new();
            snapshot.atomic_observations.retain(|observation| {
                observation.face_ref.is_some()
                    && retained_spans.insert(
                        crate::tiling::PartitionPhysicalSpanKeyV1::from_observation(observation)
                            .unwrap(),
                    )
            });
        }
        let mut ready_mosaic = crate::tiling::PartitionMosaic::default();
        for snapshot in ready_snapshots {
            ready_mosaic
                .replace_partition(snapshot, &ExecutionPolicy::default())
                .unwrap();
        }
        assert!(ready_mosaic
            .topology_span_evidence()
            .iter()
            .all(|evidence| {
                evidence.status == crate::tiling::PartitionTopologySpanStatusV1::Ready
                    && evidence.ready_observation_ids.len() == 2
            }));
        assert_eq!(
            ready_mosaic.topology_ready_observation_ids().len(),
            ready_mosaic.physical_spans.len() * 2
        );
        assert!(mosaic.topology_ready_observation_ids().is_empty());
        let mut source_conflict = mosaic.partitions[&1].clone();
        source_conflict.atomic_observations[0]
            .source_line_ids
            .push(u32::MAX);
        let before_source_conflict = mosaic.fingerprint_sha256();
        let conflict = mosaic
            .replace_partition(source_conflict, &ExecutionPolicy::default())
            .unwrap_err();
        assert!(matches!(
            conflict,
            crate::tiling::PartitionMosaicErrorV1::PhysicalConflict {
                witness: crate::tiling::PartitionPhysicalSpanWitnessV1 {
                    partition_ids,
                    payload_class:
                        crate::tiling::PartitionPhysicalSpanPayloadClassV1::SourceLineIds,
                    ..
                }
            } if partition_ids == vec![0, 1]
        ));
        assert_eq!(mosaic.fingerprint_sha256(), before_source_conflict);
        assert!(mosaic.physical_span_evidence().iter().all(|evidence| {
            evidence.status == crate::tiling::PartitionPhysicalSpanStatusV1::Valid
                && evidence.obligation.is_some()
        }));
        mosaic
            .purge_partition(1, &ExecutionPolicy::default())
            .unwrap();
        assert!(mosaic.physical_span_evidence().iter().all(|evidence| {
            evidence.status == crate::tiling::PartitionPhysicalSpanStatusV1::Incomplete
                && evidence.obligation.is_none()
                && evidence.witness.as_ref().unwrap().claim_count > 0
        }));
        let mut mismatch = independent.clone();
        mismatch.selected_input_geometry_indices.push(1);
        assert_eq!(
            snapshot.diff(&mismatch).unwrap().path,
            "$.selected_input_geometry_indices"
        );
        let mut generation_mismatch = independent.clone();
        generation_mismatch.generation.retry_attempt += 1;
        assert_eq!(
            snapshot.diff(&generation_mismatch).unwrap().path,
            "$.generation.retry_attempt"
        );
        let mut execution_mismatch = independent.clone();
        execution_mismatch.execution.max_graph_nodes = Some(1);
        assert_eq!(
            snapshot.diff(&execution_mismatch).unwrap().path,
            "$.execution"
        );
        let mut unsupported_schema = independent.clone();
        unsupported_schema.schema_version += 1;
        assert!(unsupported_schema.validate_for_mosaic().is_err());
        let mut foreign_lineage = independent.clone();
        foreign_lineage.local_face_graphs[0].partition_id += 1;
        assert!(foreign_lineage.validate_for_mosaic().is_err());
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
            let comparison = tiled.partition_router_comparison().unwrap();
            assert_eq!(comparison.oracle_difference, None, "{label}");
            assert_eq!(
                comparison.routed_assignment_oracle_difference, None,
                "{label}"
            );
            assert_eq!(comparison.routed_local_snapshot_difference, None, "{label}");
            let result = tiled.polygonize().unwrap();
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
            let comparison = tiled.partition_router_comparison().unwrap();
            assert_eq!(comparison.oracle_difference, None, "{label}");
            assert_eq!(
                comparison.routed_assignment_oracle_difference, None,
                "{label}"
            );
            assert_eq!(comparison.routed_local_snapshot_difference, None, "{label}");
            let result = tiled.polygonize().unwrap();
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
    fn partition_snapshot_retains_domain_adjacency_and_execution_evidence() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 2.0, y: 1.0 });
        let tiled = TiledPolygonizer::new(bbox, 1.0)
            .with_execution_policy(ExecutionPolicy {
                max_graph_nodes: Some(101),
                max_tile_retry_attempts: Some(2),
                ..Default::default()
            })
            .with_tile_execution_policy(TileExecutionPolicy {
                max_tiles: Some(2),
                max_parallel_tiles: Some(1),
                ..Default::default()
            });
        let result = tiled.polygonize().unwrap();

        assert_eq!(result.partition_snapshots.len(), 2);
        assert_eq!(
            result.partition_snapshots[0].declared_adjacencies,
            vec![PartitionDeclaredAdjacencyV1 {
                neighbor_partition_id: 1,
                side: 1,
                neighbor_side: 0,
                coordinate_bits: 1.0f64.to_bits(),
            }]
        );
        assert_eq!(
            result.partition_snapshots[1].declared_adjacencies,
            vec![PartitionDeclaredAdjacencyV1 {
                neighbor_partition_id: 0,
                side: 0,
                neighbor_side: 1,
                coordinate_bits: 1.0f64.to_bits(),
            }]
        );
        for snapshot in &result.partition_snapshots {
            assert_eq!(snapshot.ownership_domain_min.x, "0x0000000000000000");
            assert_eq!(snapshot.ownership_domain_max.x, "0x4000000000000000");
            assert_eq!(snapshot.execution.max_graph_nodes, Some(101));
            assert_eq!(snapshot.execution.max_tile_retry_attempts, Some(2));
            assert_eq!(snapshot.execution.max_tiles, Some(2));
            snapshot.validate_for_mosaic().unwrap();
        }

        let mut mosaic = crate::tiling::PartitionMosaic::default();
        mosaic
            .replace_partition(
                result.partition_snapshots[0].clone(),
                &ExecutionPolicy::default(),
            )
            .unwrap();
        let before_incompatible = mosaic.fingerprint_sha256();
        let mut domain_mismatch = result.partition_snapshots[1].clone();
        domain_mismatch.ownership_domain_max.x = "0x4008000000000000".to_string();
        assert!(matches!(
            mosaic.replace_partition(domain_mismatch, &ExecutionPolicy::default()),
            Err(
                crate::tiling::PartitionMosaicErrorV1::IncompatibleSnapshot {
                    reason: "ownership domain differs from existing mosaic",
                    ..
                }
            )
        ));
        assert_eq!(mosaic.fingerprint_sha256(), before_incompatible);

        let mut adjacency_mismatch = result.partition_snapshots[1].clone();
        adjacency_mismatch.declared_adjacencies.clear();
        assert!(matches!(
            mosaic.replace_partition(adjacency_mismatch, &ExecutionPolicy::default()),
            Err(
                crate::tiling::PartitionMosaicErrorV1::IncompatibleSnapshot {
                    reason: "declared adjacency has no reciprocal snapshot evidence",
                    ..
                }
            )
        ));
        assert_eq!(mosaic.fingerprint_sha256(), before_incompatible);

        let mut options_mismatch = result.partition_snapshots[1].clone();
        options_mismatch.topology.options = serde_json::json!({"mismatch": true});
        assert!(matches!(
            mosaic.replace_partition(options_mismatch, &ExecutionPolicy::default()),
            Err(
                crate::tiling::PartitionMosaicErrorV1::IncompatibleSnapshot {
                    reason: "semantic options differ from existing mosaic",
                    ..
                }
            )
        ));
        assert_eq!(mosaic.fingerprint_sha256(), before_incompatible);
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

        let tiles = result
            .tile_reports
            .iter()
            .map(|report| report.tile_bbox)
            .collect::<Vec<_>>();
        for (partition_id, report) in result.tile_reports.iter().enumerate() {
            let independent = tiled
                .process_one_partition(partition_id, report.tile_bbox, &tiles, tiled.buffer, 0)
                .unwrap();
            assert_eq!(
                result.partition_snapshots[partition_id], independent,
                "partition {partition_id} differs from independent bounded scan"
            );
        }

        let snapshots = [0, 5, 15].map(|index| result.partition_snapshots[index].clone());
        let mut fingerprints = BTreeSet::new();
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let mut mosaic = crate::tiling::PartitionMosaic::default();
            for index in order {
                mosaic
                    .replace_partition(snapshots[index].clone(), &ExecutionPolicy::default())
                    .unwrap();
            }
            let fingerprint = mosaic.fingerprint_sha256();
            let unchanged = mosaic
                .replace_partition(snapshots[1].clone(), &ExecutionPolicy::default())
                .unwrap();
            assert!(unchanged.replaced_existing);
            assert!(!unchanged.changed);
            assert_eq!(unchanged.mosaic_fingerprint_sha256, fingerprint);

            mosaic
                .purge_partition(snapshots[1].partition_id, &ExecutionPolicy::default())
                .unwrap();
            mosaic
                .replace_partition(snapshots[1].clone(), &ExecutionPolicy::default())
                .unwrap();
            assert_eq!(mosaic.fingerprint_sha256(), fingerprint);

            let cancellation = CancellationToken::new();
            cancellation.cancel();
            assert!(mosaic
                .purge_partition(
                    snapshots[1].partition_id,
                    &ExecutionPolicy {
                        cancellation_token: Some(cancellation),
                        ..Default::default()
                    },
                )
                .is_err());
            assert_eq!(mosaic.fingerprint_sha256(), fingerprint);
            fingerprints.insert(fingerprint);
        }
        assert_eq!(fingerprints.len(), 1);
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
            &[bbox],
            &components,
            None,
            &std::sync::atomic::AtomicUsize::new(0),
        ) {
            Ok(_) => panic!("expected bulk partition processing to fail"),
            Err(error) => error,
        };
        let independent_error = tiled
            .process_one_partition(0, bbox, &[bbox], tiled.buffer, 0)
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
            tiled.process_tile(0, bbox, &[bbox], &[], 0.0, 0, None),
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
            tiled.process_tile(0, bbox, &[bbox], &[], 0.0, 0, None),
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
            tiled.process_tile(0, bbox, &[bbox], &[component], 0.0, 0, None),
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
        assert!(result.partition_snapshots.iter().all(|snapshot| {
            snapshot.generation.buffer_bits == 42.0f64.to_bits()
                && snapshot.generation.retry_attempt == 1
        }));
        assert_eq!(
            tiled
                .partition_oracle_first_difference_for_result(&result)
                .unwrap(),
            None
        );
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
