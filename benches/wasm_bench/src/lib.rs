use wasm_bindgen::prelude::*;
use geo_polygonize::Polygonizer;
use geo_types::{LineString, Geometry, GeometryCollection};
use talc::*;

#[global_allocator]
static ALLOCATOR: TalckWasm = unsafe { TalckWasm::new_global() };

#[wasm_bindgen]
pub fn setup_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct BenchmarkContext {
    polygonizer: Polygonizer,
}

#[wasm_bindgen]
impl BenchmarkContext {
    pub fn new(size: usize) -> BenchmarkContext {
        let mut lines = Vec::new();
        // Generate grid
        for i in 0..size {
            // Vertical
            lines.push(LineString::from(vec![
                (i as f64, 0.0),
                (i as f64, size as f64),
            ]));
            // Horizontal
            lines.push(LineString::from(vec![
                (0.0, i as f64),
                (size as f64, i as f64),
            ]));
        }

        let geom_coll: GeometryCollection<f64> = lines.into_iter().map(Geometry::LineString).collect();
        let geom = Geometry::GeometryCollection(geom_coll);

        let mut polygonizer = Polygonizer::new();
        polygonizer.node_input = true; // Force noding
        polygonizer.add_geometry(geom);

        BenchmarkContext { polygonizer }
    }

    pub fn run(&mut self) {
        let _ = self.polygonizer.polygonize().unwrap();
    }
}
