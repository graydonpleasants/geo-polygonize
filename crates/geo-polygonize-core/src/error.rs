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
    InvalidGeometry { reason: String },

    #[error("Invalid buffer shape: {reason}")]
    InvalidBufferShape { reason: String },

    #[error("Resource limit exceeded at {stage}: observed {observed}, limit {limit}")]
    ResourceLimitExceeded {
        stage: String,
        limit: usize,
        observed: usize,
    },

    #[error("Polygonization cancelled at {stage}")]
    Cancelled { stage: String },

    #[error("Unsupported option combination: {reason}")]
    UnsupportedOptionCombination { reason: String },

    #[error("Topology failure: {reason}")]
    TopologyFailure { reason: String },

    #[error("Z conflict at ({x}, {y}) from input lines {line_ids:?}")]
    ZConflict { x: f64, y: f64, line_ids: Vec<u32> },

    #[error(
        "Noding validation failed between output segments {first_segment} and {second_segment}: {reason}"
    )]
    NodingValidationFailure {
        first_segment: usize,
        second_segment: usize,
        reason: String,
    },

    #[error("Internal invariant violation: {reason}")]
    InternalInvariantViolation { reason: String },

    #[error("Arrow array conversion failed: {0}")]
    ArrowError(String),

    #[error("Null pointer provided to FFI function: {0}")]
    NullPointer(String),

    #[error("Panic occurred across FFI/WASM boundary: {0}")]
    Panic(String),
}

pub type Result<T> = std::result::Result<T, PolygonizeError>;
