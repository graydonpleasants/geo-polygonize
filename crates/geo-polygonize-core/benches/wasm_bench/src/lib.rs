use wasm_bindgen::prelude::*;
use geo::{LineString, Geometry, Rect, Coord};
use geo_polygonize_core::{Polygonizer, TiledPolygonizer};
use geo_polygonize_core::noding::snap::{NodingStrategy, SnapNoder};
use geoarrow::array::{GeoArrowArrayAccessor, LineStringBuilder};
use geoarrow::datatypes::{LineStringType, Dimension};
use std::convert::TryInto;
use geo_polygonize_core::graph::PlanarGraph;

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
    polygonizer.node_input = true;
    let results = polygonizer.polygonize();

    let results_vec = results.map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
    Ok(JsValue::from(results_vec.polygons.len()))
}

#[wasm_bindgen]
pub fn polygonize_tiled(lines: JsValue, size: f64) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;

    let bbox = Rect::new(
        Coord { x: 0.0, y: 0.0 },
        Coord {
            x: size,
            y: size,
        },
    );
    let tile_size = size / 2.0;

    let mut tiler = TiledPolygonizer::new(bbox, tile_size).with_buffer(1.0);
    let geoms: Vec<Geometry<f64>> = lines.into_iter().map(|l| Geometry::LineString(l)).collect();
    for geom in &geoms {
        tiler.add_geometry(geom);
    }
    let results = tiler.polygonize();

    let count: usize = results.len();
    Ok(JsValue::from(count))
}

#[wasm_bindgen]
pub fn polygonize_random(lines: JsValue) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;

    // Core Logic
    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = true;
    for line in lines {
        polygonizer.add_geometry(Geometry::LineString(line));
    }
    let results = polygonizer.polygonize();

    let results_vec = results.map_err(|e| JsValue::from_str(&format!("{:?}", e)))?;
    Ok(JsValue::from(results_vec.polygons.len()))
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
    Ok(JsValue::from(results_vec.polygons.len()))
}

#[wasm_bindgen]
pub fn bowtie_noder_auto(lines: JsValue) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;
    let mut input_segments = Vec::new();
    for ls in &lines {
        for line in ls.lines() {
            input_segments.push(line.into());
        }
    }
    let noder = SnapNoder::new(1e-10); // Auto
    let _ = noder.node(input_segments);
    Ok(JsValue::NULL)
}

#[wasm_bindgen]
pub fn bowtie_noder_force_grid(lines: JsValue) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;
    let mut input_segments = Vec::new();
    for ls in &lines {
        for line in ls.lines() {
            input_segments.push(line.into());
        }
    }
    let noder = SnapNoder::new(1e-10).with_strategy(NodingStrategy::Grid);
    let _ = noder.node(input_segments);
    Ok(JsValue::NULL)
}

#[wasm_bindgen]
pub fn bowtie_noder_force_simd(lines: JsValue) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;
    let mut input_segments = Vec::new();
    for ls in &lines {
        for line in ls.lines() {
            input_segments.push(line.into());
        }
    }
    let noder = SnapNoder::new(1e-10).with_strategy(NodingStrategy::Simd);
    let _ = noder.node(input_segments);
    Ok(JsValue::NULL)
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

#[wasm_bindgen]
pub fn get_edge_rings(lines: JsValue) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;
    let mut graph = PlanarGraph::new();
    for line in &lines {
        graph.add_line_string(line.clone());
    }
    graph.sort_edges();
    let rings = graph.get_edge_rings();
    Ok(JsValue::from(rings.len()))
}

#[wasm_bindgen]
pub fn get_edge_rings_with_dangles(lines: JsValue) -> Result<JsValue, JsValue> {
    let lines = parse_input(lines)?;
    let mut graph = PlanarGraph::new();
    for line in &lines {
        graph.add_line_string(line.clone());
    }
    loop {
        if graph.prune_dangles().is_empty() {
            break;
        }
    }
    graph.sort_edges();
    let rings = graph.get_edge_rings();
    Ok(JsValue::from(rings.len()))
}
