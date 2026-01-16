//! A native Rust port of the JTS/GEOS polygonization algorithm.
//!
//! This crate allows you to reconstruct valid polygons from a set of lines,
//! including handling of complex topologies like holes, nested shells, and disconnected components.
//!
//! # Features
//! - **Robust Noding**: Uses Iterated Snap Rounding to handle dirty inputs.
//! - **Performance**: SIMD-accelerated predicates and efficient memory layout.
//! - **Wasm**: Optimized for WebAssembly environments.

pub mod graph;
pub mod polygonizer;
pub mod error;
pub mod utils;
pub mod tiling;
pub mod noding;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(test)]
mod polygonizer_tests;

pub use polygonizer::Polygonizer;
pub use tiling::TiledPolygonizer;
