#![no_main]

use arbitrary::Arbitrary;
use geo_polygonize_core::options::{
    DedupPolicy, PolygonizerOptions, TileOwnershipPolicy, TilingOptions,
};
use geo_polygonize_core::polygonizer::polygonize_with_options;
use geo_polygonize_core::types::{Coord3D, Line3D};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    lines: Vec<(f64, f64, f64, f64, f64, f64)>,
}

fuzz_target!(|input: FuzzInput| {
    if input.lines.len() > 100 {
        return;
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
        tiling: Some(TilingOptions {
            ownership_policy: TileOwnershipPolicy::Centroid,
            dedup_policy: DedupPolicy::CanonicalRingHash,
        }),
        ..Default::default()
    };

    let _ = polygonize_with_options(&lines, &options);
});
