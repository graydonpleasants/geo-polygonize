mod error;

use arrow::compute::concat;
use arrow_ipc::reader::StreamReader;
use arrow_ipc::writer::FileWriter;
use geo_polygonize_core::arrow_api::{polygonize_arrow, PolygonizerOptions};
use geo_polygonize_core::{Coord3D, Line3D, Polygonizer};
use geoarrow::array::GeoArrowArray;
use geojson::{GeoJson, Geometry, Value};
use std::convert::TryInto;
use std::io::Cursor;
use std::str::FromStr;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

use crate::error::{from_polygonizer_error, to_js_error};

#[cfg(feature = "threads")]
pub use wasm_bindgen_rayon::init_thread_pool;

#[wasm_bindgen]
pub fn polygonize(
    geojson_str: &str,
    node_input: Option<bool>,
    snap_grid_size: Option<f64>,
    extract_only_polygonal: Option<bool>,
) -> Result<String, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let geojson = GeoJson::from_str(geojson_str)
        .map_err(|e| to_js_error("InvalidInput", format!("Invalid GeoJSON: {}", e)))?;

    let mut polygonizer = Polygonizer::new();
    if let Some(ni) = node_input {
        polygonizer.node_input = ni;
    }
    if let Some(sgs) = snap_grid_size {
        polygonizer.snap_grid_size = sgs;
    }
    if let Some(eop) = extract_only_polygonal {
        polygonizer.extract_only_polygonal = eop;
    }

    match geojson {
        GeoJson::FeatureCollection(fc) => {
            for feature in fc.features {
                if let Some(geom) = feature.geometry {
                    let geo_geom: geo::Geometry<f64> = geom.try_into().map_err(|e| {
                        to_js_error("ConversionError", format!("Conversion error: {}", e))
                    })?;
                    polygonizer.add_geometry(geo_geom);
                }
            }
        }
        GeoJson::Feature(f) => {
            if let Some(geom) = f.geometry {
                let geo_geom: geo::Geometry<f64> = geom.try_into().map_err(|e| {
                    to_js_error("ConversionError", format!("Conversion error: {}", e))
                })?;
                polygonizer.add_geometry(geo_geom);
            }
        }
        GeoJson::Geometry(g) => {
            let geo_geom: geo::Geometry<f64> = g
                .try_into()
                .map_err(|e| to_js_error("ConversionError", format!("Conversion error: {}", e)))?;
            polygonizer.add_geometry(geo_geom);
        }
    }

    let result = polygonizer.polygonize().map_err(from_polygonizer_error)?;

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
        return Err(to_js_error("InvalidInput", "stride must be 2 or 3"));
    }

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = node_input;
    polygonizer.snap_grid_size = snap_grid_size;

    let mut lines = Vec::new();

    for i in 0..offsets.len() {
        let start = offsets[i] as usize;
        let end = if i + 1 < offsets.len() {
            offsets[i + 1] as usize
        } else {
            coords.len() / stride as usize
        };

        if start > end {
            return Err(to_js_error(
                "InvalidInput",
                format!(
                "Invalid offsets: start offset ({}) is greater than end offset ({}) at index {}",
                start, end, i
            ),
            ));
        }
        if end * stride as usize > coords.len() {
            return Err(to_js_error("InvalidInput", format!(
                "Invalid offsets: calculated end offset {} exceeds coordinate capacity {} for stride {}",
                end * stride as usize, coords.len(), stride
            )));
        }

        for j in start..end.saturating_sub(1) {
            let idx = j * stride as usize;
            let jdx = (j + 1) * stride as usize;
            let z1 = if stride == 3 { coords[idx + 2] } else { 0.0 };
            let z2 = if stride == 3 { coords[jdx + 2] } else { 0.0 };

            lines.push(Line3D::new(
                Coord3D::new(coords[idx], coords[idx + 1], z1),
                Coord3D::new(coords[jdx], coords[jdx + 1], z2),
                0,
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
    extract_only_polygonal: bool,
) -> Result<Vec<u8>, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let reader = StreamReader::try_new(Cursor::new(ipc_bytes), None)
        .map_err(|e| to_js_error("InvalidInput", format!("Invalid Arrow IPC stream: {e}")))?;

    let schema = reader.schema();

    // Find geometry column
    let mut geom_col_idx = None;
    for (i, field) in schema.fields().iter().enumerate() {
        if let Some(metadata) = field.metadata().get("ARROW:extension:name") {
            if metadata.starts_with("ogc.geoarrow.linestring") {
                geom_col_idx = Some(i);
                break;
            }
        }
    }

    let geom_col_idx = geom_col_idx
        .ok_or_else(|| to_js_error("InvalidInput", "No GeoArrow LineString column found"))?;
    let field = schema.field(geom_col_idx).clone();

    // Collect batches
    let mut arrays = Vec::new();
    for batch_result in reader {
        let batch = batch_result
            .map_err(|e| to_js_error("ArrowError", format!("Failed reading batch: {e}")))?;
        arrays.push(batch.column(geom_col_idx).clone());
    }

    if arrays.is_empty() {
        return Err(to_js_error("InvalidInput", "No data found"));
    }

    let arrays_ref: Vec<&dyn arrow::array::Array> = arrays.iter().map(|a| a.as_ref()).collect();
    let combined_array = concat(&arrays_ref)
        .map_err(|e| to_js_error("ArrowError", format!("Failed to concat arrays: {e}")))?;

    let options = PolygonizerOptions {
        node_input,
        snap_grid_size,
        extract_only_polygonal,
    };

    let result_array = polygonize_arrow(combined_array.as_ref(), &field, options)
        .map_err(|e| to_js_error("PolygonizationError", format!("Polygonization error: {e}")))?;

    // Serialize result to IPC
    let mut output_buffer = Vec::new();
    {
        // Use data_type().clone().into() to get arrow DataType
        let field =
            arrow::datatypes::Field::new("geometry", result_array.data_type().clone().into(), true);
        let schema = Arc::new(arrow::datatypes::Schema::new(vec![field]));

        let mut writer = FileWriter::try_new(&mut output_buffer, &schema)
            .map_err(|e| to_js_error("ArrowError", format!("Failed to create IPC writer: {e}")))?;

        let batch = arrow::record_batch::RecordBatch::try_new(
            schema.clone(),
            vec![result_array.into_array_ref()],
        )
        .map_err(|e| to_js_error("ArrowError", format!("Failed to create RecordBatch: {e}")))?;

        writer
            .write(&batch)
            .map_err(|e| to_js_error("ArrowError", format!("Failed to write batch: {e}")))?;
        writer
            .finish()
            .map_err(|e| to_js_error("ArrowError", format!("Failed to finish writer: {e}")))?;
    }

    Ok(output_buffer)
}

fn polygonize_and_flatten(
    mut polygonizer: Polygonizer,
    stride: u8,
) -> Result<WasmPolygonResult, JsValue> {
    let result = polygonizer.polygonize().map_err(from_polygonizer_error)?;

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
