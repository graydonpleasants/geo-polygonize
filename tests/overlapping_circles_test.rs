#[cfg(test)]
mod tests {
    use geo_polygonize::Polygonizer;
    use geo_types::{Coord, LineString, Geometry};

    #[test]
    fn test_overlapping_circles_count() {
        // Create 3 overlapping circles
        let mut polygonizer = Polygonizer::new();
        polygonizer.node_input = true;

        let r = 10.0;
        let centers = vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (5.0, 8.66),
        ];

        for (cx, cy) in centers {
            let mut coords = Vec::new();
            for i in 0..60 { // Resolution
                let angle = (i as f64) * (360.0 / 60.0f64).to_radians();
                coords.push(Coord {
                    x: cx + r * angle.cos(),
                    y: cy + r * angle.sin(),
                });
            }
            coords.push(coords[0]); // Close
            polygonizer.add_geometry(Geometry::LineString(LineString::new(coords)));
        }

        let polys = polygonizer.polygonize().unwrap();
        // Expect 7 polygons: 1 center, 3 petals, 3 outer crescents
        // The extra 8th polygon (Outer Face / Union) which has 0 area after hole assignment is now filtered out.
        assert_eq!(polys.len(), 7, "Should find exactly 7 polygons for 3 overlapping circles");
    }
}
