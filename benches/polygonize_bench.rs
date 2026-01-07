use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use geo_polygonize::Polygonizer;
use geo_types::LineString;

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

fn bench_polygonize(c: &mut Criterion) {
    let mut group = c.benchmark_group("polygonize");
    group.sample_size(10); // Reduce sample size for speed

    for size in [5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::new("grid", size), size, |b, &size| {
            let lines = generate_grid(size);
            b.iter(|| {
                let mut poly = Polygonizer::new();
                for line in &lines {
                    poly.add_geometry(line.clone().into());
                }
                // Grid inputs are already noded at intersections if we add segments?
                // My `generate_grid` adds LONG lines (0..N).
                // They cross at integer coordinates.
                // So `node_input` MUST be true for this to work correctly!
                // If node_input is false, graph edges cross without nodes.
                // This stress tests the graph builder but returns 0 polygons (dangles).

                // Let's benchmark WITH noding to test the noder performance too.
                poly.node_input = true;
                poly.polygonize().unwrap();
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_polygonize);
criterion_main!(benches);
