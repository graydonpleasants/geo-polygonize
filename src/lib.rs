pub mod graph;
pub mod polygonizer;
pub mod error;
pub mod utils;
pub mod tiling;
pub mod noding;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

#[cfg(test)]
mod polygonizer_tests;

pub use polygonizer::Polygonizer;
pub use tiling::TiledPolygonizer;
