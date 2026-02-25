use thiserror::Error;

#[derive(Error, Debug)]
pub enum PolygonizerError {
    #[error("Topology error: {0}")]
    TopologyError(String),

    #[error("Invalid geometry: {0}")]
    InvalidGeometry(String),

    #[error("Noding failed: {0}")]
    NodingError(String),
}

pub type Result<T> = std::result::Result<T, PolygonizerError>;
