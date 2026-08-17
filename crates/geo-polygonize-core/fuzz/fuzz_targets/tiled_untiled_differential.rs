#![no_main]

use arbitrary::Arbitrary;
use geo::{Coord, Geometry, GeometryCollection, LineString, MultiLineString, Rect};
use geo_polygonize_core::{
    polygonize, Coord3D, Line3D, Polygon3D, PolygonizerOptions, ProvenanceOptions, PrecisionModel,
    SnapStrategy, TiledPolygonizer, ZOptions, ZPolicy,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    lines: Vec<(i8, i8, i8, i8)>,
    origin_x: i8,
    origin_y: i8,
    tile_size: u8,
    buffer: u8,
    order: u8,
    reverse_input: bool,
    group_lines: bool,
    geometry_grouping: u8,
    nested_geometry_collection: bool,
    fixed_grid: bool,
    geos_compat: bool,
    grid_size: u8,
    provenance: bool,
    profile_id: bool,
    z_policy: u8,
}

fn bounded_coordinate(value: i8, origin: i8) -> f64 {
    f64::from(origin) + f64::from(value.clamp(0, 32))
}

fn input_bbox(input: &FuzzInput) -> Rect<f64> {
    Rect::new(
        Coord {
            x: f64::from(input.origin_x),
            y: f64::from(input.origin_y),
        },
        Coord {
            x: f64::from(input.origin_x) + 32.0,
            y: f64::from(input.origin_y) + 32.0,
        },
    )
}

fn same_provenance(left: &Polygon3D, right: &Polygon3D) -> bool {
    left.provenance.as_ref().map(|value| &value.input_profile_id)
        == right.provenance.as_ref().map(|value| &value.input_profile_id)
}

fn same_polygons(left: &[Polygon3D], right: &[Polygon3D]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            left.exterior == right.exterior
                && left.interiors == right.interiors
                && same_provenance(left, right)
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

fn z_policy(value: u8) -> ZPolicy {
    match value % 4 {
        0 => ZPolicy::Ignore,
        1 => ZPolicy::InterpolateAlongEdge,
        2 => ZPolicy::PreferNearestEndpoint,
        _ => ZPolicy::ErrorOnConflict,
    }
}

fn options(input: &FuzzInput) -> PolygonizerOptions {
    PolygonizerOptions {
        node_input: true,
        precision_model: if input.fixed_grid {
            PrecisionModel::FixedGrid {
                grid_size: 0.25 + f64::from(input.grid_size % 8) * 0.25,
            }
        } else {
            PrecisionModel::Floating
        },
        snap_strategy: if input.fixed_grid && input.geos_compat {
            SnapStrategy::GeosCompat
        } else {
            SnapStrategy::Grid
        },
        provenance: ProvenanceOptions {
            enabled: input.provenance,
            include_boundary_line_ids: input.provenance,
        },
        z: ZOptions {
            policy: z_policy(input.z_policy),
            conflict_tolerance: 0.0,
        },
        input_profile_id: input
            .profile_id
            .then(|| "tiled_untiled_differential".to_string()),
        ..Default::default()
    }
}

fn geometries(lines: &[Line3D], input: &FuzzInput) -> Vec<Geometry<f64>> {
    let parts = lines
        .iter()
        .map(|line| {
            LineString::new(vec![line.start.to_coord_2d(), line.end.to_coord_2d()])
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
    if input.nested_geometry_collection {
        vec![Geometry::GeometryCollection(GeometryCollection::new_from(
            geometries,
        ))]
    } else {
        geometries
    }
}

fuzz_target!(|input: FuzzInput| {
    if input.lines.len() > 24 {
        return;
    }

    let mut lines = input
        .lines
        .iter()
        .copied()
        .enumerate()
        .map(|(line_id, (sx, sy, ex, ey))| {
            Line3D::new(
                Coord3D::new(
                    bounded_coordinate(sx, input.origin_x),
                    bounded_coordinate(sy, input.origin_y),
                    0.0,
                ),
                Coord3D::new(
                    bounded_coordinate(ex, input.origin_x),
                    bounded_coordinate(ey, input.origin_y),
                    0.0,
                ),
                u32::try_from(line_id).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    if input.reverse_input {
        lines.reverse();
    }
    if !lines.is_empty() {
        let rotation = usize::from(input.order) % lines.len();
        lines.rotate_left(rotation);
    }

    let options = options(&input);
    let Ok(expected) = polygonize(lines.iter().copied(), &options) else {
        return;
    };
    let geometries = geometries(&lines, &input);
    let mut tiled = TiledPolygonizer::new(
        input_bbox(&input),
        2.0 + f64::from(input.tile_size % 15),
    )
    .with_buffer(f64::from(input.buffer % 9))
    .with_options(options)
    .with_untiled_equivalence_check();
    for geometry in &geometries {
        tiled.add_geometry(geometry);
    }
    let Ok(actual) = tiled.polygonize() else {
        return;
    };

    if actual
        .stitching_report
        .partition_border_global_untiled_equivalence_checked
        && !actual
            .stitching_report
            .partition_border_global_untiled_equivalence_ready
    {
        panic!(
            "validated stitched output diverged from untiled output: mismatches={}",
            actual
                .stitching_report
                .partition_border_global_untiled_equivalence_mismatch_count
        );
    }
    if same_polygons(&expected.polygons, &actual.polygons) || has_coverage_evidence(&actual) {
        return;
    }

    panic!(
        "undetected tiled mismatch: origin=({},{}) tile_size={} buffer={} order={} fixed_grid={} provenance={} z_policy={} untiled_polygons={} tiled_polygons={}",
        input.origin_x,
        input.origin_y,
        2 + (input.tile_size % 15),
        input.buffer % 9,
        input.order,
        input.fixed_grid,
        input.provenance,
        input.z_policy % 4,
        expected.polygons.len(),
        actual.polygons.len(),
    );
});
