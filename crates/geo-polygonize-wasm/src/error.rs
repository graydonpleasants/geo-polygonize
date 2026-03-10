use geo_polygonize_core::error::PolygonizerError;
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

pub fn from_polygonizer_error(e: PolygonizerError) -> JsValue {
    let name = match &e {
        PolygonizerError::TopologyError(_) => "TopologyError".to_string(),
        PolygonizerError::InvalidGeometry(_) => "InvalidGeometry".to_string(),
        PolygonizerError::NodingError(_) => "NodingError".to_string(),
        PolygonizerError::ArrowError(_) => "ArrowError".to_string(),
        PolygonizerError::NullPointer(_) => "NullPointer".to_string(),
        PolygonizerError::Panic(_) => "Panic".to_string(),
    };

    let err = PolygonizerWasmError::new(name, e.to_string());
    JsValue::from(err)
}

pub fn to_js_error<T: std::fmt::Display>(name: &str, message: T) -> JsValue {
    let err = PolygonizerWasmError::new(name.to_string(), message.to_string());
    JsValue::from(err)
}
