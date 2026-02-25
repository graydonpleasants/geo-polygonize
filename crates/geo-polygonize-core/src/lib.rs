//! A native Rust port of the JTS/GEOS polygonization algorithm.
//!
//! This crate allows you to reconstruct valid polygons from a set of lines,
//! including handling of complex topologies like holes, nested shells, and disconnected components.
//!
//! # Features
//! - **Robust Noding**: Uses Iterated Snap Rounding to handle dirty inputs.
//! - **Performance**: SIMD-accelerated predicates and efficient memory layout.
//! - **Wasm**: Optimized for WebAssembly environments.

pub mod error;
pub mod ffi;
pub mod graph;
pub mod noding;
pub mod polygonizer;
pub mod tiling;
pub mod utils;

#[cfg(feature = "python")]
pub mod python;

#[cfg(test)]
mod polygonizer_tests;

pub use polygonizer::{Polygonizer, PolygonizerResult};
pub use tiling::TiledPolygonizer;
