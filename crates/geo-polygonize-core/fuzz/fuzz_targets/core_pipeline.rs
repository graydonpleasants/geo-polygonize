#![no_main]

use arbitrary::Arbitrary;
use geo_polygonize_core::options::{PolygonizerOptions, SnapStrategy};
use geo_polygonize_core::polygonizer::polygonize;
use geo_polygonize_core::types::{Coord3D, Line3D};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    lines: Vec<(f64, f64, f64, f64, f64, f64)>,
    node_input: bool,
    snap_grid_size: f64,
}

fuzz_target!(|input: FuzzInput| {
    if input.lines.len() > 100 {
        return; // limit line count to avoid timeouts
    }

    let mut lines = Vec::new();
    for (sx, sy, sz, ex, ey, ez) in input.lines {
        if sx.is_nan() || sy.is_nan() || sz.is_nan() || ex.is_nan() || ey.is_nan() || ez.is_nan() {
            return;
        }
        lines.push(Line3D {
            start: Coord3D {
                x: sx,
                y: sy,
                z: sz,
            },
            end: Coord3D {
                x: ex,
                y: ey,
                z: ez,
            },
            line_id: 0,
        });
    }

    let options = PolygonizerOptions {
        node_input: input.node_input,
        snap_grid_size: input.snap_grid_size.abs().clamp(1e-10, 1.0), // reasonable grid size
        snap_strategy: SnapStrategy::Grid,
        ..Default::default()
    };

    let _ = polygonize(lines.iter().copied(), &options);
});
