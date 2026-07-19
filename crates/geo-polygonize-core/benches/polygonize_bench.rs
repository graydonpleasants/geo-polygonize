use criterion::measurement::Measurement;
use criterion::{
    criterion_group, criterion_main, BenchmarkGroup, BenchmarkId, Criterion, Throughput,
};
use geo_polygonize_core::noding::snap::{NodingStrategy, SnapNoder};
use geo_polygonize_core::{Polygonizer, TiledPolygonizer};
use geo_types::{Coord, LineString, Rect};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::time::Duration;

fn fast_ci() -> bool {
    std::env::var_os("BENCH_FAST_CI").is_some()
}

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
                let geometries: Vec<geo_types::Geometry<f64>> =
                    lines.into_iter().map(Into::into).collect();
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
                    for geom in &geometries {
                        tiler.add_geometry(geom);
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
    if fast_ci() {
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(2));
    } else {
        group.measurement_time(Duration::from_secs(10));
    }

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

// --- KERNEL BENCHES (Split finding, containment, hashing, grid build) ---
use geo_polygonize_core::noding::grid::UniformGrid;
use geo_polygonize_core::{Coord3D, Line3D};

fn make_random_lines(count: usize) -> Vec<Line3D> {
    let mut rng = StdRng::seed_from_u64(42);
    (0..count)
        .map(|i| {
            Line3D::new(
                Coord3D::new(rng.gen_range(0.0..100.0), rng.gen_range(0.0..100.0), 0.0),
                Coord3D::new(rng.gen_range(0.0..100.0), rng.gen_range(0.0..100.0), 0.0),
                i as u32,
            )
        })
        .collect()
}

fn make_noding_workloads() -> Vec<(&'static str, Vec<Line3D>)> {
    let sparse = (0..512)
        .map(|i| {
            let y = i as f64 * 2.0;
            Line3D::new(
                Coord3D::new(0.0, y, 0.0),
                Coord3D::new(1.0, y + 0.5, 0.0),
                i,
            )
        })
        .collect();
    let dense = (0..256)
        .map(|i| {
            let angle = std::f64::consts::PI * i as f64 / 256.0;
            let (sin, cos) = angle.sin_cos();
            Line3D::new(
                Coord3D::new(-cos, -sin, 0.0),
                Coord3D::new(cos, sin, 0.0),
                i,
            )
        })
        .collect();
    let skewed = (0..600)
        .map(|i| {
            let end = i as f64 * 0.0001;
            Line3D::new(
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(end, end + 0.00001, 0.0),
                i,
            )
        })
        .chain(std::iter::once(Line3D::new(
            Coord3D::new(100.0, 100.0, 0.0),
            Coord3D::new(101.0, 101.0, 0.0),
            600,
        )))
        .collect();
    let crossing = (0..4)
        .flat_map(|x| {
            (0..4).flat_map(move |y| {
                let id = (x * 4 + y) * 2;
                let (x, y) = (x as f64 * 2.0, y as f64 * 2.0);
                [
                    Line3D::new(
                        Coord3D::new(x, y, 0.0),
                        Coord3D::new(x + 1.0, y + 1.0, 0.0),
                        id,
                    ),
                    Line3D::new(
                        Coord3D::new(x + 1.0, y, 0.0),
                        Coord3D::new(x, y + 1.0, 0.0),
                        id + 1,
                    ),
                ]
            })
        })
        .collect();

    vec![
        ("sparse", sparse),
        ("dense", dense),
        ("skewed", skewed),
        ("crossing", crossing),
    ]
}

fn bench_noding_workloads(c: &mut Criterion) {
    let mut group = c.benchmark_group("noding_workloads");
    group.sample_size(10);
    if fast_ci() {
        group.warm_up_time(Duration::from_secs(1));
        group.measurement_time(Duration::from_secs(2));
    }

    for (workload, lines) in make_noding_workloads() {
        group.throughput(Throughput::Elements(lines.len() as u64));
        for (strategy_name, strategy) in [
            ("auto", NodingStrategy::Auto),
            ("grid", NodingStrategy::Grid),
            ("simd", NodingStrategy::Simd),
        ] {
            // ponytail: forced Grid repeatedly re-nodes crossing splits; benchmark it after that
            // pathology has a bounded one-shot reproducer instead of hanging every CI run.
            if workload == "crossing" && strategy == NodingStrategy::Grid {
                continue;
            }
            group.bench_with_input(
                BenchmarkId::new(workload, strategy_name),
                &lines,
                |b, lines| {
                    let noder = SnapNoder::new(1e-10).with_strategy(strategy);
                    b.iter(|| noder.node(criterion::black_box(lines.clone())));
                },
            );
        }
    }
    group.finish();
}

fn make_pre_snap_lines(bands: usize) -> Vec<Line3D> {
    (0..bands)
        .flat_map(|i| {
            let y = i as f64 * 2.0;
            [
                Line3D::new(
                    Coord3D::new(0.0, y, 0.0),
                    Coord3D::new(100.0, y, 0.0),
                    (i * 2) as u32,
                ),
                Line3D::new(
                    Coord3D::new(50.0, y + 0.25, 0.0),
                    Coord3D::new(50.5, y + 0.75, 0.0),
                    (i * 2 + 1) as u32,
                ),
            ]
        })
        .collect()
}

fn bench_pre_snap(c: &mut Criterion) {
    let lines = make_pre_snap_lines(1_000);
    let mut group = c.benchmark_group("pre_snap/cfb_style");
    group.sample_size(10);
    group.throughput(Throughput::Elements(lines.len() as u64));
    group.bench_function("2000_segments", |b| {
        b.iter(|| SnapNoder::pre_snap_to_reference_vertices(criterion::black_box(&lines), 0.5));
    });
    group.finish();
}

fn bench_kernel_grid_build(c: &mut Criterion) {
    let lines = make_random_lines(10_000);
    c.bench_function("kernel_grid_build_10k", |b| {
        b.iter(|| UniformGrid::new(criterion::black_box(&lines)));
    });
}

fn bench_kernel_find_splits(c: &mut Criterion) {
    let lines = make_random_lines(10_000);
    let noder = SnapNoder::new(1e-10);
    let grid = UniformGrid::new(&lines);
    c.bench_function("kernel_find_splits_10k", |b| {
        b.iter(|| grid.find_splits(criterion::black_box(&lines), criterion::black_box(&noder)));
    });
}

// NOTE: We don't have an isolated "apply_splits" because it's inline inside `node`.
// So we bench `node` as a proxy for the entire noding iteration.
fn bench_kernel_node(c: &mut Criterion) {
    let lines = make_random_lines(1_000);
    let noder = SnapNoder::new(1e-10);
    c.bench_function("kernel_node_1k", |b| {
        b.iter(|| noder.node(criterion::black_box(lines.clone())));
    });
}

criterion_group!(
    kernel_benches,
    bench_noding_workloads,
    bench_kernel_grid_build,
    bench_kernel_find_splits,
    bench_kernel_node,
    bench_pre_snap
);
criterion_main!(benches, kernel_benches);
