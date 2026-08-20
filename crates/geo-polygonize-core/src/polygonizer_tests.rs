#[cfg(test)]
mod tests {
    use crate::Polygonizer;
    use geo::Area;
    use geo_types::LineString;

    #[test]
    fn test_polygonize_simple_triangle() {
        let mut poly = Polygonizer::new();
        poly.add_geometry(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]).into());
        poly.add_geometry(LineString::from(vec![(10.0, 0.0), (0.0, 10.0)]).into());
        poly.add_geometry(LineString::from(vec![(0.0, 10.0), (0.0, 0.0)]).into());

        let polygons = poly.polygonize().unwrap().polygons;
        assert!(!polygons.is_empty());
        let triangle = polygons.iter().find(|p| {
            let p2d = p.to_polygon_2d();
            p2d.unsigned_area() > 49.0 && p2d.unsigned_area() < 51.0
        });
        assert!(triangle.is_some());
    }

    #[test]
    fn test_polygonize_hole() {
        let mut poly = Polygonizer::new();
        // Outer square
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // Inner square
        poly.add_geometry(
            LineString::from(vec![
                (2.0, 2.0),
                (2.0, 8.0),
                (8.0, 8.0),
                (8.0, 2.0),
                (2.0, 2.0),
            ])
            .into(),
        );

        let polygons = poly.polygonize().unwrap().polygons;
        assert_eq!(
            polygons.len(),
            2,
            "Expected 2 polygons, found {}",
            polygons.len()
        );

        let donut = polygons
            .iter()
            .find(|p| (p.unsigned_area_2d() - 64.0).abs() < 1.0);
        assert!(donut.is_some(), "Donut polygon not found");
        assert_eq!(donut.unwrap().interiors.len(), 1);

        let island = polygons
            .iter()
            .find(|p| (p.unsigned_area_2d() - 36.0).abs() < 1.0);
        assert!(island.is_some(), "Island polygon not found");
    }

    #[test]
    fn test_noding_crossing_lines() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = true;

        // Frame
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // Diagonals
        poly.add_geometry(LineString::from(vec![(0.0, 0.0), (10.0, 10.0)]).into());
        poly.add_geometry(LineString::from(vec![(0.0, 10.0), (10.0, 0.0)]).into());

        let polygons = poly.polygonize().expect("Polygonization failed").polygons;
        // Frame (empty because triangles are holes) + 4 Triangles
        // Wait, the logic assigns holes to shells.
        // Frame is OuterCCW (100) and OuterCW (-100).
        // Triangles are InnerCCW (25) and InnerCW (-25).
        // 4 Triangles (CW) are holes of Frame (OuterCCW).
        // Area = 100 - 4*25 = 0.
        // The Frame has Area 0 and is filtered out by the Polygonizer (tolerance 1e-6).
        // 4 Triangles (CCW) are shells. Area 25.
        // So we get:
        // 1. Triangle 1 (Area 25)
        // 2. Triangle 2 (Area 25)
        // 3. Triangle 3 (Area 25)
        // 4. Triangle 4 (Area 25)

        assert_eq!(
            polygons.len(),
            4,
            "Expected 4 polygons, found {}",
            polygons.len()
        );
        let triangles_count = polygons
            .iter()
            .filter(|p| (p.unsigned_area_2d() - 25.0).abs() < 1e-6)
            .count();
        assert_eq!(triangles_count, 4, "Expected 4 triangles of area 25");
    }

    #[test]
    fn test_noding_collinear_lines() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = true;

        // 1. Line (0,0)->(10,0)
        // 2. Line (5,0)->(15,0) (Overlap 5..10)
        // 3. Line (10,0)->(10,10)->(5,10)->(5,0) (To close the rectangle with the overlap)

        // The overlap is on (5,0) to (10,0).
        // If handled correctly, we should get:
        // - Segment (0,0)-(5,0)
        // - Segment (5,0)-(10,0) (Double covered but graph should unique-ify edges or handle overlap?)
        // - Segment (10,0)-(15,0)
        // - And the rest of the box.

        // We expect a rectangle (5,0)-(10,0)-(10,10)-(5,10)-(5,0). Area 50.

        poly.add_geometry(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]).into());
        poly.add_geometry(LineString::from(vec![(5.0, 0.0), (15.0, 0.0)]).into());
        poly.add_geometry(
            LineString::from(vec![(10.0, 0.0), (10.0, 10.0), (5.0, 10.0), (5.0, 0.0)]).into(),
        );

        let polygons = poly.polygonize().expect("Polygonization failed").polygons;

        // Should find the rectangle of area 50.
        let rect = polygons
            .iter()
            .find(|p| (p.unsigned_area_2d() - 50.0).abs() < 1e-6);
        assert!(
            rect.is_some(),
            "Expected rectangle of area 50 from collinear overlap"
        );
    }

    #[test]
    fn test_figure_8_pinching_bowtie() {
        let mut poly = Polygonizer::new();
        // Self-intersecting bowtie that forms exactly two valid cycles.
        // Node_input = true is needed to create the intersection node at (5, 5).
        poly.options_mut().node_input = true;
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 10.0),
                (10.0, 0.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        let polygons = poly.polygonize().expect("Polygonization failed").polygons;
        // The intersection is at (5,5).
        // Cycle 1: (0,0)-(5,5)-(0,10)-(0,0). Area = 25.
        // Cycle 2: (10,10)-(5,5)-(10,0)-(10,10). Area = 25.
        // Both are valid polygons.
        assert_eq!(
            polygons.len(),
            2,
            "Expected 2 polygons from figure-8 pinching, found {}",
            polygons.len()
        );

        let area1 = polygons[0].to_polygon_2d().unsigned_area();
        let area2 = polygons[1].to_polygon_2d().unsigned_area();
        assert!((area1 - 25.0).abs() < 1e-6);
        assert!((area2 - 25.0).abs() < 1e-6);
    }

    #[test]
    fn test_polygonize_empty_input() {
        let mut poly = Polygonizer::new();
        let polygons = poly
            .polygonize()
            .expect("Polygonization should not fail on empty input")
            .polygons;
        assert_eq!(polygons.len(), 0);
    }

    #[test]
    fn test_polygonize_empty_linestring() {
        let mut poly = Polygonizer::new();
        // Add an empty LineString
        poly.add_geometry(geo_types::Geometry::LineString(geo_types::LineString::new(
            vec![],
        )));
        let polygons = poly
            .polygonize()
            .expect("Polygonization should not fail on empty LineString")
            .polygons;
        assert_eq!(polygons.len(), 0);
    }

    #[test]
    fn test_polygonize_point() {
        let mut poly = Polygonizer::new();
        // Add a single point (should be ignored by extract_lines)
        poly.add_geometry(geo_types::Geometry::Point(geo_types::Point::new(0.0, 0.0)));
        let polygons = poly
            .polygonize()
            .expect("Polygonization should not fail on Point input")
            .polygons;
        assert_eq!(polygons.len(), 0);
    }

    #[test]
    fn test_polygonize_empty_input_with_noding() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = true;
        let polygons = poly
            .polygonize()
            .expect("Polygonization should not fail on empty input with noding")
            .polygons;
        assert_eq!(polygons.len(), 0);
    }

    #[test]
    fn test_polygonize_empty_linestring_with_noding() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = true;
        poly.add_geometry(geo_types::Geometry::LineString(geo_types::LineString::new(
            vec![],
        )));
        let polygons = poly
            .polygonize()
            .expect("Polygonization should not fail on empty LineString with noding")
            .polygons;
        assert_eq!(polygons.len(), 0);
    }

    #[test]
    fn test_polygonize_point_with_noding() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = true;
        poly.add_geometry(geo_types::Geometry::Point(geo_types::Point::new(0.0, 0.0)));
        let polygons = poly
            .polygonize()
            .expect("Polygonization should not fail on Point input with noding")
            .polygons;
        assert_eq!(polygons.len(), 0);
    }

    #[test]
    fn test_concave_hole_uses_interior_probe_not_centroid() {
        let mut poly = Polygonizer::new();

        // Outer shell (CCW)
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // Concave C-shaped hole (CW). Its centroid lies outside of the ring.
        poly.add_geometry(
            LineString::from(vec![
                (3.0, 3.0),
                (3.0, 9.0),
                (9.0, 9.0),
                (9.0, 7.0),
                (5.0, 7.0),
                (5.0, 5.0),
                (9.0, 5.0),
                (9.0, 3.0),
                (3.0, 3.0),
            ])
            .into(),
        );

        let polygons = poly.polygonize().expect("Polygonization failed").polygons;

        let shell_with_hole = polygons
            .iter()
            .find(|p| (p.unsigned_area_2d() - 72.0).abs() < 1.0);
        assert!(
            shell_with_hole.is_some(),
            "Expected outer shell area near 72 with assigned concave hole"
        );
        assert_eq!(shell_with_hole.unwrap().interiors.len(), 1);
    }

    #[test]
    fn test_point_touch_hole_is_kept() {
        let mut poly = Polygonizer::new();

        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // CW hole touching shell at one vertex (5,0)
        poly.add_geometry(
            LineString::from(vec![(5.0, 0.0), (4.0, 2.0), (6.0, 2.0), (5.0, 0.0)]).into(),
        );

        let polygons = poly.polygonize().expect("Polygonization failed").polygons;

        let shell_with_hole = polygons
            .iter()
            .find(|p| (p.unsigned_area_2d() - 98.0).abs() < 1.0);
        assert!(
            shell_with_hole.is_some(),
            "Point-touch hole should be retained on parent shell"
        );
        assert_eq!(shell_with_hole.unwrap().interiors.len(), 1);
    }

    #[test]
    fn test_edge_touch_hole_is_dropped() {
        let mut poly = Polygonizer::new();

        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // CW hole sharing an entire edge segment with the shell boundary.
        poly.add_geometry(
            LineString::from(vec![
                (2.0, 0.0),
                (2.0, 2.0),
                (4.0, 2.0),
                (4.0, 0.0),
                (2.0, 0.0),
            ])
            .into(),
        );

        let polygons = poly.polygonize().expect("Polygonization failed").polygons;

        let outer = polygons
            .iter()
            .find(|p| (p.unsigned_area_2d() - 100.0).abs() < 1.0);
        assert!(
            outer.is_some(),
            "Edge-touch hole should not be assigned to the parent shell"
        );
        assert_eq!(outer.unwrap().interiors.len(), 0);
    }

    #[test]
    fn test_extract_only_polygonal_nested() {
        let mut poly = Polygonizer::new();
        poly.options_mut().extract_only_polygonal = true;

        // Outer square (0,0)-(10,0)-(10,10)-(0,10)-(0,0) (Area 100)
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // Inner square (2,2)-(8,2)-(8,8)-(2,8)-(2,2) (Area 36)
        poly.add_geometry(
            LineString::from(vec![
                (2.0, 2.0),
                (8.0, 2.0),
                (8.0, 8.0),
                (2.0, 8.0),
                (2.0, 2.0),
            ])
            .into(),
        );

        let result = poly.polygonize().expect("Polygonization failed");
        let polygons = result.polygons;

        // Default behavior would return 2 polygons.
        // With extract_only_polygonal=true, the inner shell should be discarded.
        assert_eq!(polygons.len(), 1, "Expected 1 polygon (outer)");
        assert!((polygons[0].to_polygon_2d().unsigned_area() - 64.0).abs() < 1e-6);
        assert_eq!(polygons[0].interiors.len(), 1);
    }

    #[test]
    fn test_extract_only_polygonal_disjoint() {
        let mut poly = Polygonizer::new();
        poly.options_mut().extract_only_polygonal = true;

        // Poly 1
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // Poly 2 (Disjoint)
        poly.add_geometry(
            LineString::from(vec![
                (20.0, 0.0),
                (30.0, 0.0),
                (30.0, 10.0),
                (20.0, 10.0),
                (20.0, 0.0),
            ])
            .into(),
        );

        let result = poly.polygonize().expect("Polygonization failed");
        assert_eq!(result.polygons.len(), 2);
    }

    #[test]
    fn test_extract_only_polygonal_concentric_squares() {
        let mut poly = Polygonizer::new();
        poly.options_mut().extract_only_polygonal = true;

        // Square 1: Outer, area 100
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // Square 2: Middle (hole of Square 1), area 64
        // Hole winding should be CW
        poly.add_geometry(
            LineString::from(vec![
                (1.0, 1.0),
                (1.0, 9.0),
                (9.0, 9.0),
                (9.0, 1.0),
                (1.0, 1.0),
            ])
            .into(),
        );

        // Square 3: Inner (shell inside Square 2), area 36
        // Shell winding should be CCW
        poly.add_geometry(
            LineString::from(vec![
                (2.0, 2.0),
                (8.0, 2.0),
                (8.0, 8.0),
                (2.0, 8.0),
                (2.0, 2.0),
            ])
            .into(),
        );

        let result = poly.polygonize().expect("Polygonization failed");
        let polygons = result.polygons;

        // With extract_only_polygonal=true:
        // Square 1 is an outer shell (depth 0) -> kept.
        // Square 2 is a hole.
        // Square 3 is a shell inside Square 2 (depth 1) -> dropped.
        assert_eq!(polygons.len(), 1, "Expected 1 polygon (outer)");
        assert!((polygons[0].to_polygon_2d().unsigned_area() - 36.0).abs() < 1e-6);
        assert_eq!(polygons[0].interiors.len(), 0);

        // Now, let's add a fourth square
        let mut poly2 = Polygonizer::new();
        poly2.options_mut().extract_only_polygonal = true;

        poly2.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );
        poly2.add_geometry(
            LineString::from(vec![
                (1.0, 1.0),
                (1.0, 9.0),
                (9.0, 9.0),
                (9.0, 1.0),
                (1.0, 1.0),
            ])
            .into(),
        );
        poly2.add_geometry(
            LineString::from(vec![
                (2.0, 2.0),
                (8.0, 2.0),
                (8.0, 8.0),
                (2.0, 8.0),
                (2.0, 2.0),
            ])
            .into(),
        );
        // Square 4: inner-most hole, area 16
        poly2.add_geometry(
            LineString::from(vec![
                (3.0, 3.0),
                (3.0, 7.0),
                (7.0, 7.0),
                (7.0, 3.0),
                (3.0, 3.0),
            ])
            .into(),
        );
        // Square 5: innermost shell, area 4
        poly2.add_geometry(
            LineString::from(vec![
                (4.0, 4.0),
                (6.0, 4.0),
                (6.0, 6.0),
                (4.0, 6.0),
                (4.0, 4.0),
            ])
            .into(),
        );

        let result2 = poly2.polygonize().expect("Polygonization failed");
        let polygons2 = result2.polygons;

        // With extract_only_polygonal=true:
        // Square 1 (depth 0) -> kept (has Square 2 as hole)
        // Square 3 (depth 1) -> dropped
        // Square 5 (depth 2) -> kept (has no holes)
        assert_eq!(
            polygons2.len(),
            2,
            "Expected 2 polygons (outermost and innermost)"
        );
        let areas: std::collections::HashSet<_> = polygons2
            .iter()
            .map(|p| p.to_polygon_2d().unsigned_area() as i64)
            .collect();
        assert!(areas.contains(&16), "Areas: {:?}", areas);
        assert!(areas.contains(&4), "Areas: {:?}", areas);
    }

    #[test]
    fn test_small_rings_are_preserved() {
        let mut poly = Polygonizer::new();

        // 1. Valid Square (10x10)
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        // 2. Tiny Ring 1 (formerly discarded by the absolute area cutoff)
        // A triangle with base 1e-5 and height 1e-5 has area 0.5e-10.
        poly.add_geometry(
            LineString::from(vec![
                (20.0, 0.0),
                (20.00001, 0.0),
                (20.0, 0.00001),
                (20.0, 0.0),
            ])
            .into(),
        );

        // 3. Tiny Ring 2 (Inside Tiny Ring 1? No, let's make them disjoint first to test capture)
        // Another tiny one.
        poly.add_geometry(
            LineString::from(vec![
                (30.0, 0.0),
                (30.00001, 0.0),
                (30.0, 0.00001),
                (30.0, 0.0),
            ])
            .into(),
        );

        let result = poly.polygonize().expect("Polygonization failed");
        assert_eq!(result.polygons.len(), 3);
        assert!(result.invalid_rings.is_empty());
    }

    #[test]
    fn test_repeated_polygonize_preserves_input_segments_when_node_input_disabled() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = false;
        poly.options_mut().diagnostics.enabled = true;

        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );

        let first = poly.polygonize().expect("First polygonization failed");
        let first_diagnostics = first.diagnostics.unwrap();
        assert_eq!(first_diagnostics.input_segment_count, 4);
        assert_eq!(first_diagnostics.noded_segment_count, 4);

        let second = poly.polygonize().expect("Second polygonization failed");
        let second_diagnostics = second.diagnostics.unwrap();
        assert_eq!(second_diagnostics.input_segment_count, 4);
        assert_eq!(second_diagnostics.noded_segment_count, 4);
    }

    #[test]
    fn diagnostics_report_component_memory_evidence_without_changing_output() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = false;
        poly.options_mut().diagnostics.enabled = true;
        for offset in [0.0, 20.0] {
            poly.add_geometry(
                LineString::from(vec![
                    (offset, 0.0),
                    (offset + 10.0, 0.0),
                    (offset + 10.0, 10.0),
                    (offset, 10.0),
                    (offset, 0.0),
                ])
                .into(),
            );
        }

        let result = poly.polygonize().unwrap();
        let stats = poly.component_memory_stats();
        assert_eq!(result.polygons.len(), 2);
        assert_eq!(stats.component_count, 2);
        assert_eq!(stats.active_node_count, 8);
        assert_eq!(stats.largest_component_edge_count, 4);
        assert!(stats.scratch_instance_count >= 1);
        assert!(stats.max_merged_output_item_count > 0);
    }

    #[test]
    fn component_memory_shape_contract_is_shared_by_feature_builds() {
        // This test intentionally runs in both the default (parallel) and
        // --no-default-features (serial) builds. Shape/capacity evidence must
        // remain stable; worker and scratch-state cardinality are execution
        // details and are checked only as bounded values.
        let mut poly = Polygonizer::new();
        poly.options_mut().diagnostics.enabled = true;
        for offset in [0.0, 20.0] {
            poly.add_geometry(
                LineString::from(vec![
                    (offset, 0.0),
                    (offset + 10.0, 0.0),
                    (offset + 10.0, 10.0),
                    (offset, 10.0),
                    (offset, 0.0),
                ])
                .into(),
            );
        }

        let result = poly.polygonize().unwrap();
        let stats = poly.component_memory_stats();
        assert_eq!(result.polygons.len(), 2);
        assert_eq!(stats.component_count, 2);
        assert_eq!(stats.active_node_count, 8);
        assert_eq!(stats.active_edge_count, 8);
        assert_eq!(stats.largest_component_node_count, 4);
        assert_eq!(stats.largest_component_edge_count, 4);
        assert_eq!(stats.partition_node_capacity, 8);
        assert_eq!(stats.partition_edge_capacity, 8);
        assert_eq!(stats.global_graph_node_capacity, 8);
        assert_eq!(stats.global_graph_edge_capacity, 8);
        assert_eq!(stats.global_graph_directed_edge_capacity, 16);
        assert_eq!(stats.global_graph_adjacency_capacity, 32);
        assert_eq!(stats.max_scratch_node_capacity, 4);
        assert_eq!(stats.max_scratch_edge_capacity, 4);
        assert_eq!(stats.max_scratch_directed_edge_capacity, 8);
        assert_eq!(stats.max_scratch_adjacency_capacity, 16);
        assert_eq!(stats.max_scratch_global_node_capacity, 4);
        assert_eq!(stats.max_scratch_local_node_capacity, 7);
        assert_eq!(stats.max_scratch_global_dir_edge_capacity, 4);
        assert_eq!(stats.max_merged_output_item_count, 4);
        assert_eq!(stats.max_merged_output_coordinate_capacity, 20);
        assert!(stats.scratch_instance_count >= 1);
        assert!(stats.execution_worker_count >= 1);
    }

    #[test]
    fn diagnostics_report_noding_iterations() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = true;
        poly.options_mut().diagnostics.enabled = true;
        poly.add_geometry(LineString::from(vec![(0.0, 0.0), (10.0, 10.0)]).into());
        poly.add_geometry(LineString::from(vec![(0.0, 10.0), (10.0, 0.0)]).into());

        let diagnostics = poly.polygonize().unwrap().diagnostics.unwrap();

        assert_eq!(diagnostics.noding_iterations.len(), 1);
        assert_eq!(diagnostics.noding_iterations[0].intersections_found, 2);
        assert_eq!(diagnostics.noding_iterations[0].nodes_added, 2);
        assert_eq!(diagnostics.intersection_stats.interpolated_intersections, 2);
        assert_eq!(diagnostics.intersection_stats.exact_intersections, 1);
        assert_eq!(diagnostics.noding_work_stats.candidate_pairs, 1);
        assert_eq!(diagnostics.noding_work_stats.aabb_rejections, 0);
        assert_eq!(diagnostics.noding_work_stats.exact_intersection_calls, 1);
        assert_eq!(diagnostics.noding_work_stats.split_events, 2);
    }

    #[test]
    fn timing_only_diagnostics_skip_work_counters() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = true;
        poly.options_mut().diagnostics.timings = true;
        poly.add_geometry(LineString::from(vec![(0.0, 0.0), (10.0, 10.0)]).into());
        poly.add_geometry(LineString::from(vec![(0.0, 10.0), (10.0, 0.0)]).into());

        let diagnostics = poly.polygonize().unwrap().diagnostics.unwrap();

        assert_eq!(diagnostics.input_segment_count, 2);
        assert!(diagnostics.noding_iterations.is_empty());
        assert_eq!(diagnostics.noding_work_stats.candidate_pairs, 0);
    }

    #[test]
    fn diagnostics_report_pre_snap_candidates() {
        let mut poly = Polygonizer::new();
        poly.options_mut().node_input = true;
        poly.options_mut().pre_snap_tolerance = 0.5;
        poly.options_mut().diagnostics.enabled = true;
        poly.add_geometry(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]).into());
        poly.add_geometry(LineString::from(vec![(5.0, 0.4), (5.0, 1.0)]).into());

        let diagnostics = poly.polygonize().unwrap().diagnostics.unwrap();

        assert!(diagnostics.noding_work_stats.pre_snap_vertex_candidates > 0);
    }

    #[test]
    fn diagnostics_report_containment_work() {
        let mut poly = Polygonizer::new();
        poly.options_mut().diagnostics.enabled = true;
        poly.add_geometry(
            LineString::from(vec![
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 10.0),
                (0.0, 10.0),
                (0.0, 0.0),
            ])
            .into(),
        );
        poly.add_geometry(
            LineString::from(vec![
                (2.0, 2.0),
                (2.0, 8.0),
                (8.0, 8.0),
                (8.0, 2.0),
                (2.0, 2.0),
            ])
            .into(),
        );

        let stats = poly
            .polygonize()
            .unwrap()
            .diagnostics
            .unwrap()
            .containment_stats;

        assert!(stats.prepared_shells > 0);
        assert!(stats.envelope_candidates > 0);
        assert!(stats.point_in_ring_calls > 0);
        assert!(stats.point_in_ring_calls <= stats.envelope_candidates);
        assert_eq!(stats.max_point_in_ring_calls_per_shell, 1);
        assert_eq!(stats.shells_with_64_plus_point_in_ring_calls, 0);
    }

    #[test]
    fn test_nested_small_rings_are_preserved() {
        let mut poly = Polygonizer::new();

        // Ring A: Tiny but "outer" relative to B.
        // Area = 1e-10.
        let ring_a = LineString::from(vec![
            (0.0, 0.0),
            (1e-5, 0.0),
            (1e-5, 1e-5),
            (0.0, 1e-5),
            (0.0, 0.0),
        ]);

        // Ring B (Inner): Area is smaller and is contained in A.
        let ring_b = LineString::from(vec![
            (0.2e-5, 0.2e-5),
            (0.8e-5, 0.2e-5),
            (0.8e-5, 0.8e-5),
            (0.2e-5, 0.8e-5),
            (0.2e-5, 0.2e-5),
        ]);

        poly.add_geometry(ring_a.into());
        poly.add_geometry(ring_b.into());

        let result = poly.polygonize().expect("Polygonization failed");
        assert_eq!(result.polygons.len(), 2);
        assert!(result.invalid_rings.is_empty());
    }

    mod serial_parallel_conformance {
        use crate::utils::parallel::{par_flat_map_dispatches, reset_par_flat_map_dispatches};
        use crate::{
            normalize_polygonize_error, polygonize, Coord3D, DiagnosticsOptions, Line3D,
            NodingGuarantee, NodingOptions, NormalizedPolygonizeErrorV1, PolygonizerOptions,
            ProvenanceOptions, TopologyFingerprintV1, ZOptions, ZPolicy,
        };
        use serde::{Deserialize, Serialize};
        use sha2::{Digest, Sha256};

        #[derive(Deserialize)]
        struct Fixture {
            options: Option<PolygonizerOptions>,
            #[serde(default)]
            profile_id: Option<String>,
            inputs: Vec<FixtureLine>,
        }

        #[derive(Deserialize)]
        struct FixtureLine {
            start: FixtureCoordinate,
            end: FixtureCoordinate,
            id: u32,
        }

        #[derive(Deserialize)]
        struct FixtureCoordinate {
            x: f64,
            y: f64,
            z: f64,
        }

        #[derive(Debug, Serialize)]
        struct ConformanceEvidenceV1 {
            schema_version: u32,
            cases: Vec<CaseEvidenceV1>,
        }

        #[derive(Debug, Serialize)]
        struct CaseEvidenceV1 {
            name: &'static str,
            outcome: OutcomeV1,
        }

        #[derive(Debug, Serialize)]
        #[serde(tag = "status", rename_all = "snake_case")]
        enum OutcomeV1 {
            Success {
                fingerprint_sha256: String,
            },
            Error {
                normalized: Box<NormalizedPolygonizeErrorV1>,
            },
        }

        fn fixture(source: &str) -> (Vec<Line3D>, PolygonizerOptions) {
            let fixture: Fixture = serde_json::from_str(source).unwrap();
            let mut options = fixture.options.unwrap_or_default();
            options.diagnostics = DiagnosticsOptions {
                enabled: true,
                ..Default::default()
            };
            options.provenance = ProvenanceOptions {
                enabled: true,
                include_boundary_line_ids: true,
            };
            options.input_profile_id = fixture.profile_id;
            let lines = fixture
                .inputs
                .into_iter()
                .map(|line| {
                    Line3D::new(
                        Coord3D::new(line.start.x, line.start.y, line.start.z),
                        Coord3D::new(line.end.x, line.end.y, line.end.z),
                        line.id,
                    )
                })
                .collect();
            (lines, options)
        }

        fn outcome(
            name: &'static str,
            lines: Vec<Line3D>,
            options: PolygonizerOptions,
        ) -> CaseEvidenceV1 {
            let outcome = match polygonize(lines, &options) {
                Ok(result) => {
                    let fingerprint =
                        TopologyFingerprintV1::try_from_result(&result, &options).unwrap();
                    let bytes = serde_json::to_vec(&fingerprint).unwrap();
                    OutcomeV1::Success {
                        fingerprint_sha256: format!("{:x}", Sha256::digest(bytes)),
                    }
                }
                Err(error) => OutcomeV1::Error {
                    normalized: Box::new(normalize_polygonize_error(&error)),
                },
            };
            CaseEvidenceV1 { name, outcome }
        }

        #[test]
        fn representative_outcomes_match_the_shared_feature_build_snapshot() {
            reset_par_flat_map_dispatches();

            let mut cases = Vec::new();
            for (name, source) in [
                (
                    "square_with_hole",
                    include_str!("../tests/fixtures/basic/square_with_hole.json"),
                ),
                (
                    "bowtie",
                    include_str!("../tests/fixtures/dirty/bowtie.json"),
                ),
                (
                    "reported_output_families",
                    include_str!("../tests/fixtures/topology/reported_outputs.json"),
                ),
                (
                    "provenance_with_profile",
                    include_str!("../tests/fixtures/provenance/mixed_boundary_with_profile.json"),
                ),
                (
                    "z_ignore_conflicts",
                    include_str!("../tests/fixtures/z/ignore_conflicts.json"),
                ),
            ] {
                let (lines, options) = fixture(source);
                cases.push(outcome(name, lines, options));
            }

            let crossing = vec![
                Line3D::new(Coord3D::new(-1.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 1),
                Line3D::new(Coord3D::new(0.0, -1.0, 0.0), Coord3D::new(0.0, 1.0, 0.0), 2),
            ];
            cases.push(outcome(
                "noding_validation_failure",
                crossing,
                PolygonizerOptions {
                    noding: NodingOptions {
                        guarantee: NodingGuarantee::Validate,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ));
            cases.push(outcome(
                "invalid_option",
                Vec::new(),
                PolygonizerOptions {
                    pre_snap_tolerance: -1.0,
                    ..Default::default()
                },
            ));
            let (z_conflicts, mut z_options) =
                fixture(include_str!("../tests/fixtures/z/ignore_conflicts.json"));
            z_options.z = ZOptions {
                policy: ZPolicy::ErrorOnConflict,
                conflict_tolerance: 0.0,
            };
            cases.push(outcome("z_conflict", z_conflicts, z_options));

            let actual = serde_json::to_value(ConformanceEvidenceV1 {
                schema_version: 1,
                cases,
            })
            .unwrap();
            let expected: serde_json::Value = serde_json::from_str(include_str!(
                "../tests/fixtures/conformance/serial_parallel_outcomes_v1.json"
            ))
            .unwrap();
            assert_eq!(
                actual,
                expected,
                "feature-build conformance evidence changed:\n{}",
                serde_json::to_string_pretty(&actual).unwrap()
            );

            if cfg!(feature = "parallel") && !cfg!(target_arch = "wasm32") {
                assert!(
                    par_flat_map_dispatches() > 0,
                    "parallel feature build did not exercise the Rayon graph-construction path"
                );
            } else {
                assert_eq!(
                    par_flat_map_dispatches(),
                    0,
                    "serial feature build unexpectedly exercised the Rayon graph-construction path"
                );
            }
        }
    }
}
