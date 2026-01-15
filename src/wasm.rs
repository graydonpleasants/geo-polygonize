use wasm_bindgen::prelude::*;
use crate::Polygonizer;
use geojson::{GeoJson, Geometry, Value};
use std::convert::TryInto;
use std::str::FromStr;

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
                    let geo_geom: geo_types::Geometry<f64> = geom.try_into()
                        .map_err(|e| JsValue::from_str(&format!("Conversion error: {}", e)))?;
                    polygonizer.add_geometry(geo_geom);
                }
            }
        },
        GeoJson::Feature(f) => {
             if let Some(geom) = f.geometry {
                let geo_geom: geo_types::Geometry<f64> = geom.try_into()
                    .map_err(|e| JsValue::from_str(&format!("Conversion error: {}", e)))?;
                polygonizer.add_geometry(geo_geom);
            }
        },
        GeoJson::Geometry(g) => {
            let geo_geom: geo_types::Geometry<f64> = g.try_into()
                .map_err(|e| JsValue::from_str(&format!("Conversion error: {}", e)))?;
            polygonizer.add_geometry(geo_geom);
        }
    }

    let polygons = polygonizer.polygonize()
        .map_err(|e| JsValue::from_str(&format!("Polygonization error: {}", e)))?;

    // Convert back to GeoJSON
    let geometries: Vec<Geometry> = polygons.into_iter()
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
