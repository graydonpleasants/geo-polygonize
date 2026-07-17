use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use geo_polygonize_core::containment::ContainmentForest;
use geo_polygonize_core::options::{PolygonizerOptions, TouchPolicy};
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
}

fn bench_end_to_end(c: &mut Criterion) {
    let shell = circle_polygon(0.0, 0.0, 100.0, 1_024);
    let holes = holes(1_000, 64);
    let lines: Vec<_> = polygon_lines(&shell, 0)
        .chain(
            holes
                .iter()
                .enumerate()
                .flat_map(|(i, hole)| polygon_lines(hole, (i + 1) as u32)),
        )
        .collect();
    let options = PolygonizerOptions::default();

    c.bench_function("containment/end_to_end/1000_holes", |b| {
        b.iter(|| {
            let mut polygonizer = Polygonizer::with_options(options.clone());
            polygonizer.add_lines(lines.clone());
            polygonizer.polygonize().unwrap()
        });
    });
}

criterion_group!(
    benches,
    bench_preparation,
    bench_filtering,
    bench_hole_assignment,
    bench_end_to_end
);
criterion_main!(benches);
