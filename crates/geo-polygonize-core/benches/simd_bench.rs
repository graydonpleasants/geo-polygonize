use criterion::{criterion_group, criterion_main, Criterion};
use geo_polygonize_core::types::Coord3D;
use geo_polygonize_core::utils::simd::SimdRing;
use geo_types::Coord;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

fn bench_simd_new(c: &mut Criterion) {
    let mut rng = StdRng::seed_from_u64(42);
    let coords: Vec<Coord<f64>> = (0..1000)
        .map(|_| Coord {
            x: rng.gen(),
            y: rng.gen(),
        })
        .collect();

    c.bench_function("simd_ring_new", |b| b.iter(|| SimdRing::new(&coords)));

    let coords3d: Vec<Coord3D> = (0..1000)
        .map(|_| Coord3D {
            x: rng.gen(),
            y: rng.gen(),
            z: 0.0,
        })
        .collect();

    c.bench_function("simd_ring_new_3d", |b| {
        b.iter(|| SimdRing::new_3d(&coords3d))
    });
}
criterion_group!(benches, bench_simd_new);
criterion_main!(benches);
