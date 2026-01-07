use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use geo_polygonize::Polygonizer;
use geo_types::LineString;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

fn generate_grid(n: usize) -> Vec<LineString<f64>> {
    let mut lines = Vec::new();
    for i in 0..=n {
        // Horizontal
        lines.push(LineString::from(vec![
            (0.0, i as f64),
            (n as f64, i as f64)
        ]));
        // Vertical
        lines.push(LineString::from(vec![
            (i as f64, 0.0),
            (i as f64, n as f64)
        ]));
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
        lines.push(LineString::from(vec![
            (x1, y1),
            (x2, y2)
        ]));
    }
    lines
}

fn bench_polygonize(c: &mut Criterion) {
    let mut group = c.benchmark_group("polygonize");
    group.sample_size(10);
    group.measurement_time(std::time::Duration::from_secs(10));

    // Grid sizes
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
    }

    // Random line counts
    // Limiting to 200 as 500 takes too long in the current implementation
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

    group.finish();
}

criterion_group!(benches, bench_polygonize);
criterion_main!(benches);
