use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use geo_polygonize_core::Polygonizer;
use geo_types::{Geometry, GeometryCollection, LineString, MultiLineString, MultiPolygon, Polygon};

fn generate_deep_geometry(depth: usize) -> Geometry<f64> {
    let ls = LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]);
    let mut geom = Geometry::LineString(ls.clone());
    for _ in 0..depth {
        geom = Geometry::GeometryCollection(GeometryCollection::new_from(vec![
            geom,
            Geometry::LineString(ls.clone()),
        ]));
    }
    geom
}

fn bench_extract_segments(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_segments");
    for depth in [10, 50, 100].iter() {
        let geom = generate_deep_geometry(*depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, _| {
            b.iter(|| {
                let mut polygonizer = Polygonizer::new();
                polygonizer.add_borrowed_geometry(&geom);
            });
        });
    }
    group.finish();
}

fn generate_shallow_geometry(depth: usize) -> Geometry<f64> {
    let mut vec = Vec::new();
    let ls = LineString::from(vec![(0.0, 0.0), (1.0, 1.0)]);
    for _ in 0..depth {
        vec.push(Geometry::LineString(ls.clone()));
    }
    Geometry::GeometryCollection(GeometryCollection::new_from(vec))
}

fn bench_extract_segments_shallow(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_segments_shallow");
    for depth in [10, 50, 100].iter() {
        let geom = generate_shallow_geometry(*depth);
        group.bench_with_input(BenchmarkId::from_parameter(depth), depth, |b, _| {
            b.iter(|| {
                let mut polygonizer = Polygonizer::new();
                polygonizer.add_borrowed_geometry(&geom);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_extract_segments,
    bench_extract_segments_shallow
);
criterion_main!(benches);
