#![no_main]

use arbitrary::Arbitrary;
use geo::{Coord, Geometry, LineString, MultiLineString, Rect};
use geo_polygonize_core::{
    polygonize, Coord3D, DedupPolicy, Line3D, Polygon3D, PolygonizerOptions,
    TileOwnershipPolicy, TiledPolygonizer,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    lines: Vec<(i8, i8, i8, i8)>,
    tile_size: u8,
    buffer: u8,
    reverse_input: bool,
    group_lines: bool,
}

fn world() -> Rect<f64> {
    Rect::new(
        Coord { x: -32.0, y: -32.0 },
        Coord { x: 32.0, y: 32.0 },
    )
}

fn bounded_coordinate(value: i8) -> f64 {
    f64::from(value.clamp(-24, 24))
}

fn same_polygons(left: &[Polygon3D], right: &[Polygon3D]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.exterior == right.exterior && left.interiors == right.interiors
        })
}

fn has_coverage_evidence(result: &geo_polygonize_core::TiledPolygonizeResult) -> bool {
    result.tile_reports.iter().any(|report| {
        !report.coverage_issues.is_empty()
            || !report.input_boundary_issues.is_empty()
            || !report.excluded_component_issues.is_empty()
    })
}

fuzz_target!(|input: FuzzInput| {
    if input.lines.len() > 32 {
        return;
    }

    let mut lines = input
        .lines
        .into_iter()
        .enumerate()
        .map(|(line_id, (sx, sy, ex, ey))| {
            Line3D::new(
                Coord3D::new(bounded_coordinate(sx), bounded_coordinate(sy), 0.0),
                Coord3D::new(bounded_coordinate(ex), bounded_coordinate(ey), 0.0),
                u32::try_from(line_id).unwrap(),
            )
        })
        .collect::<Vec<_>>();

    if input.reverse_input {
        lines.reverse();
    }
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let Ok(expected) = polygonize(lines.iter().copied(), &options) else {
        return;
    };

    let parts = lines
        .iter()
        .map(|line| {
            LineString::new(vec![
                line.start.to_coord_2d(),
                line.end.to_coord_2d(),
            ])
        })
        .collect::<Vec<_>>();
    let geometries = if input.group_lines {
        vec![Geometry::MultiLineString(MultiLineString::new(parts))]
    } else {
        parts.into_iter().map(Geometry::LineString).collect()
    };
    let mut tiled = TiledPolygonizer::new(world(), 1.0 + f64::from(input.tile_size % 16))
        .with_buffer(f64::from(input.buffer % 17))
        .with_options(options)
        .with_ownership_policy(TileOwnershipPolicy::RepresentativePointInsidePolygon)
        .with_dedup_policy(DedupPolicy::CanonicalRingHash);
    for geometry in &geometries {
        tiled.add_geometry(geometry);
    }

    let Ok(actual) = tiled.polygonize() else {
        return;
    };
    if same_polygons(&expected.polygons, &actual.polygons) || has_coverage_evidence(&actual) {
        return;
    }

    panic!(
        "undetected tiled mismatch: lines={}, tile_size={}, buffer={}, untiled_polygons={}, tiled_polygons={}",
        lines.len(),
        1 + (input.tile_size % 16),
        input.buffer % 17,
        expected.polygons.len(),
        actual.polygons.len(),
    );
});
