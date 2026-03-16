use geo_polygonize_core::error::PolygonizeError;
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

pub(crate) fn error_name(e: &PolygonizeError) -> &'static str {
    match e {
        PolygonizeError::InvalidArgumentType { .. } => "InvalidArgumentType",
        PolygonizeError::InvalidGeometry { .. } => "InvalidGeometry",
        PolygonizeError::InvalidBufferShape { .. } => "InvalidBufferShape",
        PolygonizeError::UnsupportedOptionCombination { .. } => "UnsupportedOptionCombination",
        PolygonizeError::TopologyFailure { .. } => "TopologyFailure",
        PolygonizeError::InternalInvariantViolation { .. } => "InternalInvariantViolation",
        PolygonizeError::ArrowError(_) => "ArrowError",
        PolygonizeError::NullPointer(_) => "NullPointer",
        PolygonizeError::Panic(_) => "Panic",
    }
}

pub fn from_polygonizer_error(e: PolygonizeError) -> JsValue {
    let name = error_name(&e);
    let err = PolygonizerWasmError::new(name.to_string(), e.to_string());
    JsValue::from(err)
}

pub fn to_js_error<T: std::fmt::Display>(name: &str, message: T) -> JsValue {
    let err = PolygonizerWasmError::new(name.to_string(), message.to_string());
    JsValue::from(err)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_polygonize_core::error::PolygonizeError;

    #[test]
    fn test_error_name_mapping() {
        let cases = vec![
            (PolygonizeError::InvalidArgumentType { field: "".into(), expected: "".into(), actual: "".into() }, "InvalidArgumentType"),
            (PolygonizeError::InvalidGeometry { reason: "".into() }, "InvalidGeometry"),
            (PolygonizeError::InvalidBufferShape { reason: "".into() }, "InvalidBufferShape"),
            (PolygonizeError::UnsupportedOptionCombination { reason: "".into() }, "UnsupportedOptionCombination"),
            (PolygonizeError::TopologyFailure { reason: "".into() }, "TopologyFailure"),
            (PolygonizeError::InternalInvariantViolation { reason: "".into() }, "InternalInvariantViolation"),
            (PolygonizeError::ArrowError("".into()), "ArrowError"),
            (PolygonizeError::NullPointer("".into()), "NullPointer"),
            (PolygonizeError::Panic("".into()), "Panic"),
        ];

        for (error, expected_name) in cases {
            assert_eq!(error_name(&error), expected_name);
        }
    }
}
