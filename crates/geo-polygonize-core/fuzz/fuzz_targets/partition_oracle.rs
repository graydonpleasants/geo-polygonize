#![no_main]

use arbitrary::Arbitrary;
use geo::{Coord, Geometry, LineString, MultiLineString, Rect};
use geo_polygonize_core::{
    Coord3D, Line3D, PolygonizerOptions, PrecisionModel, SnapStrategy, TiledPolygonizer,
};
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    lines: Vec<(i8, i8, i8, i8)>,
    tile_size: u8,
    buffer: u8,
    order: u8,
    reverse_input: bool,
    group_lines: bool,
    geometry_grouping: u8,
    fixed_grid: bool,
    grid_size: u8,
}

fn world() -> Rect<f64> {
    Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 16.0, y: 16.0 })
}

fn bounded_coordinate(value: i8) -> f64 {
    f64::from(value.clamp(0, 16))
}

fn geometries(lines: &[Line3D], input: &FuzzInput) -> Vec<Geometry<f64>> {
    let parts = lines
        .iter()
        .map(|line| {
            LineString::new(vec![
                line.start.to_coord_2d(),
                line.end.to_coord_2d(),
            ])
        })
        .collect::<Vec<_>>();
    if !input.group_lines {
        return parts.into_iter().map(Geometry::LineString).collect();
    }

    let group_count = (1 + usize::from(input.geometry_grouping % 4)).min(parts.len().max(1));
    let mut groups = vec![Vec::new(); group_count];
    for (index, part) in parts.into_iter().enumerate() {
        groups[index % group_count].push(part);
    }
    groups
        .into_iter()
        .filter(|parts| !parts.is_empty())
        .map(|parts| Geometry::MultiLineString(MultiLineString::new(parts)))
        .collect()
}

fuzz_target!(|input: FuzzInput| {
    if input.lines.len() > 16 {
        return;
    }

    let mut lines = input
        .lines
        .iter()
        .copied()
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
    if !lines.is_empty() {
        let rotation = usize::from(input.order) % lines.len();
        lines.rotate_left(rotation);
    }

    let options = PolygonizerOptions {
        node_input: true,
        precision_model: if input.fixed_grid {
            PrecisionModel::FixedGrid {
                grid_size: 0.25 + f64::from(input.grid_size % 4) * 0.25,
            }
        } else {
            PrecisionModel::Floating
        },
        snap_strategy: SnapStrategy::Grid,
        ..Default::default()
    };
    let geometries = geometries(&lines, &input);
    let mut tiled = TiledPolygonizer::new(
        world(),
        2.0 + f64::from(input.tile_size % 7),
    )
    .with_buffer(f64::from(input.buffer % 3))
    .with_options(options);
    for geometry in &geometries {
        tiled.add_geometry(geometry);
    }

    let Ok(Some(difference)) = tiled.partition_oracle_first_difference() else {
        return;
    };
    panic!(
        "partition oracle mismatch: partition={} stage={}",
        difference.partition_id, difference.stage
    );
});
