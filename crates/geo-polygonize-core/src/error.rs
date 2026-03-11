use thiserror::Error;

#[derive(Error, Debug)]
pub enum PolygonizeError {
    #[error("Invalid argument type for {field}: expected {expected}, got {actual}")]
    InvalidArgumentType {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("Invalid geometry: {reason}")]
    InvalidGeometry {
        reason: String,
    },

    #[error("Invalid buffer shape: {reason}")]
    InvalidBufferShape {
        reason: String,
    },

    #[error("Unsupported option combination: {reason}")]
    UnsupportedOptionCombination {
        reason: String,
    },

    #[error("Topology failure: {reason}")]
    TopologyFailure {
        reason: String,
    },

    #[error("Internal invariant violation: {reason}")]
    InternalInvariantViolation {
        reason: String,
    },

    #[error("Arrow array conversion failed: {0}")]
    ArrowError(String),

    #[error("Null pointer provided to FFI function: {0}")]
    NullPointer(String),

    #[error("Panic occurred across FFI/WASM boundary: {0}")]
    Panic(String),
}

pub type Result<T> = std::result::Result<T, PolygonizeError>;
