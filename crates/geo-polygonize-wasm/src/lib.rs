mod buffer;
mod error;

use arrow::compute::concat;
use arrow_ipc::reader::StreamReader;
pub use buffer::parse_buffer_lines;
use geo_polygonize_arrow::{polygonize_arrow, PolygonizerOptions};
use geo_polygonize_core::tiling::TiledPolygonizer;
use geo_polygonize_core::trace::TraceLevelV1;
use geo_polygonize_core::{
    polygonize as polygonize_lines, Line3D, PolygonizeError, Polygonizer, PolygonizerResult,
    TopologyFingerprintV1,
};
use geoarrow::array::GeoArrowArray;
use geojson::{GeoJson, Geometry, Value};
use std::convert::TryInto;
use std::io::Cursor;
use std::str::FromStr;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

use crate::error::{from_polygonizer_error, to_js_error};

type RouterInput = (Vec<geo::Geometry<f64>>, geo::Rect<f64>);

#[cfg(feature = "threads")]
pub use wasm_bindgen_rayon::init_thread_pool;

#[wasm_bindgen(js_name = polygonizeWithOptions)]
/// Polygonizes a GeoJSON FeatureCollection using the canonical `PolygonizerOptions`.
///
/// This compatibility entry point returns polygons only. Use
/// [`polygonize_report_with_options_js`] when dangles, cut edges, invalid rings,
/// provenance, diagnostics, and exact coordinate encoding are required.
pub fn polygonize_with_options_js(
    geojson_str: &str,
    options_val: JsValue,
) -> Result<String, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let options: geo_polygonize_core::PolygonizerOptions =
        serde_wasm_bindgen::from_value(options_val).map_err(|e| {
            to_js_error(
                "InvalidArgumentType",
                format!("Failed to parse options: {}", e),
            )
        })?;

    let result = polygonize_geojson(geojson_str, &options)?;

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

#[wasm_bindgen(js_name = polygonizeFingerprintWithOptions)]
/// Returns the exact retained topology contract for a GeoJSON canonical-options call.
pub fn polygonize_fingerprint_with_options_js(
    geojson_str: &str,
    options_val: JsValue,
) -> Result<String, JsValue> {
    let options = serde_wasm_bindgen::from_value(options_val).map_err(|e| {
        to_js_error(
            "InvalidArgumentType",
            format!("Failed to parse options: {e}"),
        )
    })?;
    let result = polygonize_geojson(geojson_str, &options)?;
    serde_json::to_string(
        &TopologyFingerprintV1::try_from_result(&result, &options)
            .map_err(from_polygonizer_error)?,
    )
    .map_err(|e| to_js_error("InternalInvariantViolation", e))
}

#[wasm_bindgen(js_name = polygonizeReportWithOptions)]
/// Returns the complete versioned topology report for a GeoJSON canonical-options call.
///
/// The report is the JSON serialization of `TopologyFingerprintV1`; it is the
/// stable cross-binding success contract, not a polygon-only GeoJSON projection.
pub fn polygonize_report_with_options_js(
    geojson_str: &str,
    options_val: JsValue,
) -> Result<String, JsValue> {
    polygonize_fingerprint_with_options_js(geojson_str, options_val)
}

#[wasm_bindgen(js_name = polygonizeTraceWithOptions)]
/// Returns a versioned topology report and bounded physical-pipeline trace.
pub fn polygonize_trace_with_options_js(
    geojson_str: &str,
    options_val: JsValue,
    trace_level: &str,
    byte_limit: f64,
) -> Result<String, JsValue> {
    let options: geo_polygonize_core::PolygonizerOptions =
        serde_wasm_bindgen::from_value(options_val).map_err(|e| {
            to_js_error(
                "InvalidArgumentType",
                format!("Failed to parse options: {e}"),
            )
        })?;
    let level = match trace_level {
        "summary" => TraceLevelV1::Summary,
        "noding" => TraceLevelV1::Noding,
        "graph" => TraceLevelV1::Graph,
        "rings" => TraceLevelV1::Rings,
        "full" => TraceLevelV1::Full,
        actual => {
            return Err(from_polygonizer_error(
                PolygonizeError::InvalidArgumentType {
                    field: "trace_level".to_string(),
                    expected: "summary, noding, graph, rings, or full".to_string(),
                    actual: actual.to_string(),
                },
            ));
        }
    };
    if !byte_limit.is_finite()
        || byte_limit < 0.0
        || byte_limit > f64::from(u32::MAX)
        || byte_limit.fract() != 0.0
    {
        return Err(from_polygonizer_error(
            PolygonizeError::InvalidArgumentType {
                field: "byte_limit".to_string(),
                expected: "an integer from 0 through u32::MAX".to_string(),
                actual: byte_limit.to_string(),
            },
        ));
    }
    let byte_limit = usize::try_from(byte_limit as u32).map_err(|_| {
        from_polygonizer_error(PolygonizeError::InvalidArgumentType {
            field: "byte_limit".to_string(),
            expected: "a platform-sized unsigned integer".to_string(),
            actual: byte_limit.to_string(),
        })
    })?;
    let mut polygonizer = polygonizer_from_geojson(geojson_str, &options)?;
    let traced = polygonizer
        .polygonize_with_trace(level, byte_limit)
        .map_err(from_polygonizer_error)?;
    let topology = TopologyFingerprintV1::try_from_result(&traced.result, &options)
        .map_err(from_polygonizer_error)?;

    serde_json::to_string(&serde_json::json!({
        "schema_version": 1,
        "topology": topology,
        "trace": traced.trace,
    }))
    .map_err(|e| to_js_error("InternalInvariantViolation", e))
}

fn polygonize_geojson(
    geojson_str: &str,
    options: &geo_polygonize_core::PolygonizerOptions,
) -> Result<PolygonizerResult, JsValue> {
    polygonizer_from_geojson(geojson_str, options)?
        .polygonize()
        .map_err(from_polygonizer_error)
}

fn polygonizer_from_geojson(
    geojson_str: &str,
    options: &geo_polygonize_core::PolygonizerOptions,
) -> Result<Polygonizer, JsValue> {
    let mut polygonizer = Polygonizer::with_options(options.clone());
    for geometry in geometries_from_geojson(geojson_str)? {
        polygonizer.add_geometry(geometry);
    }
    Ok(polygonizer)
}

fn geometries_from_geojson(geojson_str: &str) -> Result<Vec<geo::Geometry<f64>>, JsValue> {
    let geojson = GeoJson::from_str(geojson_str)
        .map_err(|e| to_js_error("InvalidArgumentType", format!("Invalid GeoJSON: {e}")))?;
    let convert = |geometry: geojson::Geometry| {
        geometry
            .try_into()
            .map_err(|e| to_js_error("InvalidGeometry", format!("Conversion error: {e}")))
    };
    match geojson {
        GeoJson::FeatureCollection(collection) => collection
            .features
            .into_iter()
            .filter_map(|feature| feature.geometry)
            .map(convert)
            .collect(),
        GeoJson::Feature(feature) => feature.geometry.into_iter().map(convert).collect(),
        GeoJson::Geometry(geometry) => Ok(vec![convert(geometry)?]),
    }
}

fn partition_router_input(geojson_str: &str) -> Result<RouterInput, JsValue> {
    use geo::BoundingRect;

    let geometries = geometries_from_geojson(geojson_str)?;
    let mut bounds = geometries
        .iter()
        .filter_map(|geometry| geometry.bounding_rect());
    let Some(first) = bounds.next() else {
        return Err(to_js_error(
            "InvalidGeometry",
            "Partition router benchmark requires non-empty geometry",
        ));
    };
    let bbox = bounds.fold(first, |bbox, next| {
        geo::Rect::new(
            geo::Coord {
                x: bbox.min().x.min(next.min().x),
                y: bbox.min().y.min(next.min().y),
            },
            geo::Coord {
                x: bbox.max().x.max(next.max().x),
                y: bbox.max().y.max(next.max().y),
            },
        )
    });
    Ok((geometries, bbox))
}

fn with_partition_router<T>(
    geojson_str: &str,
    tile_size: f64,
    buffer: f64,
    options_val: JsValue,
    run: impl FnOnce(&TiledPolygonizer<'_>) -> geo_polygonize_core::Result<T>,
) -> Result<T, JsValue> {
    let options = serde_wasm_bindgen::from_value(options_val).map_err(|e| {
        to_js_error(
            "InvalidArgumentType",
            format!("Failed to parse options: {e}"),
        )
    })?;
    let (geometries, bbox) = partition_router_input(geojson_str)?;
    let mut tiled = TiledPolygonizer::new(bbox, tile_size)
        .with_buffer(buffer)
        .with_options(options);
    for geometry in &geometries {
        tiled.add_geometry(geometry);
    }
    run(&tiled).map_err(from_polygonizer_error)
}

#[wasm_bindgen(js_name = __partitionRouterComparison)]
#[doc(hidden)]
pub fn partition_router_comparison_js(
    geojson_str: &str,
    tile_size: f64,
    buffer: f64,
    options_val: JsValue,
) -> Result<String, JsValue> {
    let comparison = with_partition_router(geojson_str, tile_size, buffer, options_val, |tiled| {
        tiled.partition_router_comparison()
    })?;
    serde_json::to_string(&comparison).map_err(|e| to_js_error("InternalInvariantViolation", e))
}

#[wasm_bindgen(js_name = __benchmarkPartitionRouter)]
#[doc(hidden)]
pub fn benchmark_partition_router_js(
    geojson_str: &str,
    tile_size: f64,
    buffer: f64,
    options_val: JsValue,
    warmup_iterations: u32,
    samples: u32,
) -> Result<String, JsValue> {
    if samples == 0 {
        return Err(to_js_error(
            "InvalidArgumentType",
            "Partition router samples must be greater than zero",
        ));
    }
    if warmup_iterations.saturating_add(samples) > 10_000 {
        return Err(to_js_error(
            "ResourceLimitExceeded",
            "Partition router warmups and samples exceed 10000",
        ));
    }
    let benchmark = with_partition_router(geojson_str, tile_size, buffer, options_val, |tiled| {
        for _ in 0..warmup_iterations {
            tiled.benchmark_partition_router()?;
        }
        let expected = tiled.benchmark_partition_router()?;
        let mut samples_ms = Vec::with_capacity(samples as usize);
        for _ in 0..samples {
            let started = js_sys::Date::now();
            let work = tiled.benchmark_partition_router()?;
            samples_ms.push(js_sys::Date::now() - started);
            if work != expected {
                return Err(PolygonizeError::InternalInvariantViolation {
                    reason: "partition router work changed between Wasm samples".to_string(),
                });
            }
        }
        Ok(serde_json::json!({
            "schema_version": 1,
            "samples_ms": samples_ms,
            "router_work": expected,
        }))
    })?;
    serde_json::to_string(&benchmark).map_err(|e| to_js_error("InternalInvariantViolation", e))
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

    let mut options = geo_polygonize_core::PolygonizerOptions::default();
    if let Some(ni) = node_input {
        options.node_input = ni;
    }
    if options.node_input {
        options.precision_model = snap_grid_size.map_or(
            geo_polygonize_core::PrecisionModel::FixedGrid { grid_size: 1e-10 },
            geo_polygonize_core::PrecisionModel::from_grid_size,
        );
    }
    if let Some(eop) = extract_only_polygonal {
        options.extract_only_polygonal = eop;
    }
    if let Some(rm) = report_mode {
        options.diagnostics.enabled = rm;
        options.diagnostics.report_mode = rm;
    }
    let mut polygonizer = Polygonizer::with_options(options);
    for geometry in geometries_from_geojson(geojson_str)? {
        polygonizer.add_geometry(geometry);
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
    flat_line_ids: Vec<u32>,
    stride: u8,
    provenance: JsValue,
    dangles: JsValue,
    cut_edges: JsValue,
    invalid_rings: JsValue,
    diagnostics: JsValue,
    topology_fingerprint: JsValue,
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
    pub fn dangles(&self) -> JsValue {
        self.dangles.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn cut_edges(&self) -> JsValue {
        self.cut_edges.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn invalid_rings(&self) -> JsValue {
        self.invalid_rings.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn diagnostics(&self) -> JsValue {
        self.diagnostics.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn topology_fingerprint(&self) -> JsValue {
        self.topology_fingerprint.clone()
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

    let options: geo_polygonize_core::PolygonizerOptions =
        serde_wasm_bindgen::from_value(options_val).map_err(|e| {
            to_js_error(
                "InvalidArgumentType",
                format!("Failed to parse options: {}", e),
            )
        })?;

    let lines = parse_buffer_lines(coords, offsets, stride, line_ids.as_deref())
        .map_err(|error| to_js_error(error.name, error.message))?;

    polygonize_and_flatten(lines, options, stride)
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

    let options = geo_polygonize_core::PolygonizerOptions {
        node_input,
        precision_model: if node_input {
            geo_polygonize_core::PrecisionModel::from_grid_size(snap_grid_size)
        } else {
            geo_polygonize_core::PrecisionModel::Floating
        },
        ..Default::default()
    };
    let lines = parse_buffer_lines(coords, offsets, stride, line_ids.as_deref())
        .map_err(|error| to_js_error(error.name, error.message))?;

    polygonize_and_flatten(lines, options, stride)
}

#[wasm_bindgen(js_name = polygonizeGeoArrowWithOptions)]
/// Polygonizes an Arrow IPC stream containing a GeoArrow LineString array.
///
/// This binary path avoids JSON serialization overhead and returns an
/// Arrow IPC stream containing a GeoArrow Polygon array. Requires the options
/// to be passed as a parsed JS object.
pub fn polygonize_geoarrow_with_options_js(
    ipc_bytes: &[u8],
    options_val: JsValue,
) -> Result<Vec<u8>, JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();

    let options: geo_polygonize_core::PolygonizerOptions =
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
        precision_model: if node_input {
            geo_polygonize_core::PrecisionModel::from_grid_size(snap_grid_size)
        } else {
            geo_polygonize_core::PrecisionModel::Floating
        },
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
            if matches!(
                metadata.as_str(),
                "geoarrow.linestring" | "ogc.geoarrow.linestring"
            ) {
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
        .map_err(from_polygonizer_error)?;

    // Serialize result to IPC
    let mut output_buffer = Vec::new();
    {
        let field = result_array.data_type().to_field("geometry", true);
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
    lines: Vec<Line3D>,
    options: geo_polygonize_core::PolygonizerOptions,
    stride: u8,
) -> Result<WasmPolygonResult, JsValue> {
    let mut result = polygonize_lines(lines, &options).map_err(from_polygonizer_error)?;
    let topology_fingerprint = TopologyFingerprintV1::try_from_result(&result, &options)
        .map_err(from_polygonizer_error)?;
    let topology_fingerprint = js_sys::JSON::parse(
        &serde_json::to_string(&topology_fingerprint)
            .map_err(|e| to_js_error("InternalInvariantViolation", e))?,
    )
    .map_err(|e| to_js_error("InternalInvariantViolation", format!("{e:?}")))?;

    let flatten_started = js_sys::Date::now();
    let mut flat_coords = Vec::new();
    let mut ring_offsets = Vec::new();
    let mut polygon_offsets = Vec::new();
    let mut flat_line_ids = Vec::new();
    let mut provenances = Vec::new();

    let js_dangles = serde_wasm_bindgen::to_value(&result.dangles).unwrap_or(JsValue::NULL);
    let js_cut_edges = serde_wasm_bindgen::to_value(&result.cut_edges).unwrap_or(JsValue::NULL);
    let js_invalid_rings =
        serde_wasm_bindgen::to_value(&result.invalid_rings).unwrap_or(JsValue::NULL);
    let offset = |value: usize, name: &str| {
        u32::try_from(value).map_err(|_| {
            to_js_error(
                "ResourceLimitExceeded",
                format!("{name} exceeds the Wasm u32 offset range"),
            )
        })
    };

    for poly in result.polygons {
        provenances.push(poly.provenance);
        polygon_offsets.push(offset(ring_offsets.len(), "polygon ring offset")?);

        let exterior = poly.exterior;
        let interiors = poly.interiors;

        ring_offsets.push(offset(
            flat_coords.len() / stride as usize,
            "ring coordinate offset",
        )?);
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
            ring_offsets.push(offset(
                flat_coords.len() / stride as usize,
                "ring coordinate offset",
            )?);
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

    if let Some(diag) = result.diagnostics.as_mut() {
        diag.phase_times.output_flatten =
            std::time::Duration::from_secs_f64((js_sys::Date::now() - flatten_started) / 1_000.0);
    }
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
        dangles: js_dangles,
        cut_edges: js_cut_edges,
        invalid_rings: js_invalid_rings,
        diagnostics: js_diagnostics,
        topology_fingerprint,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use geoarrow::array::PolygonArray;
    use std::sync::Arc;
    use wasm_bindgen_test::*;

    #[allow(dead_code)]
    mod geoarrow_reference {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/geoarrow/reference.rs"
        ));
    }

    #[wasm_bindgen_test]
    fn test_polygonize_geoarrow_empty() {
        let result = polygonize_geoarrow(b"invalid data", false, 0.0, false);
        assert!(result.is_err());
    }

    #[wasm_bindgen_test]
    fn test_polygonize_geoarrow_valid() {
        let ipc_bytes = geoarrow_reference::square_ipc(Arc::new(Default::default()));
        let outputs = [
            polygonize_geoarrow(&ipc_bytes, false, 0.0, false),
            polygonize_geoarrow_with_options_js(
                &ipc_bytes,
                serde_wasm_bindgen::to_value(&PolygonizerOptions::default()).unwrap(),
            ),
        ];

        for result in outputs {
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
            let batches = reader.collect::<Result<Vec<_>, _>>().unwrap();

            assert_eq!(batches.len(), 1);
            let geom_col = batches[0].column(0);
            let poly_array = PolygonArray::try_from((geom_col.as_ref(), geom_field)).unwrap();
            geoarrow_reference::assert_square(&poly_array);
        }
    }

    #[wasm_bindgen_test]
    fn test_polygonize_geoarrow_errors_keep_normalized_fields() {
        let ipc_bytes = geoarrow_reference::square_ipc(Arc::new(Default::default()));
        let options = PolygonizerOptions {
            node_input: true,
            precision_model: geo_polygonize_core::PrecisionModel::FixedGrid { grid_size: -1.0 },
            ..Default::default()
        };
        let errors = [
            polygonize_geoarrow(&ipc_bytes, true, -1.0, false).unwrap_err(),
            polygonize_geoarrow_with_options_js(
                &ipc_bytes,
                serde_wasm_bindgen::to_value(&options).unwrap(),
            )
            .unwrap_err(),
        ];

        for error in errors {
            let normalized = js_sys::Reflect::get(&error, &"normalized".into()).unwrap();
            assert_eq!(
                js_sys::Reflect::get(&normalized, &"family".into())
                    .unwrap()
                    .as_string()
                    .as_deref(),
                Some("invalid_argument")
            );
            assert_eq!(
                js_sys::Reflect::get(&normalized, &"code".into())
                    .unwrap()
                    .as_string()
                    .as_deref(),
                Some("invalid_argument_type")
            );
            assert_eq!(
                js_sys::Reflect::get(&normalized, &"stage".into())
                    .unwrap()
                    .as_string()
                    .as_deref(),
                Some("options")
            );
        }
    }

    #[wasm_bindgen_test]
    fn test_polygonize_official_geoarrow_reference_layouts() {
        for ipc_bytes in [
            geoarrow_reference::official_separated_ipc(),
            geoarrow_reference::official_interleaved_ipc(),
        ] {
            let out_bytes = polygonize_geoarrow(&ipc_bytes, false, 0.0, false).unwrap();
            let mut reader =
                arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(out_bytes), None)
                    .unwrap();
            let schema = reader.schema();
            let batch = reader.next().unwrap().unwrap();
            let polygons =
                PolygonArray::try_from((batch.column(0).as_ref(), schema.field(0))).unwrap();
            assert_eq!(polygons.len(), 0);
        }

        for ipc_bytes in geoarrow_reference::official_non_xy_ipc() {
            assert!(polygonize_geoarrow(&ipc_bytes, false, 0.0, false).is_err());
        }
    }
}
