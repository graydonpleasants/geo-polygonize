use arrow_ipc::reader::StreamReader;
use geo::{Coord, LineString};
use geo_polygonize_core::Polygonizer;
use geo_traits::to_geo::ToGeoLineString;
use geoarrow::array::{GeoArrowArrayAccessor, LineStringArray};
use geojson::{GeoJson, Geometry, Value};
use std::convert::TryInto;
use std::io::Cursor;
use std::str::FromStr;
use wasm_bindgen::prelude::*;

#[cfg(feature = "threads")]
pub use wasm_bindgen_rayon::init_thread_pool;

#[wasm_bindgen]
pub fn polygonize(geojson_str: &str) -> Result<String, JsValue> {
    // Set panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let geojson = GeoJson::from_str(geojson_str)
        .map_err(|e| JsValue::from_str(&format!("Invalid GeoJSON: {}", e)))?;

    let mut polygonizer = Polygonizer::new();

    // Process inputs
    match geojson {
        GeoJson::FeatureCollection(fc) => {
            for feature in fc.features {
                if let Some(geom) = feature.geometry {
                    let geo_geom: geo::Geometry<f64> = geom
                        .try_into()
                        .map_err(|e| JsValue::from_str(&format!("Conversion error: {}", e)))?;
                    polygonizer.add_geometry(geo_geom);
                }
            }
        }
        GeoJson::Feature(f) => {
            if let Some(geom) = f.geometry {
                let geo_geom: geo::Geometry<f64> = geom
                    .try_into()
                    .map_err(|e| JsValue::from_str(&format!("Conversion error: {}", e)))?;
                polygonizer.add_geometry(geo_geom);
            }
        }
        GeoJson::Geometry(g) => {
            let geo_geom: geo::Geometry<f64> = g
                .try_into()
                .map_err(|e| JsValue::from_str(&format!("Conversion error: {}", e)))?;
            polygonizer.add_geometry(geo_geom);
        }
    }

    let result = polygonizer
        .polygonize()
        .map_err(|e| JsValue::from_str(&format!("Polygonization error: {}", e)))?;

    // Convert back to GeoJSON
    let geometries: Vec<Geometry> = result.polygons
        .into_iter()
        .map(|p| Geometry::new(Value::from(&p)))
        .collect();

    // Wrap in FeatureCollection? Or GeometryCollection?
    // Let's return a FeatureCollection as it's standard for multiple geometries
    let mut features = Vec::new();
    for geom in geometries {
        features.push(geojson::Feature {
            bbox: None,
            geometry: Some(geom),
            id: None,
            properties: None,
            foreign_members: None,
        });
    }

    let fc = GeoJson::FeatureCollection(geojson::FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    });

    Ok(fc.to_string())
}

#[wasm_bindgen]
pub struct WasmPolygonResult {
    // Flat representation of output polygons
    coords: Vec<f64>,
    ring_offsets: Vec<u32>,
    polygon_offsets: Vec<u32>,
}

#[wasm_bindgen]
impl WasmPolygonResult {
    // Expose memory directly to JS to avoid copying
    pub fn coords_ptr(&self) -> *const f64 {
        self.coords.as_ptr()
    }
    pub fn coords_len(&self) -> usize {
        self.coords.len()
    }

    pub fn ring_offsets_ptr(&self) -> *const u32 {
        self.ring_offsets.as_ptr()
    }
    pub fn ring_offsets_len(&self) -> usize {
        self.ring_offsets.len()
    }

    pub fn polygon_offsets_ptr(&self) -> *const u32 {
        self.polygon_offsets.as_ptr()
    }
    pub fn polygon_offsets_len(&self) -> usize {
        self.polygon_offsets.len()
    }
}

#[wasm_bindgen]
pub fn polygonize_buffers(
    coords: &[f64],  // wasm-bindgen automatically views Float64Array as &[f64]
    offsets: &[u32], // views Uint32Array as &[u32]
    node_input: bool,
    snap_grid_size: f64,
) -> Result<WasmPolygonResult, JsValue> {
    // Set panic hook for better error messages
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = node_input;
    polygonizer.snap_grid_size = snap_grid_size;

    // Process inputs
    // offsets are start indices of LineStrings in coords
    for i in 0..offsets.len() {
        let start = offsets[i] as usize;
        let end = if i < offsets.len() - 1 {
            offsets[i + 1] as usize
        } else {
            coords.len()
        };

        if start > coords.len() || end > coords.len() || start > end {
            return Err(JsValue::from_str(&format!(
                "Invalid offsets at index {}: start={}, end={}",
                i, start, end
            )));
        }

        let len = end - start;
        if !len.is_multiple_of(2) {
            return Err(JsValue::from_str(&format!(
                "Odd number of coordinates at index {}: len={}",
                i, len
            )));
        }

        let num_points = len / 2;
        let mut points = Vec::with_capacity(num_points);
        for j in 0..num_points {
            points.push(Coord {
                x: coords[start + 2 * j],
                y: coords[start + 2 * j + 1],
            });
        }

        polygonizer.add_geometry(geo::Geometry::LineString(LineString::new(points)));
    }

    polygonize_and_flatten(polygonizer)
}

#[wasm_bindgen]
pub fn polygonize_geoarrow(
    ipc_bytes: &[u8],
    node_input: bool,
    snap_grid_size: f64,
) -> Result<WasmPolygonResult, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = node_input;
    polygonizer.snap_grid_size = snap_grid_size;

    let mut reader = StreamReader::try_new(Cursor::new(ipc_bytes), None)
        .map_err(|e| JsValue::from_str(&format!("Invalid Arrow IPC stream: {e}")))?;

    let mut found_lines = false;
    for batch_result in &mut reader {
        let batch = batch_result
            .map_err(|e| JsValue::from_str(&format!("Failed reading Arrow IPC batch: {e}")))?;

        for (array, field) in batch.columns().iter().zip(batch.schema().fields().iter()) {
            let lines = match LineStringArray::try_from((array.as_ref(), field.as_ref())) {
                Ok(lines) => lines,
                Err(_) => continue,
            };

            found_lines = true;
            for scalar_result in lines.iter_values() {
                let line = scalar_result.map_err(|e| {
                    JsValue::from_str(&format!("Failed to decode GeoArrow LineString: {e}"))
                })?;
                polygonizer.add_geometry(geo::Geometry::LineString(line.to_line_string()));
            }
        }
    }

    if !found_lines {
        return Err(JsValue::from_str(
            "No GeoArrow LineString extension columns were found in the IPC stream",
        ));
    }

    polygonize_and_flatten(polygonizer)
}

fn polygonize_and_flatten(mut polygonizer: Polygonizer) -> Result<WasmPolygonResult, JsValue> {
    let result = polygonizer
        .polygonize()
        .map_err(|e| JsValue::from_str(&format!("Polygonization error: {}", e)))?;

    let mut flat_coords = Vec::new();
    let mut ring_offsets = Vec::new();
    let mut polygon_offsets = Vec::new();

    for poly in result.polygons {
        polygon_offsets.push(ring_offsets.len() as u32);
        let (exterior, interiors) = poly.into_inner();

        ring_offsets.push(flat_coords.len() as u32);
        for coord in exterior.0 {
            flat_coords.push(coord.x);
            flat_coords.push(coord.y);
        }

        for ring in interiors {
            ring_offsets.push(flat_coords.len() as u32);
            for coord in ring.0 {
                flat_coords.push(coord.x);
                flat_coords.push(coord.y);
            }
        }
    }

    Ok(WasmPolygonResult {
        coords: flat_coords,
        ring_offsets,
        polygon_offsets,
    })
}
