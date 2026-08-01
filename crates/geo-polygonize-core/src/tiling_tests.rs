#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::{
        trace::TraceLevelV1, CancellationToken, Coord3D, DedupPolicy, ExecutionPolicy,
        PolygonizeError, Polygonizer, PolygonizerOptions, ProvenanceOptions, TileBoundarySide,
        TileComponentConnection, TileCoverageGuarantee, TileRetryPolicy, TiledPolygonizeError,
        TiledPolygonizer,
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
    fn documents_fallback_component_global_containment_boundary() {
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
            });
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
        let tiled = tiled.polygonize().unwrap();
        assert_eq!(tiled.polygons.len(), 1);
        assert!(tiled.polygons[0].interiors.is_empty());
        assert_eq!(tiled.stitching_report.retry_exhausted_tile_count, 4);
        assert!(tiled
            .tile_reports
            .iter()
            .any(|report| !report.excluded_component_issues.is_empty()));
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
