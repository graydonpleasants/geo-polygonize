use geo_polygonize_core::PolygonizeError;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct PolygonizerWasmError {
    name: String,
    message: String,
}

#[wasm_bindgen]
impl PolygonizerWasmError {
    #[wasm_bindgen(constructor)]
    pub fn new(name: String, message: String) -> PolygonizerWasmError {
        Self { name, message }
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn message(&self) -> String {
        self.message.clone()
    }
}

pub fn from_polygonizer_error(e: PolygonizeError) -> JsValue {
    let name = match &e {
        PolygonizeError::InvalidArgumentType { .. } => "InvalidArgumentType".to_string(),
        PolygonizeError::InvalidGeometry { .. } => "InvalidGeometry".to_string(),
        PolygonizeError::InvalidBufferShape { .. } => "InvalidBufferShape".to_string(),
        PolygonizeError::UnsupportedOptionCombination { .. } => {
            "UnsupportedOptionCombination".to_string()
        }
        PolygonizeError::TopologyFailure { .. } => "TopologyFailure".to_string(),
        PolygonizeError::NodingValidationFailure { .. } => "NodingValidationFailure".to_string(),
        PolygonizeError::InternalInvariantViolation { .. } => {
            "InternalInvariantViolation".to_string()
        }
        PolygonizeError::ArrowError(_) => "ArrowError".to_string(),
        PolygonizeError::NullPointer(_) => "NullPointer".to_string(),
        PolygonizeError::Panic(_) => "Panic".to_string(),
    };

    let err = PolygonizerWasmError::new(name, e.to_string());
    JsValue::from(err)
}

pub fn to_js_error<T: std::fmt::Display>(name: &str, message: T) -> JsValue {
    let err = PolygonizerWasmError::new(name.to_string(), message.to_string());
    JsValue::from(err)
}
