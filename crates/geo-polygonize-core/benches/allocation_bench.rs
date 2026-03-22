#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use geo_polygonize_core::Polygonizer;
use geo_polygonize_core::options::PolygonizerOptions;
use geo_types::{Coord, LineString};
use rand::{rngs::StdRng, Rng, SeedableRng};

fn main() {
    let profiler = dhat::Profiler::new_heap();

    // Create a complex dataset
    let mut rng = StdRng::seed_from_u64(42);
    let mut poly = Polygonizer::with_options(PolygonizerOptions {
        node_input: true,
        ..Default::default()
    });

    for _ in 0..100 {
        let n_points = rng.gen_range(5..20);
        let mut coords = Vec::with_capacity(n_points);
        for _ in 0..n_points {
            coords.push(Coord {
                x: rng.gen_range(0.0..100.0),
                y: rng.gen_range(0.0..100.0),
            });
        }
        poly.add_geometry(LineString::new(coords).into());
    }

    let _result = poly.polygonize().expect("Polygonization failed");

    drop(profiler);
}
