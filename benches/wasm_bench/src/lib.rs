use wasm_bindgen::prelude::*;
use geo::{LineString, Geometry};
use geo_polygonize::Polygonizer;
use geoarrow::array::{GeoArrowArrayAccessor, LineStringBuilder};
use geoarrow::datatypes::{LineStringType, Dimension};
use std::convert::TryInto;

#[cfg(target_arch = "wasm32")]
use talc::*;

#[cfg(target_arch = "wasm32")]
#[global_allocator]
static ALLOCATOR: TalckWasm = unsafe { TalckWasm::new_global() };

#[wasm_bindgen]
pub fn setup_panic_hook() {
    console_error_panic_hook::set_once();
}

fn parse_input(lines: JsValue) -> Result<Vec<LineString>, JsValue> {
    // Deserialize as Vec<geojson::Geometry>
    let geometries: Vec<geojson::Geometry> = serde_wasm_bindgen::from_value(lines)?;

    let mut geo_lines = Vec::with_capacity(geometries.len());
    for g in geometries {
        // Convert geojson::Geometry to geo::Geometry
        let geo_geom: Geometry<f64> = g.try_into()
            .map_err(|e| JsValue::from_str(&format!("GeoJSON conversion error: {}", e)))?;

        match geo_geom {
            Geometry::LineString(ls) => geo_lines.push(ls),
            _ => return Err(JsValue::from_str("Input must be LineStrings")),
        }
    }
    Ok(geo_lines)
}

#[wasm_bindgen]
pub fn polygonize(lines: JsValue) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;

    // Core Logic
    let mut polygonizer = Polygonizer::new();
    for line in lines {
        polygonizer.add_geometry(Geometry::LineString(line));
    }
    let results = polygonizer.polygonize();

    let results_vec = results.map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
    Ok(JsValue::from(results_vec.len()))
}

#[wasm_bindgen]
pub fn polygonize_robust(lines: JsValue, grid_size: Option<f64>) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = true;
    if let Some(g) = grid_size {
        polygonizer.snap_grid_size = g;
    }

    for line in lines {
        polygonizer.add_geometry(Geometry::LineString(line));
    }
    let results = polygonizer.polygonize();

    let results_vec = results.map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
    Ok(JsValue::from(results_vec.len()))
}

#[wasm_bindgen]
pub fn load_geoarrow(lines: JsValue) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;

    // Core Logic: Ingest
    let mut builder = LineStringBuilder::new(LineStringType::new(Dimension::XY, Default::default()));
    for line in &lines {
        builder.push_line_string(Some(line))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    }
    let array = builder.finish();

    // Core Logic: Iterate
    let mut count = 0;
    for scalar_result in array.iter_values() {
         if let Ok(_scalar) = scalar_result {
             count += 1;
         }
    }

    Ok(JsValue::from(count))
}
