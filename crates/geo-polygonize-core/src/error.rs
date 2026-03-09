use thiserror::Error;

#[derive(Error, Debug)]
pub enum PolygonizerError {
    #[error("Topology error: {0}")]
    TopologyError(String),

    #[error("Invalid geometry: {0}")]
    InvalidGeometry(String),

    #[error("Noding failed: {0}")]
    NodingError(String),

    #[error("Arrow array conversion failed: {0}")]
    ArrowError(String),

    #[error("Null pointer provided to FFI function: {0}")]
    NullPointer(String),

    #[error("Panic occurred across FFI/WASM boundary: {0}")]
    Panic(String),
}

pub type Result<T> = std::result::Result<T, PolygonizerError>;
