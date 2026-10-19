use geo_polygonize_core::{
    polygonize, Coord3D, Line3D, PolygonizerOptions, PrecisionModel, SnapStrategy,
};
use std::time::Instant;
fn main() {
    let mut lines = Vec::new();
    for i in 0..200 {
        let x = (i % 20) as f64 * 3.;
        let y = (i / 20) as f64 * 3.;
        let points = [(x, y), (x + 1., y), (x + 1., y + 1.), (x, y + 1.), (x, y)];
        for pair in points.windows(2) {
            lines.push(Line3D::new(
                Coord3D::new(pair[0].0, pair[0].1, 0.),
                Coord3D::new(pair[1].0, pair[1].1, 0.),
                0,
            ));
        }
    }
    let options = PolygonizerOptions {
        node_input: true,
        pre_snap_tolerance: 0.01,
        precision_model: PrecisionModel::FixedGrid { grid_size: 0.1 },
        snap_strategy: SnapStrategy::GeosCompat,
        ..Default::default()
    };
    for _ in 0..5 {
        std::hint::black_box(polygonize(lines.iter().copied(), &options).unwrap());
    }
    let start = Instant::now();
    for _ in 0..100 {
        std::hint::black_box(polygonize(lines.iter().copied(), &options).unwrap());
    }
    println!("{}", start.elapsed().as_nanos());
}
