use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodingValidationKind {
    ZeroLengthSegment,
    CollinearOverlap,
    InteriorIntersection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolygonizeErrorKind {
    InvalidArgumentType,
    InvalidGeometry,
    InvalidBufferShape,
    ResourceLimitExceeded,
    Cancelled,
    UnsupportedOptionCombination,
    ZConflict,
    NodingValidationFailure(NodingValidationKind),
    InternalInvariantViolation,
    ArrowError,
    Panic,
}

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

    #[error("Invalid geometry: {reason}")]
    NonFiniteCoordinate { reason: String },

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

    #[error("Z conflict at ({x}, {y}) from input lines {line_ids:?}")]
    ZConflict { x: f64, y: f64, line_ids: Vec<u32> },

    #[error(
        "Noding validation failed between output segments {first_segment} and {second_segment}: {reason}"
    )]
    NodingValidationFailure {
        first_segment: usize,
        second_segment: usize,
        kind: NodingValidationKind,
        reason: String,
    },

    #[error("Internal invariant violation: {reason}")]
    InternalInvariantViolation { reason: String },

    #[error("Arrow array conversion failed: {0}")]
    ArrowError(String),

    #[error("Panic occurred across FFI/WASM boundary: {0}")]
    Panic(String),
}

impl PolygonizeError {
    pub fn kind(&self) -> PolygonizeErrorKind {
        match self {
            Self::InvalidArgumentType { .. } => PolygonizeErrorKind::InvalidArgumentType,
            Self::InvalidGeometry { .. } | Self::NonFiniteCoordinate { .. } => {
                PolygonizeErrorKind::InvalidGeometry
            }
            Self::InvalidBufferShape { .. } => PolygonizeErrorKind::InvalidBufferShape,
            Self::ResourceLimitExceeded { .. } => PolygonizeErrorKind::ResourceLimitExceeded,
            Self::Cancelled { .. } => PolygonizeErrorKind::Cancelled,
            Self::UnsupportedOptionCombination { .. } => {
                PolygonizeErrorKind::UnsupportedOptionCombination
            }
            Self::ZConflict { .. } => PolygonizeErrorKind::ZConflict,
            Self::NodingValidationFailure { kind, .. } => {
                PolygonizeErrorKind::NodingValidationFailure(*kind)
            }
            Self::InternalInvariantViolation { .. } => {
                PolygonizeErrorKind::InternalInvariantViolation
            }
            Self::ArrowError { .. } => PolygonizeErrorKind::ArrowError,
            Self::Panic { .. } => PolygonizeErrorKind::Panic,
        }
    }
}

pub type Result<T> = std::result::Result<T, PolygonizeError>;
