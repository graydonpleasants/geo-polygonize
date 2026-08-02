#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::tiling::InputComponent;
    use crate::{
        trace::TraceLevelV1, CancellationToken, Coord3D, DedupPolicy, ExecutionPolicy,
        NodingGuarantee, NodingOptions, Polygon3D, PolygonizeError, Polygonizer,
        PolygonizerOptions, PrecisionModel, ProvenanceOptions, TileBoundarySide,
        TileComponentConnection, TileCoverageGuarantee, TileExcludedComponentIssue, TileReport,
        TileRetryPolicy, TiledPolygonizeError, TiledPolygonizer,
    };
    use geo::{Contains, Coord, Geometry, LineString, Rect};

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

        let polys = tiler.polygonize().unwrap().polygons;

        // Should find 4 polygons
        assert_eq!(polys.len(), 4);

        // Check areas
        for p in polys {
            assert!((p.unsigned_area_2d() - 100.0).abs() < 1e-6);
        }
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
    fn records_declined_component_fallback_for_owned_face_evidence() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 10.0 });
        let face = Geometry::LineString(LineString::new(vec![
            Coord { x: 1.0, y: 2.0 },
            Coord { x: 19.0, y: 2.0 },
            Coord { x: 19.0, y: 8.0 },
            Coord { x: 1.0, y: 8.0 },
            Coord { x: 1.0, y: 2.0 },
        ]));
        let mut tiled = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(2.0)
            .with_component_fallback();
        tiled.add_geometry(&face);

        let result = tiled.polygonize().unwrap();
        assert!(result.stitching_report.component_fallback_attempted);
        assert!(!result.stitching_report.component_fallback_used);
        assert!(result.stitching_report.unresolved_owned_polygon_count > 0);

        let traced = tiled
            .polygonize_with_trace(TraceLevelV1::Full, usize::MAX)
            .unwrap();
        let declined = traced
            .trace
            .events
            .iter()
            .find(|event| event.kind == "tile_component_fallback_declined")
            .unwrap();
        assert_eq!(declined.payload["reason"], "no_indexed_component_evidence");
        assert_eq!(
            declined.payload["unresolved_owned_polygon_count"],
            result.stitching_report.unresolved_owned_polygon_count
        );
        assert_eq!(
            declined.payload["unresolved_input_geometry_count"],
            result.stitching_report.unresolved_input_geometry_count
        );
        assert_eq!(declined.payload["unresolved_component_count"], 0);
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
            tiled.process_tile(bbox, &[], 0.0, None),
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
            tiled.process_tile(bbox, &[], 0.0, None),
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
        let mut tiles = tiler.generate_tiles();
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
