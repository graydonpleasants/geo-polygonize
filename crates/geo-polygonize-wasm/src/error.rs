use geo_polygonize_core::{normalize_polygonize_error, PolygonizeError};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct PolygonizerWasmError {
    name: String,
    message: String,
    normalized: JsValue,
}

#[wasm_bindgen]
impl PolygonizerWasmError {
    #[wasm_bindgen(constructor)]
    pub fn new(name: String, message: String) -> PolygonizerWasmError {
        Self {
            name,
            message,
            normalized: JsValue::NULL,
        }
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn normalized(&self) -> JsValue {
        self.normalized.clone()
    }
}

pub fn from_polygonizer_error(e: PolygonizeError) -> JsValue {
    let name = match &e {
        PolygonizeError::InvalidArgumentType { .. } => "InvalidArgumentType".to_string(),
        PolygonizeError::InvalidGeometry { .. } | PolygonizeError::NonFiniteCoordinate { .. } => {
            "InvalidGeometry".to_string()
        }
        PolygonizeError::InvalidBufferShape { .. } => "InvalidBufferShape".to_string(),
        PolygonizeError::ResourceLimitExceeded { .. } => "ResourceLimitExceeded".to_string(),
        PolygonizeError::Cancelled { .. } => "Cancelled".to_string(),
        PolygonizeError::UnsupportedOptionCombination { .. } => {
            "UnsupportedOptionCombination".to_string()
        }
        PolygonizeError::TopologyFailure { .. } => "TopologyFailure".to_string(),
        PolygonizeError::ZConflict { .. } => "ZConflict".to_string(),
        PolygonizeError::NodingValidationFailure { .. } => "NodingValidationFailure".to_string(),
        PolygonizeError::InternalInvariantViolation { .. } => {
            "InternalInvariantViolation".to_string()
        }
        PolygonizeError::ArrowError(_) => "ArrowError".to_string(),
        PolygonizeError::NullPointer(_) => "NullPointer".to_string(),
        PolygonizeError::Panic(_) => "Panic".to_string(),
    };

    let mut err = PolygonizerWasmError::new(name, e.to_string());
    err.normalized =
        serde_wasm_bindgen::to_value(&normalize_polygonize_error(&e)).unwrap_or(JsValue::NULL);
    JsValue::from(err)
}

pub fn to_js_error<T: std::fmt::Display>(name: &str, message: T) -> JsValue {
    let err = PolygonizerWasmError::new(name.to_string(), message.to_string());
    JsValue::from(err)
}
