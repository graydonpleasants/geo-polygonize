use wasm_bindgen::prelude::*;
use geojson::{GeoJson, Feature, FeatureCollection, Value, Geometry};
use geo::Geometry as GeoGeometry;
use crate::Polygonizer;
use std::convert::TryInto;
use std::str::FromStr;

#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn polygonize(geojson_str: &str) -> Result<String, JsValue> {
    // Parse the GeoJSON string
    let geojson = GeoJson::from_str(geojson_str)
        .map_err(|e| JsValue::from_str(&format!("Failed to parse GeoJSON: {}", e)))?;

    let mut polygonizer = Polygonizer::new();
    // Enable noding to handle intersecting lines from user input
    polygonizer.node_input = true;

    // Helper to add geometry to polygonizer
    fn add_geom(p: &mut Polygonizer, geom: GeoGeometry<f64>) {
        p.add_geometry(geom);
    }

    // Process GeoJSON to extract lines
    match geojson {
        GeoJson::FeatureCollection(fc) => {
            for feature in fc.features {
                if let Some(geom) = feature.geometry {
                    if let Ok(geo_geom) = geom.try_into() {
                        add_geom(&mut polygonizer, geo_geom);
                    }
                }
            }
        },
        GeoJson::Feature(feature) => {
             if let Some(geom) = feature.geometry {
                if let Ok(geo_geom) = geom.try_into() {
                    add_geom(&mut polygonizer, geo_geom);
                }
            }
        },
        GeoJson::Geometry(geometry) => {
            if let Ok(geo_geom) = geometry.try_into() {
                add_geom(&mut polygonizer, geo_geom);
            }
        },
    }

    // Run Polygonization
    let result_polygons = polygonizer.polygonize()
        .map_err(|e| JsValue::from_str(&format!("Polygonization failed: {}", e)))?;

    // Convert back to GeoJSON
    let features: Vec<Feature> = result_polygons
        .into_iter()
        .map(|poly| {
            let geometry = Geometry::new(Value::from(&poly));
            Feature {
                bbox: None,
                geometry: Some(geometry),
                id: None,
                properties: None,
                foreign_members: None,
            }
        })
        .collect();

    let fc = FeatureCollection {
        bbox: None,
        features,
        foreign_members: None,
    };

    Ok(fc.to_string())
}
