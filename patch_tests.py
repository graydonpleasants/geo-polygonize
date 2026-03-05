import re

content = open("crates/geo-polygonize-core/src/types.rs").read()

new_tests = """
    #[test]
    fn test_centroid_winding_independence() {
        // Exterior is CCW (positive area)
        let ext = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];

        // Hole is also CCW (usually holes are CW, but we want to test independence)
        let hole = vec![
            Coord3D::new(2.0, 2.0, 0.0),
            Coord3D::new(8.0, 2.0, 0.0),
            Coord3D::new(8.0, 8.0, 0.0),
            Coord3D::new(2.0, 8.0, 0.0),
            Coord3D::new(2.0, 2.0, 0.0),
        ];

        let poly = Polygon3D::new(ext, vec![hole], vec![], vec![vec![]]);
        let centroid = poly.centroid_2d().unwrap();
        // Since it's a symmetric hole in a symmetric square, the centroid should be exactly at the center (5, 5).
        // If winding independence failed, it might add instead of subtract or produce a wildly wrong result.
        assert!((centroid.x() - 5.0).abs() < 1e-6);
        assert!((centroid.y() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_centroid_numeric_stability_at_large_offsets() {
        let offset = 10_000_000.0;
        let ext = vec![
            Coord3D::new(offset, offset, 0.0),
            Coord3D::new(offset + 0.001, offset, 0.0),
            Coord3D::new(offset + 0.001, offset + 0.001, 0.0),
            Coord3D::new(offset, offset + 0.001, 0.0),
            Coord3D::new(offset, offset, 0.0),
        ];

        let poly = Polygon3D::new(ext, vec![], vec![], vec![]);
        let centroid = poly.centroid_2d().unwrap();

        // Centroid should be exactly at the center of the small square
        let expected_x = offset + 0.0005;
        let expected_y = offset + 0.0005;

        // If there is catastrophic cancellation, the error will be large relative to the small dimensions.
        assert!((centroid.x() - expected_x).abs() < 1e-10);
        assert!((centroid.y() - expected_y).abs() < 1e-10);
    }
}
"""

if content.endswith("}\n"):
    content = content[:-2] + new_tests
    open("crates/geo-polygonize-core/src/types.rs", "w").write(content)
    print("Replaced successfully")
else:
    print("Could not append tests")
