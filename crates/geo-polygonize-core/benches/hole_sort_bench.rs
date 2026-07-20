use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use geo::algorithm::indexed::IntervalTreeMultiPolygon;
use geo::{Contains, Coord, MultiPolygon};
use geo_polygonize_core::containment::ContainmentForest;
use geo_polygonize_core::options::{PolygonizerOptions, TouchPolicy};
use geo_polygonize_core::utils::simd::SimdRing;
use geo_polygonize_core::{Coord3D, Line3D, Polygon3D, Polygonizer};
use multiversion::multiversion;
use std::f64::consts::PI;
use wide::{f64x4, CmpGt};

fn circle_points(center_x: f64, center_y: f64, radius: f64, edges: usize) -> Vec<Coord3D> {
    let mut points = Vec::with_capacity(edges + 1);
    for i in 0..edges {
        let angle = 2.0 * PI * i as f64 / edges as f64;
        points.push(Coord3D::new(
            center_x + radius * angle.cos(),
            center_y + radius * angle.sin(),
            0.0,
        ));
    }
    points.push(points[0]);
    points
}

fn circle_polygon(center_x: f64, center_y: f64, radius: f64, edges: usize) -> Polygon3D {
    Polygon3D::new(
        circle_points(center_x, center_y, radius, edges),
        vec![],
        vec![],
        vec![],
    )
}

fn disjoint_shells(count: usize, edges: usize) -> Vec<Polygon3D> {
    (0..count)
        .map(|i| circle_polygon(i as f64 * 4.0, 0.0, 1.0, edges))
        .collect()
}

fn nested_shells(count: usize, edges: usize) -> Vec<Polygon3D> {
    (0..count)
        .map(|i| circle_polygon(0.0, 0.0, (count - i) as f64 * 2.0, edges))
        .collect()
}

fn overlapping_non_containing_shells(count: usize, edges: usize) -> Vec<Polygon3D> {
    (0..count)
        .map(|i| circle_polygon(i as f64 * 0.25, 0.0, 10.0, edges))
        .collect()
}

fn holes(count: usize, edges: usize) -> Vec<Polygon3D> {
    (0..count)
        .map(|i| {
            let mut hole = circle_polygon(
                (i % 32) as f64 * 2.0 - 31.0,
                (i / 32) as f64 * 2.0 - 31.0,
                0.75,
                edges,
            );
            hole.exterior.reverse();
            hole
        })
        .collect()
}

fn polygon_lines(polygon: &Polygon3D, line_id: u32) -> impl Iterator<Item = Line3D> + '_ {
    polygon
        .exterior
        .windows(2)
        .map(move |pair| Line3D::new(pair[0], pair[1], line_id))
}

fn locator_query_points(count: usize) -> Vec<Coord<f64>> {
    (0..count)
        .map(|i| {
            let angle = 2.0 * PI * i as f64 / count as f64;
            let radius = if i % 2 == 0 { 50.0 } else { 150.0 };
            Coord {
                x: radius * angle.cos(),
                y: radius * angle.sin(),
            }
        })
        .collect()
}

fn scalar_contains(x: &[f64], y: &[f64], len: usize, point: Coord<f64>) -> bool {
    let mut crossings = 0;
    for i in 0..len - 1 {
        if ((y[i] > point.y) != (y[i + 1] > point.y))
            && point.x < (x[i + 1] - x[i]) * (point.y - y[i]) / (y[i + 1] - y[i]) + x[i]
        {
            crossings += 1;
        }
    }
    crossings % 2 != 0
}

#[multiversion(targets(
    "x86_64+avx512f+avx512dq",
    "x86_64+avx2",
    "x86+avx2",
    "x86_64+avx",
    "x86+avx",
    "x86_64+sse2",
    "x86+sse2",
))]
fn wide_contains(x: &[f64], y: &[f64], len: usize, point: Coord<f64>) -> bool {
    let px = f64x4::splat(point.x);
    let py = f64x4::splat(point.y);
    let mut crossings = 0;
    let mut i = 0;
    let segments = len - 1;

    while i + 4 <= segments {
        let xi = f64x4::from(&x[i..i + 4]);
        let yi = f64x4::from(&y[i..i + 4]);
        let xj = f64x4::from(&x[i + 1..i + 5]);
        let yj = f64x4::from(&y[i + 1..i + 5]);
        let crossings_mask =
            (yi.cmp_gt(py) ^ yj.cmp_gt(py)) & (((xj - xi) * (py - yi) / (yj - yi)) + xi).cmp_gt(px);
        crossings += crossings_mask.move_mask().count_ones();
        i += 4;
    }

    while i < segments {
        if ((y[i] > point.y) != (y[i + 1] > point.y))
            && point.x < (x[i + 1] - x[i]) * (point.y - y[i]) / (y[i + 1] - y[i]) + x[i]
        {
            crossings += 1;
        }
        i += 1;
    }
    crossings % 2 != 0
}

fn bench_point_in_ring_crossover(c: &mut Criterion) {
    let points = locator_query_points(1_024);

    for query_count in [1, points.len()] {
        let mut group = c.benchmark_group(format!(
            "point_in_ring_crossover/{}",
            if query_count == 1 {
                "one_shot"
            } else {
                "repeated"
            }
        ));

        for edges in [32, 64, 96, 128, 192, 256, 384, 512, 1_024] {
            let polygon = circle_polygon(0.0, 0.0, 100.0, edges);
            let ring = SimdRing::new_3d(&polygon.exterior);
            let len = polygon.exterior.len();
            let queries = &points[..query_count];
            let scalar_count = queries
                .iter()
                .filter(|point| scalar_contains(&ring.x, &ring.y, len, **point))
                .count();
            assert_eq!(
                scalar_count,
                queries
                    .iter()
                    .filter(|point| wide_contains(&ring.x, &ring.y, len, **point))
                    .count()
            );

            group.throughput(Throughput::Elements((edges * query_count) as u64));
            group.bench_function(BenchmarkId::new("scalar", edges), |b| {
                b.iter(|| {
                    black_box(queries)
                        .iter()
                        .filter(|point| scalar_contains(&ring.x, &ring.y, len, **point))
                        .count()
                });
            });
            group.bench_function(BenchmarkId::new("wide", edges), |b| {
                b.iter(|| {
                    black_box(queries)
                        .iter()
                        .filter(|point| wide_contains(&ring.x, &ring.y, len, **point))
                        .count()
                });
            });
        }
        group.finish();
    }
}

fn bench_point_locators(c: &mut Criterion) {
    for edges in [16, 64, 256, 1_024, 16_384] {
        let ring = circle_polygon(0.0, 0.0, 100.0, edges);
        let polygon = ring.to_polygon_2d();
        let multipolygon = MultiPolygon(vec![polygon.clone()]);
        let simd = SimdRing::new_3d(&ring.exterior);
        let interval = IntervalTreeMultiPolygon::new(&multipolygon);
        let points = locator_query_points(1_024);

        let scalar_count = points
            .iter()
            .filter(|point| polygon.contains(*point))
            .count();
        assert_eq!(
            scalar_count,
            points.iter().filter(|point| simd.contains(**point)).count()
        );
        assert_eq!(
            scalar_count,
            points
                .iter()
                .filter(|point| interval.contains(*point))
                .count()
        );

        let mut group = c.benchmark_group("point_locator/prepare");
        group.throughput(Throughput::Elements(edges as u64));
        group.bench_function(BenchmarkId::new("simd", edges), |b| {
            b.iter(|| SimdRing::new_3d(black_box(&ring.exterior)));
        });
        group.bench_function(BenchmarkId::new("scalar", edges), |b| {
            b.iter(|| black_box(&ring).to_polygon_2d());
        });
        group.bench_function(BenchmarkId::new("interval", edges), |b| {
            b.iter(|| {
                let polygon = black_box(&ring).to_polygon_2d();
                IntervalTreeMultiPolygon::new(&MultiPolygon(vec![polygon]))
            });
        });
        group.finish();

        for query_count in [1, 16, 64, 1_024] {
            let points = &points[..query_count];
            let mut group =
                c.benchmark_group(format!("point_locator/amortized_{query_count}_queries"));
            group.throughput(Throughput::Elements(query_count as u64));
            group.bench_function(BenchmarkId::new("simd", edges), |b| {
                b.iter(|| {
                    let locator = SimdRing::new_3d(&ring.exterior);
                    black_box(points)
                        .iter()
                        .filter(|point| locator.contains(**point))
                        .count()
                });
            });
            group.bench_function(BenchmarkId::new("scalar", edges), |b| {
                b.iter(|| {
                    let locator = ring.to_polygon_2d();
                    black_box(points)
                        .iter()
                        .filter(|point| locator.contains(*point))
                        .count()
                });
            });
            group.bench_function(BenchmarkId::new("interval", edges), |b| {
                b.iter(|| {
                    let polygon = ring.to_polygon_2d();
                    let locator = IntervalTreeMultiPolygon::new(&MultiPolygon(vec![polygon]));
                    black_box(points)
                        .iter()
                        .filter(|point| locator.contains(*point))
                        .count()
                });
            });
            group.finish();
        }
    }
}

fn bench_preparation(c: &mut Criterion) {
    let mut group = c.benchmark_group("containment/prepare_ring_edges");
    for edges in [16, 64, 256, 1_024, 16_384] {
        let shells = vec![circle_polygon(0.0, 0.0, 100.0, edges)];
        group.throughput(Throughput::Elements(edges as u64));
        group.bench_with_input(BenchmarkId::from_parameter(edges), &shells, |b, shells| {
            b.iter(|| ContainmentForest::new(black_box(shells)));
        });
    }
    group.finish();

    let mut group = c.benchmark_group("containment/prepare_shell_count");
    for count in [1, 64, 256, 1_000] {
        let shells = disjoint_shells(count, 64);
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &shells, |b, shells| {
            b.iter(|| ContainmentForest::new(black_box(shells)));
        });
    }
    group.finish();
}

fn bench_filtering(c: &mut Criterion) {
    let mut group = c.benchmark_group("containment/filter_shells");
    for (name, shells) in [
        ("disjoint", disjoint_shells(256, 64)),
        ("nested", nested_shells(256, 64)),
        (
            "overlapping_non_containing",
            overlapping_non_containing_shells(256, 64),
        ),
    ] {
        let forest = ContainmentForest::new(&shells);
        group.throughput(Throughput::Elements(shells.len() as u64));
        group.bench_function(name, |b| {
            b.iter(|| {
                forest.filter_polygonal(black_box(&shells), black_box(&TouchPolicy::AllowEdgeShare))
            });
        });
    }
    group.finish();
}

fn bench_hole_assignment(c: &mut Criterion) {
    let shells = vec![circle_polygon(0.0, 0.0, 100.0, 1_024)];
    let forest = ContainmentForest::new(&shells);
    let mut group = c.benchmark_group("containment/assign_holes");
    for count in [1, 100, 1_000] {
        let holes = holes(count, 64);
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::from_parameter(count), &holes, |b, holes| {
            b.iter(|| {
                holes
                    .iter()
                    .filter_map(|hole| {
                        forest.assign_hole(
                            black_box(hole),
                            black_box(&shells),
                            black_box(&TouchPolicy::AllowEdgeShare),
                        )
                    })
                    .sum::<usize>()
            });
        });
    }
    group.finish();

    let shells = nested_shells(256, 64);
    let forest = ContainmentForest::new(&shells);
    let hole = circle_polygon(0.0, 0.0, 0.5, 64);
    c.bench_function("containment/assign_one_hole/256_nested_shells", |b| {
        b.iter(|| {
            forest.assign_hole(
                black_box(&hole),
                black_box(&shells),
                black_box(&TouchPolicy::AllowEdgeShare),
            )
        });
    });

    let shells = vec![circle_polygon(0.0, 0.0, 100.0, 1_024)];
    let forest = ContainmentForest::new(&shells);
    let hole = circle_polygon(0.0, 0.0, 0.75, 64);
    let mut group = c.benchmark_group("containment/touch_policy/1024x64");
    for policy in [
        TouchPolicy::AllowEdgeShare,
        TouchPolicy::AllowPointTouchDisallowEdgeShare,
        TouchPolicy::TreatAnyTouchAsDisjoint,
    ] {
        group.bench_with_input(format!("{policy:?}"), &policy, |b, policy| {
            b.iter(|| forest.assign_hole(black_box(&hole), black_box(&shells), black_box(policy)));
        });
    }
    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    let shell = circle_polygon(0.0, 0.0, 100.0, 1_024);
    let holes_1000 = holes(1_000, 64);
    let lines: Vec<_> = polygon_lines(&shell, 0)
        .chain(
            holes_1000
                .iter()
                .enumerate()
                .flat_map(|(i, hole)| polygon_lines(hole, (i + 1) as u32)),
        )
        .collect();
    let options = PolygonizerOptions::default();

    let mut diagnostic_options = options.clone();
    diagnostic_options.diagnostics.enabled = true;
    let mut diagnostic_polygonizer = Polygonizer::with_options(diagnostic_options);
    diagnostic_polygonizer.add_lines(lines.clone());
    let stats = diagnostic_polygonizer
        .polygonize()
        .unwrap()
        .diagnostics
        .unwrap()
        .containment_stats;
    assert_eq!(stats.max_point_in_ring_calls_per_shell, 1_000);
    assert_eq!(stats.shells_with_64_plus_point_in_ring_calls, 1);
    // Equal-radius rings can differ by a few ulps after translation, so exact
    // area ordering (and the number of short-circuited touch checks) is
    // target-dependent. Keep the benchmark contract focused on its workload.
    assert!(stats.shared_edge_checks >= 1_000);
    assert_eq!(stats.shared_edge_checks, stats.point_in_ring_calls);
    assert_eq!(
        stats.shared_edge_pair_checks,
        65_536_000 + stats.shared_edge_checks - 1_000
    );
    assert_eq!(stats.graph_edge_key_checks, 0);

    c.bench_function("containment/end_to_end/1000_holes", |b| {
        b.iter(|| {
            let mut polygonizer = Polygonizer::with_options(options.clone());
            polygonizer.add_lines(lines.clone());
            polygonizer.polygonize().unwrap()
        });
    });

    let holes = holes(100, 64);
    let lines: Vec<_> = polygon_lines(&shell, 0)
        .chain(
            holes
                .iter()
                .enumerate()
                .flat_map(|(i, hole)| polygon_lines(hole, (i + 1) as u32)),
        )
        .collect();
    let prepare = |node_input| {
        let options = PolygonizerOptions {
            node_input,
            ..Default::default()
        };
        let mut polygonizer = Polygonizer::with_options(options);
        polygonizer.add_lines(lines.clone());
        let result = polygonizer.polygonize().unwrap();
        let area: f64 = result
            .polygons
            .iter()
            .map(Polygon3D::unsigned_area_2d)
            .sum();
        (polygonizer, result.polygons.len(), area)
    };
    let (geometry, geometry_count, geometry_area) = prepare(false);
    let (graph, graph_count, graph_area) = prepare(true);
    assert_eq!(geometry_count, graph_count);
    assert!((geometry_area - graph_area).abs() < 1e-6);

    let mut group = c.benchmark_group("containment/reused_graph/100_holes");
    for (name, mut polygonizer) in [("geometry_fallback", geometry), ("graph_identity", graph)] {
        group.bench_function(name, |b| {
            b.iter(|| black_box(polygonizer.polygonize().unwrap()));
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_point_in_ring_crossover,
    bench_point_locators,
    bench_preparation,
    bench_filtering,
    bench_hole_assignment,
    bench_end_to_end
);
criterion_main!(benches);
