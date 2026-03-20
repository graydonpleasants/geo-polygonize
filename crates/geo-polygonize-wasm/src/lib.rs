mod error;

use arrow::compute::concat;
use arrow_ipc::reader::StreamReader;
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

#[wasm_bindgen(js_name = polygonizeWithOptions)]
pub fn polygonize_with_options_js(
    geojson_str: &str,
    options_val: JsValue,
) -> Result<String, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let options: geo_polygonize_core::options::PolygonizerOptions =
        serde_wasm_bindgen::from_value(options_val).map_err(|e| {
            to_js_error(
                "InvalidArgumentType",
                format!("Failed to parse options: {}", e),
            )
        })?;

    let geojson = GeoJson::from_str(geojson_str)
        .map_err(|e| to_js_error("InvalidArgumentType", format!("Invalid GeoJSON: {}", e)))?;

    let mut polygonizer = Polygonizer::with_options(options);

    match geojson {
        GeoJson::FeatureCollection(fc) => {
            for feature in fc.features {
                if let Some(geom) = feature.geometry {
                    let geo_geom: geo::Geometry<f64> = geom.try_into().map_err(|e| {
                        to_js_error("InvalidGeometry", format!("Conversion error: {}", e))
                    })?;
                    polygonizer.add_geometry(geo_geom);
                }
            }
        }
        GeoJson::Feature(f) => {
            if let Some(geom) = f.geometry {
                let geo_geom: geo::Geometry<f64> = geom.try_into().map_err(|e| {
                    to_js_error("InvalidGeometry", format!("Conversion error: {}", e))
                })?;
                polygonizer.add_geometry(geo_geom);
            }
        }
        GeoJson::Geometry(g) => {
            let geo_geom: geo::Geometry<f64> = g
                .try_into()
                .map_err(|e| to_js_error("InvalidGeometry", format!("Conversion error: {}", e)))?;
            polygonizer.add_geometry(geo_geom);
        }
    }

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| polygonizer.polygonize()))
            .unwrap_or_else(|_| {
                Err(geo_polygonize_core::error::PolygonizeError::Panic(
                    "Panic occurred in Rust core".to_string(),
                ))
            })
            .map_err(from_polygonizer_error)?;

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
pub fn polygonize(
    geojson_str: &str,
    node_input: Option<bool>,
    snap_grid_size: Option<f64>,
    extract_only_polygonal: Option<bool>,
    report_mode: Option<bool>,
) -> Result<String, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let geojson = GeoJson::from_str(geojson_str)
        .map_err(|e| to_js_error("InvalidArgumentType", format!("Invalid GeoJSON: {}", e)))?;

    let mut options = geo_polygonize_core::options::PolygonizerOptions::default();
    if let Some(ni) = node_input {
        options.node_input = ni;
    }
    if let Some(sgs) = snap_grid_size {
        options.snap_grid_size = sgs;
    }
    if let Some(eop) = extract_only_polygonal {
        options.extract_only_polygonal = eop;
    }
    if let Some(rm) = report_mode {
        options.diagnostics.enabled = rm;
        options.diagnostics.report_mode = rm;
    }
    let mut polygonizer = Polygonizer::with_options(options);

    match geojson {
        GeoJson::FeatureCollection(fc) => {
            for feature in fc.features {
                if let Some(geom) = feature.geometry {
                    let geo_geom: geo::Geometry<f64> = geom.try_into().map_err(|e| {
                        to_js_error("InvalidGeometry", format!("Conversion error: {}", e))
                    })?;
                    polygonizer.add_geometry(geo_geom);
                }
            }
        }
        GeoJson::Feature(f) => {
            if let Some(geom) = f.geometry {
                let geo_geom: geo::Geometry<f64> = geom.try_into().map_err(|e| {
                    to_js_error("InvalidGeometry", format!("Conversion error: {}", e))
                })?;
                polygonizer.add_geometry(geo_geom);
            }
        }
        GeoJson::Geometry(g) => {
            let geo_geom: geo::Geometry<f64> = g
                .try_into()
                .map_err(|e| to_js_error("InvalidGeometry", format!("Conversion error: {}", e)))?;
            polygonizer.add_geometry(geo_geom);
        }
    }

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| polygonizer.polygonize()))
            .unwrap_or_else(|_| {
                Err(geo_polygonize_core::error::PolygonizeError::Panic(
                    "Panic occurred in Rust core".to_string(),
                ))
            })
            .map_err(from_polygonizer_error)?;

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
    flat_line_ids: Vec<u32>,
    stride: u8,
    provenance: JsValue,
    diagnostics: JsValue,
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

    pub fn flat_line_ids_ptr(&self) -> *const u32 {
        self.flat_line_ids.as_ptr()
    }
    pub fn flat_line_ids_len(&self) -> usize {
        self.flat_line_ids.len()
    }
    pub fn stride(&self) -> u8 {
        self.stride
    }

    #[wasm_bindgen(getter)]
    pub fn provenance(&self) -> JsValue {
        self.provenance.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn diagnostics(&self) -> JsValue {
        self.diagnostics.clone()
    }
}

#[wasm_bindgen(js_name = polygonizeWithOptionsBuffer)]
pub fn polygonize_with_options_buffer_js(
    coords: &[f64],
    offsets: &[u32],
    stride: u8,
    options_val: JsValue,
    line_ids: Option<Vec<u32>>,
) -> Result<WasmPolygonResult, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let options: geo_polygonize_core::options::PolygonizerOptions =
        serde_wasm_bindgen::from_value(options_val).map_err(|e| {
            to_js_error(
                "InvalidArgumentType",
                format!("Failed to parse options: {}", e),
            )
        })?;

    if stride != 2 && stride != 3 {
        return Err(to_js_error("InvalidArgumentType", "stride must be 2 or 3"));
    }

    if let Some(ref ids) = line_ids {
        if !offsets.is_empty() && ids.len() != offsets.len() {
            return Err(to_js_error(
                "InvalidBufferShape",
                format!(
                    "line_ids length {} does not match line count {}",
                    ids.len(),
                    offsets.len()
                ),
            ));
        }
    }

    let mut polygonizer = Polygonizer::with_options(options);

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

        let line_id = if let Some(ref ids) = line_ids {
            if i < ids.len() {
                ids[i]
            } else {
                0
            }
        } else {
            0
        };
        if end * stride as usize > coords.len() {
            return Err(to_js_error("InvalidArgumentType", format!(
                "Invalid offsets: calculated end offset {} exceeds coordinate capacity {} for stride {}",
                end * stride as usize, coords.len(), stride
            )));
        }

        for j in start..end.saturating_sub(1) {
            let idx = j * stride as usize;
            let jdx = (j + 1) * stride as usize;
            let z1 = if stride == 3 { coords[idx + 2] } else { 0.0 };
            let z2 = if stride == 3 { coords[jdx + 2] } else { 0.0 };

            if !coords[idx].is_finite()
                || !coords[idx + 1].is_finite()
                || !z1.is_finite()
                || !coords[jdx].is_finite()
                || !coords[jdx + 1].is_finite()
                || !z2.is_finite()
            {
                return Err(to_js_error(
                    "InvalidGeometry",
                    "NaN or Inf coordinates detected in buffers",
                ));
            }

            lines.push(Line3D::new(
                Coord3D::new(coords[idx], coords[idx + 1], z1),
                Coord3D::new(coords[jdx], coords[jdx + 1], z2),
                line_id,
            ));
        }
    }

    polygonizer.add_lines(lines);

    polygonize_and_flatten(polygonizer, stride)
}

#[wasm_bindgen]
pub fn polygonize_buffers(
    coords: &[f64],
    offsets: &[u32],
    stride: u8,
    node_input: bool,
    snap_grid_size: f64,
    line_ids: Option<Vec<u32>>,
) -> Result<WasmPolygonResult, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    if stride != 2 && stride != 3 {
        return Err(to_js_error("InvalidArgumentType", "stride must be 2 or 3"));
    }

    let options = geo_polygonize_core::options::PolygonizerOptions {
        node_input,
        snap_grid_size,
        ..Default::default()
    };
    if let Some(ref ids) = line_ids {
        if !offsets.is_empty() && ids.len() != offsets.len() {
            return Err(to_js_error(
                "InvalidBufferShape",
                format!(
                    "line_ids length {} does not match line count {}",
                    ids.len(),
                    offsets.len()
                ),
            ));
        }
    }

    let mut polygonizer = Polygonizer::with_options(options);

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

        let line_id = if let Some(ref ids) = line_ids {
            if i < ids.len() {
                ids[i]
            } else {
                0
            }
        } else {
            0
        };
        if end * stride as usize > coords.len() {
            return Err(to_js_error("InvalidArgumentType", format!(
                "Invalid offsets: calculated end offset {} exceeds coordinate capacity {} for stride {}",
                end * stride as usize, coords.len(), stride
            )));
        }

        for j in start..end.saturating_sub(1) {
            let idx = j * stride as usize;
            let jdx = (j + 1) * stride as usize;
            let z1 = if stride == 3 { coords[idx + 2] } else { 0.0 };
            let z2 = if stride == 3 { coords[jdx + 2] } else { 0.0 };

            if !coords[idx].is_finite()
                || !coords[idx + 1].is_finite()
                || !z1.is_finite()
                || !coords[jdx].is_finite()
                || !coords[jdx + 1].is_finite()
                || !z2.is_finite()
            {
                return Err(to_js_error(
                    "InvalidGeometry",
                    "NaN or Inf coordinates detected in buffers",
                ));
            }

            lines.push(Line3D::new(
                Coord3D::new(coords[idx], coords[idx + 1], z1),
                Coord3D::new(coords[jdx], coords[jdx + 1], z2),
                line_id,
            ));
        }
    }

    polygonizer.add_lines(lines);

    polygonize_and_flatten(polygonizer, stride)
}

#[wasm_bindgen(js_name = polygonizeGeoArrowWithOptions)]
pub fn polygonize_geoarrow_with_options_js(
    ipc_bytes: &[u8],
    options_val: JsValue,
) -> Result<Vec<u8>, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let options: geo_polygonize_core::options::PolygonizerOptions =
        serde_wasm_bindgen::from_value(options_val).map_err(|e| {
            to_js_error(
                "InvalidArgumentType",
                format!("Failed to parse options: {}", e),
            )
        })?;

    polygonize_geoarrow_internal(ipc_bytes, options)
}

#[wasm_bindgen]
pub fn polygonize_geoarrow(
    ipc_bytes: &[u8],
    node_input: bool,
    snap_grid_size: f64,
    extract_only_polygonal: bool,
) -> Result<Vec<u8>, JsValue> {
    let options = PolygonizerOptions {
        node_input,
        snap_grid_size,
        extract_only_polygonal,
        ..Default::default()
    };
    polygonize_geoarrow_internal(ipc_bytes, options)
}

fn polygonize_geoarrow_internal(
    ipc_bytes: &[u8],
    options: PolygonizerOptions,
) -> Result<Vec<u8>, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let reader = StreamReader::try_new(Cursor::new(ipc_bytes), None).map_err(|e| {
        to_js_error(
            "InvalidArgumentType",
            format!("Invalid Arrow IPC stream: {e}"),
        )
    })?;

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
        .ok_or_else(|| to_js_error("InvalidArgumentType", "No GeoArrow LineString column found"))?;
    let field = schema.field(geom_col_idx).clone();

    // Collect batches
    let mut arrays = Vec::new();
    for batch_result in reader {
        let batch = batch_result
            .map_err(|e| to_js_error("ArrowError", format!("Failed reading batch: {e}")))?;
        arrays.push(batch.column(geom_col_idx).clone());
    }

    if arrays.is_empty() {
        return Err(to_js_error("InvalidArgumentType", "No data found"));
    }

    let arrays_ref: Vec<&dyn arrow::array::Array> = arrays.iter().map(|a| a.as_ref()).collect();
    let combined_array = concat(&arrays_ref)
        .map_err(|e| to_js_error("ArrowError", format!("Failed to concat arrays: {e}")))?;

    let result_array = polygonize_arrow(combined_array.as_ref(), &field, options)
        .map_err(|e| to_js_error("TopologyFailure", format!("Polygonization error: {e}")))?;

    // Serialize result to IPC
    let mut output_buffer = Vec::new();
    {
        // Use data_type().clone().into() to get arrow DataType
        let field =
            arrow::datatypes::Field::new("geometry", result_array.data_type().clone().into(), true);
        let schema = Arc::new(arrow::datatypes::Schema::new(vec![field]));

        let mut writer = arrow_ipc::writer::StreamWriter::try_new(&mut output_buffer, &schema)
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
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| polygonizer.polygonize()))
            .unwrap_or_else(|_| {
                Err(geo_polygonize_core::error::PolygonizeError::Panic(
                    "Panic occurred in Rust core".to_string(),
                ))
            })
            .map_err(from_polygonizer_error)?;

    let mut flat_coords = Vec::new();
    let mut ring_offsets = Vec::new();
    let mut polygon_offsets = Vec::new();
    let mut flat_line_ids = Vec::new();
    let mut provenances = Vec::new();

    for poly in result.polygons {
        provenances.push(poly.provenance);
        polygon_offsets.push(ring_offsets.len() as u32);

        let exterior = poly.exterior;
        let interiors = poly.interiors;

        ring_offsets.push((flat_coords.len() / stride as usize) as u32);
        for (k, coord) in exterior.iter().enumerate() {
            flat_coords.push(coord.x);
            flat_coords.push(coord.y);
            if stride == 3 {
                flat_coords.push(coord.z);
            }
            if k < poly.exterior_ids.len() {
                flat_line_ids.push(poly.exterior_ids[k]);
            } else {
                flat_line_ids.push(0);
            }
        }

        for (h_idx, ring) in interiors.iter().enumerate() {
            ring_offsets.push((flat_coords.len() / stride as usize) as u32);
            for (k, coord) in ring.iter().enumerate() {
                flat_coords.push(coord.x);
                flat_coords.push(coord.y);
                if stride == 3 {
                    flat_coords.push(coord.z);
                }
                if k < poly.interiors_ids[h_idx].len() {
                    flat_line_ids.push(poly.interiors_ids[h_idx][k]);
                } else {
                    flat_line_ids.push(0);
                }
            }
        }
    }

    let js_provenance = if provenances.is_empty() {
        JsValue::NULL
    } else {
        serde_wasm_bindgen::to_value(&provenances).unwrap_or(JsValue::NULL)
    };

    let js_diagnostics = if let Some(ref diag) = result.diagnostics {
        serde_wasm_bindgen::to_value(diag).unwrap_or(JsValue::NULL)
    } else {
        JsValue::NULL
    };

    Ok(WasmPolygonResult {
        coords: flat_coords,
        ring_offsets,
        polygon_offsets,
        flat_line_ids,
        stride,
        provenance: js_provenance,
        diagnostics: js_diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow::array::Array;
    use arrow::datatypes::{Field, Schema};
    use arrow::record_batch::RecordBatch;
    use arrow_ipc::writer::StreamWriter;
    use geoarrow::array::PolygonArray;
    use std::sync::Arc;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_polygonize_geoarrow_empty() {
        let result = polygonize_geoarrow(b"invalid data", false, 0.0, false);
        assert!(result.is_err());
    }

    fn generate_valid_geoarrow_ipc() -> Vec<u8> {
        let coord0 = geo::Coord { x: 0.0, y: 0.0 };
        let coord1 = geo::Coord { x: 10.0, y: 0.0 };
        let coord2 = geo::Coord { x: 10.0, y: 10.0 };
        let coord3 = geo::Coord { x: 0.0, y: 10.0 };

        let line_string = geo::LineString::new(vec![coord0, coord1, coord2, coord3, coord0]);

        use geoarrow::array::LineStringBuilder;
        use geoarrow::datatypes::{Dimension, LineStringType};

        let typ = LineStringType::new(Dimension::XY, Arc::new(Default::default()));
        let mut builder = LineStringBuilder::new(typ);
        builder.push_line_string(Some(&line_string)).unwrap();
        let input_array = builder.finish();

        use geoarrow::array::GeoArrowArray;
        let arrow_array = input_array.into_array_ref();

        let mut field = Field::new("geometry", arrow_array.data_type().clone(), true);
        field.set_metadata(
            [(
                "ARROW:extension:name".to_string(),
                "ogc.geoarrow.linestring".to_string(),
            )]
            .into(),
        );

        let schema = Arc::new(Schema::new(vec![field]));
        let batch = RecordBatch::try_new(schema.clone(), vec![arrow_array]).unwrap();

        let mut buf = Vec::new();
        {
            let mut writer = StreamWriter::try_new(&mut buf, &schema).unwrap();
            writer.write(&batch).unwrap();
            writer.finish().unwrap();
        }
        buf
    }

    #[wasm_bindgen_test]
    fn test_polygonize_geoarrow_valid() {
        let ipc_bytes = generate_valid_geoarrow_ipc();
        let result = polygonize_geoarrow(&ipc_bytes, false, 0.0, false);

        // Ensure success without unwrapping directly which can panic
        let out_bytes = result.unwrap_or_else(|e| {
            panic!(
                "Error from polygonize_geoarrow: {:?}",
                js_sys::JSON::stringify(&e).unwrap().as_string()
            );
        });

        assert!(!out_bytes.is_empty());

        let reader =
            arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(&out_bytes), None)
                .unwrap();

        let schema = reader.schema();
        let geom_field = schema.field(0);

        let mut batches = vec![];
        for batch in reader {
            batches.push(batch.unwrap());
        }

        assert_eq!(batches.len(), 1);
        let batch = &batches[0];
        let geom_col = batch.column(0);

        let mut field = geom_field.clone();

        // Ensure the arrow result can be successfully converted to a geoarrow PolygonArray
        field.set_metadata(
            [(
                "ARROW:extension:name".to_string(),
                "geoarrow.polygon".to_string(),
            )]
            .into(),
        );

        use std::convert::TryFrom;
        let poly_array = PolygonArray::try_from((geom_col.as_ref(), &field)).unwrap();
        assert_eq!(poly_array.len(), 1);
    }
}
