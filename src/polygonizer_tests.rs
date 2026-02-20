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

        let polygons = poly.polygonize().unwrap();
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

        let polygons = poly.polygonize().unwrap();
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

        let polygons = poly.polygonize().expect("Polygonization failed");
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

        let polygons = poly.polygonize().expect("Polygonization failed");

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
            .expect("Polygonization should not fail on empty input");
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
            .expect("Polygonization should not fail on empty LineString");
        assert_eq!(polygons.len(), 0);
    }

    #[test]
    fn test_polygonize_point() {
        let mut poly = Polygonizer::new();
        // Add a single point (should be ignored by extract_lines)
        poly.add_geometry(geo_types::Geometry::Point(geo_types::Point::new(0.0, 0.0)));
        let polygons = poly
            .polygonize()
            .expect("Polygonization should not fail on Point input");
        assert_eq!(polygons.len(), 0);
    }
}
