//! # Geo Polygonize
//!
//! `geo-polygonize` provides algorithms to reconstruct valid `geo_types::Polygon`s from a set of `geo_types::Geometry` inputs.
//! It is a port of the JTS/GEOS Polygonizer, adapted for Rust's ownership model and type system.

pub mod graph;
pub mod polygonizer;
pub mod error;

#[cfg(test)]
mod polygonizer_tests;

pub use polygonizer::Polygonizer;
pub use error::{Result, PolygonizerError};
