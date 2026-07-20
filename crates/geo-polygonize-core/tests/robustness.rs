use geo::Geometry;
use geo_polygonize_core::{Coord3D, Line3D, Polygonizer};
use geo_types::{Coord, LineString};

#[test]
fn test_bowtie_noding() {
    // A bowtie shape: (0,0) -> (10,10) -> (10,0) -> (0,10) -> (0,0)
    // Intersection at (5,5).
    let ls = LineString(vec![
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 10.0 },
        Coord { x: 10.0, y: 0.0 },
        Coord { x: 0.0, y: 10.0 },
        Coord { x: 0.0, y: 0.0 },
    ]);

    let mut poly = Polygonizer::new();
    poly.options_mut().node_input = true;
    poly.options_mut().precision_model =
        geo_polygonize_core::options::PrecisionModel::FixedGrid { grid_size: 1e-6 };
    poly.add_geometry(Geometry::LineString(ls));

    let results = poly.polygonize().expect("Polygonization failed").polygons;

    println!("Bowtie Results: {}", results.len());
    for (i, p) in results.iter().enumerate() {
        println!("Poly {}: {:?}", i, p);
    }

    assert_eq!(results.len(), 2, "Expected 2 polygons from bowtie");
}

#[test]
fn test_duplicate_edge_removal() {
    let mut poly = Polygonizer::new();
    poly.options_mut().node_input = true;
    poly.options_mut().precision_model =
        geo_polygonize_core::options::PrecisionModel::FixedGrid { grid_size: 1e-6 };

    // Triangle edge 1
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 0.0 },
    ])));
    // Duplicate edge 1
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 0.0, y: 0.0 },
        Coord { x: 10.0, y: 0.0 },
    ])));

    // Edge 2
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 10.0, y: 0.0 },
        Coord { x: 5.0, y: 5.0 },
    ])));
    // Edge 3
    poly.add_geometry(Geometry::LineString(LineString(vec![
        Coord { x: 5.0, y: 5.0 },
        Coord { x: 0.0, y: 0.0 },
    ])));

    let results = poly.polygonize().expect("Polygonization failed").polygons;
    assert_eq!(results.len(), 1);
}

#[test]
fn test_nan_handling_in_snap_noder() {
    use geo_polygonize_core::noding::snap::{NodingStrategy, SnapNoder};

    let noder = SnapNoder::new(1.0).with_strategy(NodingStrategy::Scalar);

    // Create lines with NaN coordinates
    let lines = vec![
        Line3D::new(
            Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            Coord3D {
                x: 10.0,
                y: 10.0,
                z: 0.0,
            },
            0,
        ),
        Line3D::new(
            Coord3D {
                x: 0.0,
                y: 10.0,
                z: 0.0,
            },
            Coord3D {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            },
            0,
        ),
        // Line with NaN
        Line3D::new(
            Coord3D {
                x: f64::NAN,
                y: 0.0,
                z: 0.0,
            },
            Coord3D {
                x: 5.0,
                y: 5.0,
                z: 0.0,
            },
            0,
        ),
    ];

    // This should not panic or hang
    let result = std::panic::catch_unwind(|| noder.node(lines));

    match result {
        Ok(processed_lines) => {
            println!("Processed {} lines", processed_lines.len());
            for line in processed_lines {
                if line.start.x.is_nan()
                    || line.start.y.is_nan()
                    || line.end.x.is_nan()
                    || line.end.y.is_nan()
                {
                    panic!("Output contains NaN coordinates");
                }
            }
        }
        Err(_) => {
            panic!("SnapNoder panicked on NaN input");
        }
    }
}
