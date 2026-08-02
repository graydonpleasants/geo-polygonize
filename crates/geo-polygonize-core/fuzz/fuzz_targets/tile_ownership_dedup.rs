#![no_main]

use arbitrary::Arbitrary;
use geo::{Coord, Geometry, GeometryCollection, LineString, MultiLineString, Rect};
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
    geometry_grouping: u8,
    nested_geometry_collection: bool,
    ownership_domain_gap: bool,
}

fn world() -> Rect<f64> {
    Rect::new(
        Coord { x: -32.0, y: -32.0 },
        Coord { x: 32.0, y: 32.0 },
    )
}

fn bounded_coordinate(value: i8, x_offset: f64) -> f64 {
    f64::from(value.clamp(-24, 24)) + x_offset
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
            || !report.ownership_domain_issues.is_empty()
    })
}

fn polygon_overlaps_world(polygon: &Polygon3D) -> bool {
    let Some(first) = polygon.exterior.first() else {
        return false;
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) =
        (first.x, first.y, first.x, first.y);
    for coordinate in &polygon.exterior[1..] {
        min_x = min_x.min(coordinate.x);
        min_y = min_y.min(coordinate.y);
        max_x = max_x.max(coordinate.x);
        max_y = max_y.max(coordinate.y);
    }
    let bounds = world();
    min_x <= bounds.max().x
        && max_x >= bounds.min().x
        && min_y <= bounds.max().y
        && max_y >= bounds.min().y
}

fuzz_target!(|input: FuzzInput| {
    if input.lines.len() > 32 {
        return;
    }

    let x_offset = if input.ownership_domain_gap { 24.0 } else { 0.0 };
    let mut lines = input
        .lines
        .into_iter()
        .enumerate()
        .map(|(line_id, (sx, sy, ex, ey))| {
            Line3D::new(
                Coord3D::new(
                    bounded_coordinate(sx, x_offset),
                    bounded_coordinate(sy, 0.0),
                    0.0,
                ),
                Coord3D::new(
                    bounded_coordinate(ex, x_offset),
                    bounded_coordinate(ey, 0.0),
                    0.0,
                ),
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
    if input.ownership_domain_gap
        && !expected.polygons.is_empty()
        && expected
            .polygons
            .iter()
            .any(|polygon| !polygon_overlaps_world(polygon))
    {
        return;
    }

    let parts = lines
        .iter()
        .map(|line| {
            LineString::new(vec![
                line.start.to_coord_2d(),
                line.end.to_coord_2d(),
            ])
        })
        .collect::<Vec<_>>();
    let geometries: Vec<Geometry<f64>> = if input.group_lines {
        let group_count = (1 + usize::from(input.geometry_grouping % 8)).min(parts.len().max(1));
        let mut groups = vec![Vec::new(); group_count];
        for (part_index, part) in parts.into_iter().enumerate() {
            groups[part_index % group_count].push(part);
        }
        groups
            .into_iter()
            .filter(|parts| !parts.is_empty())
            .map(|parts| Geometry::MultiLineString(MultiLineString::new(parts)))
            .collect()
    } else {
        parts.into_iter().map(Geometry::LineString).collect()
    };
    let geometries = if input.nested_geometry_collection {
        vec![Geometry::GeometryCollection(GeometryCollection::new_from(geometries))]
    } else {
        geometries
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
        "undetected tiled mismatch: lines={}, tile_size={}, buffer={}, nested_collection={}, untiled_polygons={}, tiled_polygons={}",
        lines.len(),
        1 + (input.tile_size % 16),
        input.buffer % 17,
        input.nested_geometry_collection,
        expected.polygons.len(),
        actual.polygons.len(),
    );
});
