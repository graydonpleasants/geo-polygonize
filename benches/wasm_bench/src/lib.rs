use wasm_bindgen::prelude::*;
use geo_polygonize::Polygonizer;
use geo_types::{LineString, Geometry, GeometryCollection};
// use talc::*;
use geoarrow::array::LineStringArray;
use geoarrow::trait_::GeometryScalarTrait; // For .to_geo()
use arrow::array::Array;

// #[global_allocator]
// static ALLOCATOR: TalckWasm = unsafe { TalckWasm::new_global() };

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

    pub fn load_geoarrow(size: usize) -> BenchmarkContext {
        let mut lines = Vec::new();
        for i in 0..size {
            lines.push(LineString::from(vec![(i as f64, 0.0), (i as f64, size as f64)]));
            lines.push(LineString::from(vec![(0.0, i as f64), (size as f64, i as f64)]));
        }

        // Conversion to GeoArrow
        // LineStringArray in 0.7.0 is not generic over offset (uses i32)
        // With features=["geo"], From<Vec<geo::LineString>> is implemented
        let array: LineStringArray = LineStringArray::from(lines);

        let mut polygonizer = Polygonizer::new();
        polygonizer.node_input = true;

        // Iterate over scalars
        for scalar_maybe in array.iter_values() {
             // array.iter_values() returns an iterator of Option<Scalar> if validity exists?
             // Or just Scalar if not?
             // LineStringArray::iter_values() -> impl Iterator<Item = Option<LineString<'_>>> usually.
             // Let's assume Option based on common Arrow patterns.
             if let Some(scalar) = scalar_maybe {
                 let geom: LineString<f64> = scalar.to_geo();
                 polygonizer.add_geometry(Geometry::LineString(geom));
             }
        }

        BenchmarkContext { polygonizer }
    }

    pub fn run(&mut self) {
        let _ = self.polygonizer.polygonize().unwrap();
    }
}
