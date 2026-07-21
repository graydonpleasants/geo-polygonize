#![no_main]

use arbitrary::Arbitrary;
use geo_polygonize_core::{DiagnosticsOptions, PolygonizerOptions, ProvenanceOptions};
use geo_polygonize_core::polygonize;
use geo_polygonize_core::{Coord3D, Line3D};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    lines: Vec<(f64, f64, f64, f64, f64, f64)>,
    node_input: bool,
    enabled: bool,
    report_mode: bool,
}

fuzz_target!(|input: FuzzInput| {
    if input.lines.len() > 100 {
        return;
    }

    let mut lines = Vec::new();
    for (i, (sx, sy, sz, ex, ey, ez)) in input.lines.into_iter().enumerate() {
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
            line_id: i as u32,
        });
    }

    let options = PolygonizerOptions {
        node_input: input.node_input,
        provenance: ProvenanceOptions {
            enabled: true,
            include_boundary_line_ids: true,
        },
        diagnostics: DiagnosticsOptions {
            enabled: input.enabled,
            report_mode: input.report_mode,
            timings: false,
        },
        ..Default::default()
    };

    let _ = polygonize(lines.iter().copied(), &options);
});
