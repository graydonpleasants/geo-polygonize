use geo_polygonize_core::{Polygonizer, Line3D, Coord3D};
use geo_polygonize_core::noding::snap::{NodingStrategy, SnapNoder};

#[test]
fn test_reproduce_nan_in_noder() {
    let noder = SnapNoder::new(1.0).with_strategy(NodingStrategy::Scalar);

    // Create lines with NaN coordinates
    let l1: Line3D = Line3D::new(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(10.0, 10.0, 0.0)
    );
    let l2: Line3D = Line3D::new(
        Coord3D::new(0.0, 10.0, 0.0),
        Coord3D::new(10.0, 0.0, 0.0)
    );
    let l3: Line3D = Line3D::new(
        Coord3D::new(f64::NAN, 0.0, 0.0),
        Coord3D::new(5.0, 5.0, 0.0)
    );

    let lines = vec![l1, l2, l3];

    // This should not panic or hang, and should filter out the invalid line
    let result = noder.node(lines);

    assert!(result.len() > 0, "Should return valid lines");

    // Just verify no NaNs
    for line in &result {
        assert!(!line.start.x.is_nan());
        assert!(!line.start.y.is_nan());
        assert!(!line.end.x.is_nan());
        assert!(!line.end.y.is_nan());
    }

    // Should verify l1 is in result (potentially split)
    // l1 starts at (0,0) ends at (10,10). Split at (5,5).
    // So we expect (0,0)->(5,5) and (5,5)->(10,10)
    // l1 is Line3D. result contains Line3D.

    let l1_part = Line3D::new(
        Coord3D::new(0.0, 0.0, 0.0),
        Coord3D::new(5.0, 5.0, 0.0)
    );

    // Check if l1_part is in result
    let found = result.iter().any(|l|
        l.start.x == l1_part.start.x && l.start.y == l1_part.start.y &&
        l.end.x == l1_part.end.x && l.end.y == l1_part.end.y
    );
    assert!(found);
}
