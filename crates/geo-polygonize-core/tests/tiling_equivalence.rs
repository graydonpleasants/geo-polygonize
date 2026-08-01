use geo_polygonize_core::{
    polygonize, Coord3D, DedupPolicy, Line3D, PolygonizerOptions, TileCoverageGuarantee,
    TileOwnershipPolicy, TiledPolygonizer,
};
use geo_types::{Coord, Geometry, LineString, Rect};

struct Case {
    name: &'static str,
    lines: Vec<Line3D>,
    bbox: Rect<f64>,
    tile_size: f64,
    buffer: f64,
}

#[test]
fn sufficient_halos_survive_deterministic_grid_and_input_permutations() {
    let mut lines = ring(&[(2.0, 2.0), (18.0, 2.0), (18.0, 18.0), (2.0, 18.0)]);
    lines.extend(ring(&[(7.0, 7.0), (13.0, 7.0), (13.0, 13.0), (7.0, 13.0)]));
    lines.extend(ring(&[(20.0, 3.0), (23.0, 3.0), (23.0, 6.0), (20.0, 6.0)]));

    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let expected = polygonize(lines.iter().copied(), &options).unwrap();

    let mut state = 0x5eed_u64;
    for case_index in 0..24 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let offset = (state % 5) as f64;
        let tile_size = 5.0 + ((state >> 8) % 10) as f64;
        let bbox = Rect::new(
            Coord {
                x: -offset,
                y: -(4.0 - offset),
            },
            Coord {
                x: 28.0 - offset,
                y: 24.0 + offset,
            },
        );
        let mut geometries: Vec<_> = lines
            .iter()
            .map(|line| {
                Geometry::LineString(LineString::new(vec![
                    line.start.to_coord_2d(),
                    line.end.to_coord_2d(),
                ]))
            })
            .collect();
        let geometry_count = geometries.len();
        geometries.rotate_left((state as usize) % geometry_count);
        if state & 1 != 0 {
            geometries.reverse();
        }

        let mut tiler = TiledPolygonizer::new(bbox, tile_size)
            .with_buffer(40.0)
            .with_options(options.clone())
            .with_ownership_policy(TileOwnershipPolicy::RepresentativePointInsidePolygon)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash);
        for geometry in &geometries {
            tiler.add_geometry(geometry);
        }
        let actual = tiler
            .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateOwnedFaces)
            .unwrap_or_else(|error| panic!("permutation {case_index}: {error}"));

        assert_eq!(actual.polygons.len(), expected.polygons.len());
        for (actual, expected) in actual.polygons.iter().zip(&expected.polygons) {
            assert_eq!(actual.exterior, expected.exterior, "case {case_index}");
            assert_eq!(actual.interiors, expected.interiors, "case {case_index}");
        }
    }
}

fn ring(points: &[(f64, f64)]) -> Vec<Line3D> {
    (0..points.len())
        .map(|index| {
            let start = points[index];
            let end = points[(index + 1) % points.len()];
            Line3D::new(
                Coord3D::new(start.0, start.1, 0.0),
                Coord3D::new(end.0, end.1, 0.0),
                0,
            )
        })
        .collect()
}

fn line(start: (f64, f64), end: (f64, f64)) -> Line3D {
    Line3D::new(
        Coord3D::new(start.0, start.1, 0.0),
        Coord3D::new(end.0, end.1, 0.0),
        0,
    )
}

fn world(size: f64) -> Rect<f64> {
    Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: size, y: size })
}

#[test]
fn tiled_output_matches_untiled_when_the_halo_contains_each_owned_face() {
    let mut hole_case = ring(&[(2.0, 2.0), (18.0, 2.0), (18.0, 18.0), (2.0, 18.0)]);
    hole_case.extend(ring(&[(8.0, 8.0), (12.0, 8.0), (12.0, 12.0), (8.0, 12.0)]));

    let mut dirty_cross = ring(&[(2.0, 2.0), (18.0, 2.0), (18.0, 18.0), (2.0, 18.0)]);
    dirty_cross.extend([
        line((0.0, 10.0), (20.0, 10.0)),
        line((10.0, 0.0), (10.0, 20.0)),
    ]);

    let mut disconnected_nested = ring(&[(2.0, 2.0), (28.0, 2.0), (28.0, 28.0), (2.0, 28.0)]);
    disconnected_nested.extend(ring(&[(6.0, 6.0), (24.0, 6.0), (24.0, 24.0), (6.0, 24.0)]));
    disconnected_nested.extend(ring(&[
        (10.0, 10.0),
        (20.0, 10.0),
        (20.0, 20.0),
        (10.0, 20.0),
    ]));

    let mut overlaps = ring(&[(2.0, 3.0), (17.0, 3.0), (17.0, 18.0), (2.0, 18.0)]);
    overlaps.extend(ring(&[
        (11.0, 9.0),
        (26.0, 9.0),
        (26.0, 24.0),
        (11.0, 24.0),
    ]));

    let cases = [
        Case {
            name: "one boundary",
            lines: ring(&[(8.0, 2.0), (12.0, 2.0), (12.0, 6.0), (8.0, 6.0)]),
            bbox: world(20.0),
            tile_size: 10.0,
            buffer: 4.0,
        },
        Case {
            name: "two boundaries",
            lines: ring(&[(8.0, 2.0), (22.0, 2.0), (22.0, 6.0), (8.0, 6.0)]),
            bbox: world(30.0),
            tile_size: 10.0,
            buffer: 12.0,
        },
        Case {
            name: "four boundaries",
            lines: ring(&[(8.0, 8.0), (22.0, 8.0), (22.0, 22.0), (8.0, 22.0)]),
            bbox: world(30.0),
            tile_size: 10.0,
            buffer: 12.0,
        },
        Case {
            name: "many boundaries",
            lines: ring(&[(1.0, 8.0), (39.0, 8.0), (39.0, 32.0), (1.0, 32.0)]),
            bbox: world(40.0),
            tile_size: 10.0,
            buffer: 20.0,
        },
        Case {
            name: "narrow concave",
            lines: ring(&[
                (4.0, 4.0),
                (16.0, 4.0),
                (16.0, 16.0),
                (15.0, 16.0),
                (15.0, 5.0),
                (5.0, 5.0),
                (5.0, 16.0),
                (4.0, 16.0),
            ]),
            bbox: world(20.0),
            tile_size: 10.0,
            buffer: 20.0,
        },
        Case {
            name: "hole crossing boundaries",
            lines: hole_case,
            bbox: world(20.0),
            tile_size: 10.0,
            buffer: 20.0,
        },
        Case {
            name: "dirty crossings and exterior dangles",
            lines: dirty_cross,
            bbox: world(20.0),
            tile_size: 10.0,
            buffer: 20.0,
        },
        Case {
            name: "disconnected nested rings",
            lines: disconnected_nested,
            bbox: world(30.0),
            tile_size: 10.0,
            buffer: 30.0,
        },
        Case {
            name: "overlapping boundaries",
            lines: overlaps,
            bbox: world(30.0),
            tile_size: 10.0,
            buffer: 30.0,
        },
    ];

    for case in cases {
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        let expected = polygonize(case.lines.iter().copied(), &options)
            .unwrap_or_else(|error| panic!("{} untiled: {error}", case.name));
        let geometries: Vec<_> = case
            .lines
            .iter()
            .map(|line| {
                Geometry::LineString(LineString::new(vec![
                    line.start.to_coord_2d(),
                    line.end.to_coord_2d(),
                ]))
            })
            .collect();
        let mut tiler = TiledPolygonizer::new(case.bbox, case.tile_size)
            .with_buffer(case.buffer)
            .with_options(options)
            .with_ownership_policy(TileOwnershipPolicy::RepresentativePointInsidePolygon)
            .with_dedup_policy(DedupPolicy::CanonicalRingHash);
        for geometry in &geometries {
            tiler.add_geometry(geometry);
        }
        let actual = tiler
            .polygonize()
            .unwrap_or_else(|error| panic!("{} tiled: {error}", case.name));

        assert_eq!(
            actual.polygons.len(),
            expected.polygons.len(),
            "{} count",
            case.name
        );
        for (actual, expected) in actual.polygons.iter().zip(&expected.polygons) {
            assert_eq!(actual.exterior, expected.exterior, "{} exterior", case.name);
            assert_eq!(actual.interiors, expected.interiors, "{} holes", case.name);
        }
    }
}
