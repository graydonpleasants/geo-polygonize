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
        assert!(polygons.len() >= 1);
        let triangle = polygons
            .iter()
            .find(|p| p.unsigned_area() > 49.0 && p.unsigned_area() < 51.0);
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
            .find(|p| (p.unsigned_area() - 64.0).abs() < 1.0);
        assert!(donut.is_some(), "Donut polygon not found");
        assert_eq!(donut.unwrap().interiors().len(), 1);

        let island = polygons
            .iter()
            .find(|p| (p.unsigned_area() - 36.0).abs() < 1.0);
        assert!(island.is_some(), "Island polygon not found");
    }

    #[test]
    fn test_noding_crossing_lines() {
        let mut poly = Polygonizer::new();
        poly.node_input = true;

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
            .filter(|p| (p.unsigned_area() - 25.0).abs() < 1e-6)
            .count();
        assert_eq!(triangles_count, 4, "Expected 4 triangles of area 25");
    }

    #[test]
    fn test_noding_collinear_lines() {
        let mut poly = Polygonizer::new();
        poly.node_input = true;

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
            .find(|p| (p.unsigned_area() - 50.0).abs() < 1e-6);
        assert!(
            rect.is_some(),
            "Expected rectangle of area 50 from collinear overlap"
        );
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
        poly.node_input = true;
        let polygons = poly
            .polygonize()
            .expect("Polygonization should not fail on empty input with noding")
            .polygons;
        assert_eq!(polygons.len(), 0);
    }

    #[test]
    fn test_polygonize_empty_linestring_with_noding() {
        let mut poly = Polygonizer::new();
        poly.node_input = true;
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
        poly.node_input = true;
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
            .find(|p| (p.unsigned_area() - 72.0).abs() < 1.0);
        assert!(
            shell_with_hole.is_some(),
            "Expected outer shell area near 72 with assigned concave hole"
        );
        assert_eq!(shell_with_hole.unwrap().interiors().len(), 1);
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
            .find(|p| (p.unsigned_area() - 98.0).abs() < 1.0);
        assert!(
            shell_with_hole.is_some(),
            "Point-touch hole should be retained on parent shell"
        );
        assert_eq!(shell_with_hole.unwrap().interiors().len(), 1);
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
            .find(|p| (p.unsigned_area() - 100.0).abs() < 1.0);
        assert!(
            outer.is_some(),
            "Edge-touch hole should not be assigned to the parent shell"
        );
        assert_eq!(outer.unwrap().interiors().len(), 0);
    }

    #[test]
    fn test_extract_only_polygonal_nested() {
        let mut poly = Polygonizer::new();
        poly.extract_only_polygonal = true;

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
        assert!((polygons[0].unsigned_area() - 100.0).abs() < 1e-6);
        assert_eq!(polygons[0].interiors().len(), 0);
    }

    #[test]
    fn test_extract_only_polygonal_disjoint() {
        let mut poly = Polygonizer::new();
        poly.extract_only_polygonal = true;

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
}
