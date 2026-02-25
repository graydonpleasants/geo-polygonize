use geo::{Coord, Line};
use geo_polygonize_core::noding::snap::SnapNoder;

#[test]
fn test_nan_deduplication_failure() {
    let noder = SnapNoder::new(0.001);
    let l1 = Line::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 });
    let l_nan = Line::new(
        Coord {
            x: f64::NAN,
            y: 0.0,
        },
        Coord { x: 10.0, y: 10.0 },
    );
    let l_inf = Line::new(
        Coord {
            x: f64::INFINITY,
            y: 0.0,
        },
        Coord { x: 10.0, y: 10.0 },
    );

    // We pass 3 lines: l1, l_nan, l_inf.
    // If noding works correctly (deduplication + filtering), we expect 1 line (l1).
    let lines = vec![l1, l_nan, l_inf];

    let result = noder.node(lines);

    // NaNs and Infs should be filtered out.
    assert_eq!(
        result.len(),
        1,
        "Expected 1 line after filtering NaNs and Infs, found {}",
        result.len()
    );
    assert_eq!(result[0], l1, "Remaining line should be the valid one");
}
