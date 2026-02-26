use arrow_ipc::reader::StreamReader;
use geo::{Coord, LineString};
use geo_polygonize_core::{Coord3D, Line3D, Polygonizer};
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
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let geojson = GeoJson::from_str(geojson_str)
        .map_err(|e| JsValue::from_str(&format!("Invalid GeoJSON: {}", e)))?;

    let mut polygonizer = Polygonizer::new();

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

    let geometries: Vec<Geometry> = result
        .polygons
        .into_iter()
        .map(|p| {
            let exterior: Vec<Vec<f64>> = p.exterior.iter().map(|c| vec![c.x, c.y, c.z]).collect();
            let mut rings = vec![exterior];
            for hole in p.interiors {
                let hole_ring: Vec<Vec<f64>> = hole.iter().map(|c| vec![c.x, c.y, c.z]).collect();
                rings.push(hole_ring);
            }
            Geometry::new(Value::Polygon(rings))
        })
        .collect();

    let features = geometries
        .into_iter()
        .map(|geom| geojson::Feature {
            bbox: None,
            geometry: Some(geom),
            id: None,
            properties: None,
            foreign_members: None,
        })
        .collect();

    Ok(GeoJson::FeatureCollection(geojson::FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    })
    .to_string())
}

#[wasm_bindgen]
pub struct WasmPolygonResult {
    coords: Vec<f64>,
    ring_offsets: Vec<u32>,
    polygon_offsets: Vec<u32>,
    stride: u8,
}

#[wasm_bindgen]
impl WasmPolygonResult {
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
    pub fn stride(&self) -> u8 {
        self.stride
    }
}

#[wasm_bindgen]
pub fn polygonize_buffers(
    coords: &[f64],
    offsets: &[u32],
    stride: u8,
    node_input: bool,
    snap_grid_size: f64,
) -> Result<WasmPolygonResult, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    if stride != 2 && stride != 3 {
        return Err(JsValue::from_str("stride must be 2 or 3"));
    }

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = node_input;
    polygonizer.snap_grid_size = snap_grid_size;

    let mut lines = Vec::new();

    for i in 0..offsets.len() {
        let start = offsets[i] as usize;
        let end = if i < offsets.len() - 1 {
            offsets[i + 1] as usize
        } else {
            coords.len() / stride as usize
        };

        if start > end || end * stride as usize > coords.len() {
            return Err(JsValue::from_str("Invalid offsets"));
        }

        for j in start..end.saturating_sub(1) {
            let idx = j * stride as usize;
            let jdx = (j + 1) * stride as usize;
            let z1 = if stride == 3 { coords[idx + 2] } else { 0.0 };
            let z2 = if stride == 3 { coords[jdx + 2] } else { 0.0 };

            lines.push(Line3D::new(
                Coord3D::new(coords[idx], coords[idx + 1], z1),
                Coord3D::new(coords[jdx], coords[jdx + 1], z2),
            ));
        }
    }

    polygonizer.add_lines(lines);

    polygonize_and_flatten(polygonizer, stride)
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

    polygonize_and_flatten(polygonizer, 2)
}

fn polygonize_and_flatten(
    mut polygonizer: Polygonizer,
    stride: u8,
) -> Result<WasmPolygonResult, JsValue> {
    let result = polygonizer
        .polygonize()
        .map_err(|e| JsValue::from_str(&format!("Polygonization error: {}", e)))?;

    let mut flat_coords = Vec::new();
    let mut ring_offsets = Vec::new();
    let mut polygon_offsets = Vec::new();

    for poly in result.polygons {
        polygon_offsets.push(ring_offsets.len() as u32);

        let exterior = poly.exterior;
        let interiors = poly.interiors;

        ring_offsets.push((flat_coords.len() / stride as usize) as u32);
        for coord in exterior {
            flat_coords.push(coord.x);
            flat_coords.push(coord.y);
            if stride == 3 {
                flat_coords.push(coord.z);
            }
        }

        for ring in interiors {
            ring_offsets.push((flat_coords.len() / stride as usize) as u32);
            for coord in ring {
                flat_coords.push(coord.x);
                flat_coords.push(coord.y);
                if stride == 3 {
                    flat_coords.push(coord.z);
                }
            }
        }
    }

    Ok(WasmPolygonResult {
        coords: flat_coords,
        ring_offsets,
        polygon_offsets,
        stride,
    })
}
