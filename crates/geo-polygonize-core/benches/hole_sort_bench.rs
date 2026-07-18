use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use fearless_simd::{dispatch, f64x4, prelude::*, Level};
use geo::algorithm::indexed::IntervalTreeMultiPolygon;
use geo::{Contains, Coord, MultiPolygon};
use geo_polygonize_core::containment::ContainmentForest;
use geo_polygonize_core::options::{PolygonizerOptions, TouchPolicy};
use geo_polygonize_core::utils::simd::SimdRing;
use geo_polygonize_core::utils::soa::SoALines;
use geo_polygonize_core::{Coord3D, Line3D, Polygon3D, Polygonizer};
use std::f64::consts::PI;

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
    for i in 0..len.saturating_sub(1) {
        if ((y[i] > point.y) != (y[i + 1] > point.y))
            && point.x < (x[i + 1] - x[i]) * (point.y - y[i]) / (y[i + 1] - y[i]) + x[i]
        {
            crossings += 1;
        }
    }
    crossings % 2 != 0
}

#[inline(always)]
fn fearless_contains<S, V>(simd: S, x: &[f64], y: &[f64], len: usize, point: Coord<f64>) -> bool
where
    S: Simd,
    V: SimdFloat<S, Element = f64>,
{
    let px = V::splat(simd, point.x);
    let py = V::splat(simd, point.y);
    let n = len.saturating_sub(1);
    let mut crossings = 0;
    let mut i = 0;

    while i + V::N <= n {
        let xi = V::from_slice(simd, &x[i..i + V::N]);
        let yi = V::from_slice(simd, &y[i..i + V::N]);
        let xj = V::from_slice(simd, &x[i + 1..i + V::N + 1]);
        let yj = V::from_slice(simd, &y[i + 1..i + V::N + 1]);
        let in_range = yi.simd_gt(py) ^ yj.simd_gt(py);
        let intersect_x = ((xj - xi) * (py - yi) / (yj - yi)) + xi;
        crossings += (in_range & intersect_x.simd_gt(px))
            .to_bitmask()
            .count_ones();
        i += V::N;
    }

    while i < n {
        if ((y[i] > point.y) != (y[i + 1] > point.y))
            && point.x < (x[i + 1] - x[i]) * (point.y - y[i]) / (y[i + 1] - y[i]) + x[i]
        {
            crossings += 1;
        }
        i += 1;
    }
    crossings % 2 != 0
}

#[inline(always)]
fn count_fixed<S: Simd>(simd: S, x: &[f64], y: &[f64], len: usize, points: &[Coord<f64>]) -> usize {
    points
        .iter()
        .filter(|point| fearless_contains::<S, f64x4<S>>(simd, x, y, len, **point))
        .count()
}

#[inline(always)]
fn count_natural<S: Simd>(
    simd: S,
    x: &[f64],
    y: &[f64],
    len: usize,
    points: &[Coord<f64>],
) -> usize {
    points
        .iter()
        .filter(|point| fearless_contains::<S, S::f64s>(simd, x, y, len, **point))
        .count()
}

fn aabb_lines(count: usize) -> Vec<Line3D> {
    (0..count)
        .map(|i| {
            let x = (i % 256) as f64;
            let y = (i / 256) as f64;
            Line3D::new(
                Coord3D::new(x, y, 0.0),
                Coord3D::new(x + 0.75, y + 0.5, 0.0),
                i as u32,
            )
        })
        .collect()
}

fn bbox(line: Line3D) -> (f64, f64, f64, f64) {
    (
        line.start.x.min(line.end.x),
        line.start.y.min(line.end.y),
        line.start.x.max(line.end.x),
        line.start.y.max(line.end.y),
    )
}

fn scalar_aabb_count(lines: &[Line3D], query: Line3D) -> usize {
    let (q_min_x, q_min_y, q_max_x, q_max_y) = bbox(query);
    lines
        .iter()
        .filter(|line| {
            let (min_x, min_y, max_x, max_y) = bbox(**line);
            q_min_x <= max_x && q_max_x >= min_x && q_min_y <= max_y && q_max_y >= min_y
        })
        .count()
}

fn wide_aabb_count(soa: &SoALines, query: Line3D) -> usize {
    (0..soa.len())
        .step_by(4)
        .map(|i| soa.intersects_bbox_batch(query, i).count_ones() as usize)
        .sum()
}

#[inline(always)]
fn fearless_aabb_count<S, V>(simd: S, soa: &SoALines, len: usize, query: Line3D) -> usize
where
    S: Simd,
    V: SimdFloat<S, Element = f64>,
{
    let (q_min_x, q_min_y, q_max_x, q_max_y) = bbox(query);
    let q_min_x_v = V::splat(simd, q_min_x);
    let q_min_y_v = V::splat(simd, q_min_y);
    let q_max_x_v = V::splat(simd, q_max_x);
    let q_max_y_v = V::splat(simd, q_max_y);
    let mut matches = 0;
    let mut i = 0;

    while i + V::N <= len {
        let min_x = V::from_slice(simd, &soa.min_x[i..i + V::N]);
        let min_y = V::from_slice(simd, &soa.min_y[i..i + V::N]);
        let max_x = V::from_slice(simd, &soa.max_x[i..i + V::N]);
        let max_y = V::from_slice(simd, &soa.max_y[i..i + V::N]);
        let overlap = q_min_x_v.simd_le(max_x)
            & q_max_x_v.simd_ge(min_x)
            & q_min_y_v.simd_le(max_y)
            & q_max_y_v.simd_ge(min_y);
        matches += overlap.to_bitmask().count_ones() as usize;
        i += V::N;
    }

    while i < len {
        if q_min_x <= soa.max_x[i]
            && q_max_x >= soa.min_x[i]
            && q_min_y <= soa.max_y[i]
            && q_max_y >= soa.min_y[i]
        {
            matches += 1;
        }
        i += 1;
    }
    matches
}

#[inline(always)]
fn aabb_count_fixed<S: Simd>(simd: S, soa: &SoALines, len: usize, query: Line3D) -> usize {
    fearless_aabb_count::<S, f64x4<S>>(simd, soa, len, query)
}

#[inline(always)]
fn aabb_count_natural<S: Simd>(simd: S, soa: &SoALines, len: usize, query: Line3D) -> usize {
    fearless_aabb_count::<S, S::f64s>(simd, soa, len, query)
}

fn bench_fearless_point_in_ring(c: &mut Criterion) {
    let level = Level::new();
    let points = locator_query_points(1_024);
    let mut group = c.benchmark_group("fearless_simd/point_in_ring");

    for edges in [64, 256, 1_024, 16_384] {
        let ring = circle_polygon(0.0, 0.0, 100.0, edges);
        let wide = SimdRing::new_3d(&ring.exterior);
        let len = ring.exterior.len();
        let expected = points
            .iter()
            .filter(|point| scalar_contains(&wide.x, &wide.y, len, **point))
            .count();
        assert_eq!(
            expected,
            points.iter().filter(|point| wide.contains(**point)).count()
        );
        assert_eq!(
            expected,
            dispatch!(level, simd => count_fixed(simd, &wide.x, &wide.y, len, &points))
        );
        assert_eq!(
            expected,
            dispatch!(level, simd => count_natural(simd, &wide.x, &wide.y, len, &points))
        );

        group.throughput(Throughput::Elements((edges * points.len()) as u64));
        group.bench_function(BenchmarkId::new("scalar", edges), |b| {
            b.iter(|| {
                black_box(&points)
                    .iter()
                    .filter(|point| scalar_contains(&wide.x, &wide.y, len, **point))
                    .count()
            });
        });
        group.bench_function(BenchmarkId::new("production", edges), |b| {
            b.iter(|| {
                black_box(&points)
                    .iter()
                    .filter(|point| wide.contains(**point))
                    .count()
            });
        });
        group.bench_function(BenchmarkId::new("fearless_fixed_f64x4", edges), |b| {
            b.iter(|| {
                dispatch!(level, simd => count_fixed(
                    simd,
                    &wide.x,
                    &wide.y,
                    len,
                    black_box(&points)
                ))
            });
        });
        group.bench_function(BenchmarkId::new("fearless_natural", edges), |b| {
            b.iter(|| {
                dispatch!(level, simd => count_natural(
                    simd,
                    &wide.x,
                    &wide.y,
                    len,
                    black_box(&points)
                ))
            });
        });
    }
    group.finish();
}

fn bench_fearless_aabb_scan(c: &mut Criterion) {
    let level = Level::new();
    let query = Line3D::new(
        Coord3D::new(64.0, 8.0, 0.0),
        Coord3D::new(192.0, 192.0, 0.0),
        0,
    );
    let mut group = c.benchmark_group("fearless_simd/soa_aabb_scan");

    for count in [256, 4_096, 65_536] {
        let lines = aabb_lines(count);
        let soa = SoALines::new(&lines);
        let expected = scalar_aabb_count(&lines, query);
        assert_eq!(expected, wide_aabb_count(&soa, query));
        assert_eq!(
            expected,
            dispatch!(level, simd => aabb_count_fixed(simd, &soa, lines.len(), query))
        );
        assert_eq!(
            expected,
            dispatch!(level, simd => aabb_count_natural(simd, &soa, lines.len(), query))
        );

        group.throughput(Throughput::Elements(count as u64));
        group.bench_function(BenchmarkId::new("scalar", count), |b| {
            b.iter(|| scalar_aabb_count(black_box(&lines), query));
        });
        group.bench_function(BenchmarkId::new("wide", count), |b| {
            b.iter(|| wide_aabb_count(black_box(&soa), query));
        });
        group.bench_function(BenchmarkId::new("fearless_fixed_f64x4", count), |b| {
            b.iter(|| {
                dispatch!(level, simd => aabb_count_fixed(
                    simd,
                    black_box(&soa),
                    lines.len(),
                    query
                ))
            });
        });
        group.bench_function(BenchmarkId::new("fearless_natural", count), |b| {
            b.iter(|| {
                dispatch!(level, simd => aabb_count_natural(
                    simd,
                    black_box(&soa),
                    lines.len(),
                    query
                ))
            });
        });
    }
    group.finish();
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
    assert_eq!(stats.shared_edge_checks, 1_000);
    assert_eq!(stats.shared_edge_pair_checks, 65_536_000);
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
    bench_fearless_point_in_ring,
    bench_fearless_aabb_scan,
    bench_point_locators,
    bench_preparation,
    bench_filtering,
    bench_hole_assignment,
    bench_end_to_end
);
criterion_main!(benches);
