use criterion::measurement::Measurement;
use criterion::{criterion_group, criterion_main, BenchmarkGroup, BenchmarkId, Criterion};
use geo_polygonize_core::noding::snap::{NodingStrategy, SnapNoder};
use geo_polygonize_core::{Polygonizer, TiledPolygonizer};
use geo_types::{Coord, LineString, Rect};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn generate_grid(n: usize) -> Vec<LineString<f64>> {
    let mut lines = Vec::new();
    for i in 0..=n {
        // Horizontal
        lines.push(LineString::from(vec![
            (0.0, i as f64),
            (n as f64, i as f64),
        ]));
        // Vertical
        lines.push(LineString::from(vec![
            (i as f64, 0.0),
            (i as f64, n as f64),
        ]));
    }
    lines
}

// Generates a grid with bowtie patterns in every cell, guaranteeing intersections.
// This specifically stresses the noding algorithm (SnapNoder).
fn generate_bowtie_grid(n: usize) -> Vec<LineString<f64>> {
    let mut lines = Vec::new();
    for i in 0..n {
        for j in 0..n {
            // Bowtie (X) in the cell [i, i+1] x [j, j+1]
            let x = i as f64;
            let y = j as f64;
            lines.push(LineString::from(vec![(x, y), (x + 1.0, y + 1.0)]));
            lines.push(LineString::from(vec![(x + 1.0, y), (x, y + 1.0)]));
        }
    }
    lines
}

fn generate_random_lines(n: usize, seed: u64) -> Vec<LineString<f64>> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut lines = Vec::new();
    for _ in 0..n {
        let x1 = rng.gen_range(0.0..100.0);
        let y1 = rng.gen_range(0.0..100.0);
        let x2 = rng.gen_range(0.0..100.0);
        let y2 = rng.gen_range(0.0..100.0);
        lines.push(LineString::from(vec![(x1, y1), (x2, y2)]));
    }
    lines
}

fn generate_parallel_lines(n: usize) -> Vec<LineString<f64>> {
    let mut lines = Vec::new();
    for i in 0..n {
        lines.push(LineString::from(vec![(0.0, i as f64), (10.0, i as f64)]));
    }
    lines
}

fn bench_grid_scenarios<M: Measurement>(group: &mut BenchmarkGroup<'_, M>) {
    let grid_sizes = [5, 10, 20, 50, 100];
    for &size in grid_sizes.iter() {
        group.bench_with_input(BenchmarkId::new("grid", size), &size, |b, &size| {
            let lines = generate_grid(size);
            b.iter(|| {
                let mut poly = Polygonizer::new();
                for line in &lines {
                    poly.add_geometry(line.clone().into());
                }
                poly.node_input = true;
                poly.polygonize().unwrap();
            });
        });

        // Benchmark Tiled version for larger sizes
        if size >= 50 {
            group.bench_with_input(BenchmarkId::new("grid_tiled", size), &size, |b, &size| {
                let lines = generate_grid(size);
                // BBox is roughly 0,0 to size,size
                let bbox = Rect::new(
                    Coord { x: 0.0, y: 0.0 },
                    Coord {
                        x: size as f64,
                        y: size as f64,
                    },
                );
                // Tile size roughly size/2 to get 4 tiles?
                let tile_size = (size as f64) / 2.0;

                b.iter(|| {
                    let mut tiler = TiledPolygonizer::new(bbox, tile_size).with_buffer(1.0);
                    for line in &lines {
                        tiler.add_geometry(line.clone().into());
                    }
                    tiler.polygonize();
                });
            });
        }
    }
}

fn bench_bowtie_scenarios<M: Measurement>(group: &mut BenchmarkGroup<'_, M>) {
    let dirty_sizes = [10, 20, 50];
    for &size in dirty_sizes.iter() {
        let lines = generate_bowtie_grid(size);

        // Auto Strategy (Default)
        group.bench_with_input(
            BenchmarkId::new("bowtie_grid_auto", size),
            &size,
            |b, &_size| {
                b.iter(|| {
                    let mut input_segments = Vec::new();
                    for ls in &lines {
                        for line in ls.lines() {
                            input_segments.push(line.into());
                        }
                    }
                    let noder = SnapNoder::new(1e-10); // Auto
                    noder.node(input_segments);
                });
            },
        );

        // Force Grid
        group.bench_with_input(
            BenchmarkId::new("bowtie_grid_force_grid", size),
            &size,
            |b, &_size| {
                b.iter(|| {
                    let mut input_segments = Vec::new();
                    for ls in &lines {
                        for line in ls.lines() {
                            input_segments.push(line.into());
                        }
                    }
                    let noder = SnapNoder::new(1e-10).with_strategy(NodingStrategy::Grid);
                    noder.node(input_segments);
                });
            },
        );

        // Force SIMD (Brute Force) - CAUTION: O(N^2)
        if size <= 20 {
            group.bench_with_input(
                BenchmarkId::new("bowtie_grid_force_simd", size),
                &size,
                |b, &_size| {
                    b.iter(|| {
                        let mut input_segments = Vec::new();
                        for ls in &lines {
                            for line in ls.lines() {
                                input_segments.push(line.into());
                            }
                        }
                        let noder = SnapNoder::new(1e-10).with_strategy(NodingStrategy::Simd);
                        noder.node(input_segments);
                    });
                },
            );
        }
    }
}

fn bench_random_scenarios<M: Measurement>(group: &mut BenchmarkGroup<'_, M>) {
    let random_counts = [50, 100, 200];
    for &count in random_counts.iter() {
        group.bench_with_input(BenchmarkId::new("random", count), &count, |b, &count| {
            let lines = generate_random_lines(count, 42);
            b.iter(|| {
                let mut poly = Polygonizer::new();
                for line in &lines {
                    poly.add_geometry(line.clone().into());
                }
                poly.node_input = true;
                poly.polygonize().unwrap();
            });
        });
    }
}

fn bench_parallel_scenarios<M: Measurement>(group: &mut BenchmarkGroup<'_, M>) {
    group.bench_function("large_parallel_10k", |b| {
        let lines = generate_parallel_lines(10_000);
        let mut input_segments = Vec::new();
        for ls in &lines {
            for line in ls.lines() {
                input_segments.push(line.into());
            }
        }
        b.iter(|| {
            let noder = SnapNoder::new(1e-10).with_strategy(NodingStrategy::Grid);
            noder.node(input_segments.clone());
        });
    });
}

fn bench_polygonize(c: &mut Criterion) {
    let mut group = c.benchmark_group("polygonize");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(10));

    bench_grid_scenarios(&mut group);
    bench_bowtie_scenarios(&mut group);
    bench_random_scenarios(&mut group);
    bench_parallel_scenarios(&mut group);

    group.finish();
}

fn bench_get_edge_rings(c: &mut Criterion) {
    let mut group = c.benchmark_group("planar_graph");
    let size = 50;
    let lines = generate_grid(size);

    group.bench_function("get_edge_rings", |b| {
        b.iter_with_setup(
            || {
                let mut graph = geo_polygonize_core::graph::PlanarGraph::new();
                for line in &lines {
                    graph.add_line_string(line.clone());
                }
                graph.sort_edges();
                graph
            },
            |mut graph| graph.get_edge_rings(),
        );
    });
    group.finish();
}

fn bench_get_edge_rings_dangles(c: &mut Criterion) {
    let mut group = c.benchmark_group("planar_graph_dangles");
    // Use a large enough count to trigger allocations
    let count = 500;
    let lines = generate_random_lines(count, 12345);

    group.bench_function("get_edge_rings_with_dangles", |b| {
        b.iter_with_setup(
            || {
                let mut graph = geo_polygonize_core::graph::PlanarGraph::new();
                for line in &lines {
                    graph.add_line_string(line.clone());
                }
                // Pruning dangles will mark some edges, causing nodes to have
                // valid degree < outgoing.len(), forcing the slow path.
                loop {
                    if graph.prune_dangles().is_empty() {
                        break;
                    }
                }
                graph.sort_edges();
                graph
            },
            |mut graph| graph.get_edge_rings(),
        );
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_polygonize,
    bench_get_edge_rings,
    bench_get_edge_rings_dangles
);
criterion_main!(benches);
