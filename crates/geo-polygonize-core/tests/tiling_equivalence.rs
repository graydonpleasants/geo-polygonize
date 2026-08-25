use geo_polygonize_core::{
    polygonize, Coord3D, DedupPolicy, Line3D, Polygon3D, PolygonizerOptions, TileCoverageGuarantee,
    TileOwnershipPolicy, TileRetryPolicy, TiledPolygonizer,
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

#[test]
fn untiled_fallback_matches_global_output_across_tile_and_input_permutations() {
    let mut lines = ring(&[(-10.0, -10.0), (30.0, -10.0), (30.0, 30.0), (-10.0, 30.0)]);
    lines.extend(ring(&[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)]));
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let expected = polygonize(lines.iter().copied(), &options).unwrap();
    let geometries = lines
        .iter()
        .map(|line| {
            Geometry::LineString(LineString::new(vec![
                line.start.to_coord_2d(),
                line.end.to_coord_2d(),
            ]))
        })
        .collect::<Vec<_>>();

    for tile_size in [5.0, 10.0, 20.0] {
        for rotation in 0..geometries.len() {
            let mut permuted = geometries.clone();
            permuted.rotate_left(rotation);
            let mut tiler = TiledPolygonizer::new(world(20.0), tile_size)
                .with_buffer(2.0)
                .with_options(options.clone())
                .with_retry_policy(TileRetryPolicy {
                    max_attempts: 1,
                    buffer_increment: 1.0,
                    max_buffer: 3.0,
                })
                .with_untiled_fallback();
            for geometry in &permuted {
                tiler.add_geometry(geometry);
            }

            let actual = tiler
                .polygonize_with_coverage_guarantee(TileCoverageGuarantee::ValidateObservedCoverage)
                .unwrap();
            assert!(actual.stitching_report.untiled_fallback_used);
            assert_eq!(actual.polygons.len(), expected.polygons.len());
            for (actual, expected) in actual.polygons.iter().zip(&expected.polygons) {
                assert_eq!(actual.exterior, expected.exterior);
                assert_eq!(actual.interiors, expected.interiors);
            }
        }
    }
}

#[test]
fn in_domain_tiled_mismatches_have_observed_coverage_evidence() {
    let mut lines = ring(&[(2.0, 2.0), (18.0, 2.0), (18.0, 18.0), (2.0, 18.0)]);
    lines.extend(ring(&[(6.0, 6.0), (14.0, 6.0), (14.0, 14.0), (6.0, 14.0)]));
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let expected = polygonize(lines.iter().copied(), &options).unwrap();
    let geometries = lines
        .iter()
        .map(|line| {
            Geometry::LineString(LineString::new(vec![
                line.start.to_coord_2d(),
                line.end.to_coord_2d(),
            ]))
        })
        .collect::<Vec<_>>();

    for tile_size in [4.0, 7.0, 10.0] {
        for buffer in [0.0, 1.0, 4.0] {
            let mut tiled = TiledPolygonizer::new(world(20.0), tile_size)
                .with_buffer(buffer)
                .with_options(options.clone())
                .with_ownership_policy(TileOwnershipPolicy::RepresentativePointInsidePolygon)
                .with_dedup_policy(DedupPolicy::CanonicalRingHash);
            for geometry in &geometries {
                tiled.add_geometry(geometry);
            }
            let actual = tiled
                .polygonize()
                .unwrap_or_else(|error| panic!("tile size {tile_size}, buffer {buffer}: {error}"));
            let equivalent = actual.polygons.len() == expected.polygons.len()
                && actual
                    .polygons
                    .iter()
                    .zip(&expected.polygons)
                    .all(|(actual, expected)| {
                        actual.exterior == expected.exterior
                            && actual.interiors == expected.interiors
                    });
            if !equivalent {
                assert!(
                    actual.tile_reports.iter().any(|report| {
                        !report.coverage_issues.is_empty()
                            || !report.input_boundary_issues.is_empty()
                            || !report.excluded_component_issues.is_empty()
                    }),
                    "undetected in-domain mismatch for tile size {tile_size}, buffer {buffer}"
                );
            }
        }
    }
}

#[test]
#[ignore = "exhaustive tiling sweep; exercised by Release Tests"]
fn bounded_single_geometry_envelope_sweep_keeps_in_domain_mismatches_observed() {
    let bounds = world(32.0);
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let mut checked_cases = 0;

    for min_x in [-16.0, -8.0, 0.0, 8.0, 16.0, 24.0, 32.0] {
        for min_y in [-16.0, -8.0, 0.0, 8.0, 16.0, 24.0, 32.0] {
            for width in [4.0, 8.0, 16.0, 24.0, 40.0] {
                let max_x = min_x + width;
                let max_y = min_y + width;
                let lines = ring(&[
                    (min_x, min_y),
                    (max_x, min_y),
                    (max_x, max_y),
                    (min_x, max_y),
                ]);
                let expected = polygonize(lines.iter().copied(), &options).unwrap();
                if expected.polygons.is_empty()
                    || expected
                        .polygons
                        .iter()
                        .any(|polygon| !polygon_overlaps(&bounds, polygon))
                {
                    continue;
                }
                checked_cases += 1;
                let geometry = Geometry::LineString(LineString::new(
                    lines
                        .iter()
                        .map(|line| line.start.to_coord_2d())
                        .chain(std::iter::once(lines[0].start.to_coord_2d()))
                        .collect(),
                ));

                for ownership_policy in [
                    TileOwnershipPolicy::Centroid,
                    TileOwnershipPolicy::RepresentativePointInsidePolygon,
                    TileOwnershipPolicy::LexicographicMinVertex,
                ]
                .iter()
                {
                    for tile_size in [4.0, 8.0, 16.0] {
                        for buffer in [0.0, 1.0, 4.0] {
                            let mut tiled = TiledPolygonizer::new(bounds, tile_size)
                                .with_buffer(buffer)
                                .with_options(options.clone())
                                .with_ownership_policy(ownership_policy.clone())
                                .with_dedup_policy(DedupPolicy::CanonicalRingHash);
                            tiled.add_geometry(&geometry);
                            let actual = tiled.polygonize().unwrap();
                            let equivalent = actual.polygons.len() == expected.polygons.len()
                                && actual.polygons.iter().zip(&expected.polygons).all(
                                    |(actual, expected)| {
                                        actual.exterior == expected.exterior
                                            && actual.interiors == expected.interiors
                                    },
                                );
                            if !equivalent {
                                assert!(
                                    actual.tile_reports.iter().any(|report| {
                                        !report.coverage_issues.is_empty()
                                            || !report.ownership_domain_issues.is_empty()
                                            || !report.input_boundary_issues.is_empty()
                                            || !report.excluded_component_issues.is_empty()
                                    }),
                                    "undetected single-geometry mismatch: policy={ownership_policy:?}, min=({min_x},{min_y}), width={width}, tile_size={tile_size}, buffer={buffer}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    assert_eq!(checked_cases, 208);
}

#[test]
#[ignore = "exhaustive tiling sweep; exercised by Release Tests"]
fn adversarial_in_domain_mismatches_keep_coverage_evidence_under_permutations() {
    let mut concave = ring(&[
        (3.0, 3.0),
        (21.0, 3.0),
        (21.0, 21.0),
        (19.0, 21.0),
        (19.0, 6.0),
        (5.0, 6.0),
        (5.0, 21.0),
        (3.0, 21.0),
    ]);
    concave.push(line((1.0, 12.0), (23.0, 12.0)));

    let mut boundary_crossing_hole = ring(&[(1.0, 1.0), (23.0, 1.0), (23.0, 23.0), (1.0, 23.0)]);
    boundary_crossing_hole.extend(ring(&[(6.0, 6.0), (18.0, 6.0), (18.0, 18.0), (6.0, 18.0)]));

    let mut dirty_overlap = ring(&[(3.0, 3.0), (21.0, 3.0), (21.0, 21.0), (3.0, 21.0)]);
    dirty_overlap.extend([
        line((1.0, 12.0), (23.0, 12.0)),
        line((12.0, 1.0), (12.0, 23.0)),
    ]);
    dirty_overlap.extend(ring(&[
        (12.0, 8.0),
        (23.0, 8.0),
        (23.0, 20.0),
        (12.0, 20.0),
    ]));

    let cases = [
        (
            "shifted concavity",
            concave,
            Rect::new(Coord { x: -2.0, y: -1.0 }, Coord { x: 25.0, y: 24.0 }),
        ),
        (
            "boundary-crossing hole",
            boundary_crossing_hole,
            Rect::new(Coord { x: -1.0, y: -2.0 }, Coord { x: 25.0, y: 26.0 }),
        ),
        ("dirty overlap", dirty_overlap, world(24.0)),
    ];
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };

    for (case_index, (name, lines, bbox)) in cases.into_iter().enumerate() {
        let expected = polygonize(lines.iter().copied(), &options)
            .unwrap_or_else(|error| panic!("{name} untiled: {error}"));
        for nested in [false, true] {
            for grouping in 0..=3 {
                let base_geometries = geometries_for_grouping(&lines, grouping, nested);
                for (tile_index, tile_size) in [5.0, 7.0, 10.0].into_iter().enumerate() {
                    for buffer in [0.0, 1.0, 3.0] {
                        for permutation in 0..2 {
                            let mut geometries = base_geometries.clone();
                            let rotation =
                                (case_index + tile_index + permutation) % geometries.len();
                            geometries.rotate_left(rotation);
                            if permutation == 1 {
                                geometries.reverse();
                            }

                            let mut tiled = TiledPolygonizer::new(bbox, tile_size)
                                .with_buffer(buffer)
                                .with_options(options.clone())
                                .with_ownership_policy(
                                    TileOwnershipPolicy::RepresentativePointInsidePolygon,
                                )
                                .with_dedup_policy(DedupPolicy::CanonicalRingHash);
                            for geometry in &geometries {
                                tiled.add_geometry(geometry);
                            }
                            let actual = tiled.polygonize().unwrap_or_else(|error| {
                                panic!("{name}, nested {nested}, grouping {grouping}, tile size {tile_size}, buffer {buffer}: {error}")
                            });
                            let equivalent = actual.polygons.len() == expected.polygons.len()
                                && actual.polygons.iter().zip(&expected.polygons).all(
                                    |(actual, expected)| {
                                        actual.exterior == expected.exterior
                                            && actual.interiors == expected.interiors
                                    },
                                );
                            if !equivalent {
                                assert!(
                                    actual.tile_reports.iter().any(|report| {
                                        !report.coverage_issues.is_empty()
                                            || !report.ownership_domain_issues.is_empty()
                                            || !report.input_boundary_issues.is_empty()
                                            || !report.excluded_component_issues.is_empty()
                                    }),
                                    "undetected {name} mismatch for nested {nested}, grouping {grouping}, tile size {tile_size}, buffer {buffer}, permutation {permutation}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn tiled_report_matches_untiled_dangle_and_cut_edge_families() {
    let mut lines = ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
    lines.push(line((10.0, 10.0), (15.0, 15.0)));
    lines.extend(ring(&[
        (20.0, 0.0),
        (30.0, 0.0),
        (30.0, 10.0),
        (20.0, 10.0),
    ]));
    lines.extend(ring(&[
        (40.0, 0.0),
        (50.0, 0.0),
        (50.0, 10.0),
        (40.0, 10.0),
    ]));
    lines.push(line((30.0, 5.0), (40.0, 5.0)));
    let options = PolygonizerOptions {
        node_input: true,
        ..Default::default()
    };
    let expected = polygonize(lines.iter().copied(), &options).unwrap();
    assert_eq!(expected.dangles.len(), 1);
    assert_eq!(expected.cut_edges.len(), 1);
    let geometries = lines
        .iter()
        .map(|line| {
            Geometry::LineString(LineString::new(vec![
                line.start.to_coord_2d(),
                line.end.to_coord_2d(),
            ]))
        })
        .collect::<Vec<_>>();
    let bbox = Rect::new(Coord { x: -1.0, y: -1.0 }, Coord { x: 51.0, y: 16.0 });
    let mut tiler = TiledPolygonizer::new(bbox, 60.0).with_options(options);
    for geometry in &geometries {
        tiler.add_geometry(geometry);
    }

    let actual = tiler.polygonize().unwrap();
    assert_eq!(actual.tile_reports.len(), 1);
    assert_eq!(actual.tile_reports[0].dangle_count, expected.dangles.len());
    assert_eq!(
        actual.tile_reports[0].cut_edge_count,
        expected.cut_edges.len()
    );
    assert_eq!(actual.polygons.len(), expected.polygons.len());
    for (actual, expected) in actual.polygons.iter().zip(&expected.polygons) {
        assert_eq!(actual.exterior, expected.exterior);
        assert_eq!(actual.interiors, expected.interiors);
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

fn polygon_overlaps(bounds: &Rect<f64>, polygon: &Polygon3D) -> bool {
    let Some(first) = polygon.exterior.first() else {
        return false;
    };
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (first.x, first.y, first.x, first.y);
    for coordinate in &polygon.exterior[1..] {
        min_x = min_x.min(coordinate.x);
        min_y = min_y.min(coordinate.y);
        max_x = max_x.max(coordinate.x);
        max_y = max_y.max(coordinate.y);
    }
    min_x <= bounds.max().x
        && max_x >= bounds.min().x
        && min_y <= bounds.max().y
        && max_y >= bounds.min().y
}

fn geometries_for_grouping(lines: &[Line3D], grouping: usize, nested: bool) -> Vec<Geometry<f64>> {
    let parts = lines
        .iter()
        .map(|line| LineString::new(vec![line.start.to_coord_2d(), line.end.to_coord_2d()]))
        .collect::<Vec<_>>();
    let geometries = if grouping == 0 {
        parts.into_iter().map(Geometry::LineString).collect()
    } else {
        let group_count = grouping.min(parts.len()).max(1);
        let mut groups = vec![Vec::new(); group_count];
        for (part_index, part) in parts.into_iter().enumerate() {
            groups[part_index % group_count].push(part);
        }
        groups
            .into_iter()
            .filter(|parts| !parts.is_empty())
            .map(|parts| Geometry::MultiLineString(geo_types::MultiLineString::new(parts)))
            .collect()
    };
    if nested {
        vec![Geometry::GeometryCollection(
            geo_types::GeometryCollection::new_from(geometries),
        )]
    } else {
        geometries
    }
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
