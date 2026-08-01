mod common;

use common::generate_grid;
use geo_polygonize_core::{Polygonizer, PolygonizerOptions};
use geo_types::LineString;
use iai_callgrind::{library_benchmark, library_benchmark_group, main};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

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

#[library_benchmark]
#[bench::grid_10(generate_grid(10))]
#[bench::grid_20(generate_grid(20))]
fn bench_polygonize_grid(lines: Vec<LineString<f64>>) {
    let mut poly = Polygonizer::with_options(PolygonizerOptions {
        node_input: true,
        ..Default::default()
    });
    for line in lines {
        poly.add_geometry(line.into());
    }
    let _ = poly.polygonize();
}

#[library_benchmark]
#[bench::random_50(generate_random_lines(50, 42))]
#[bench::random_100(generate_random_lines(100, 42))]
fn bench_polygonize_random(lines: Vec<LineString<f64>>) {
    let mut poly = Polygonizer::with_options(PolygonizerOptions {
        node_input: true,
        ..Default::default()
    });
    for line in lines {
        poly.add_geometry(line.into());
    }
    let _ = poly.polygonize();
}

library_benchmark_group!(
    name = polygonize_group;
    benchmarks = bench_polygonize_grid, bench_polygonize_random
);

main!(library_benchmark_groups = polygonize_group);
