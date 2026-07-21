#![doc = include_str!("../../../README.md")]

pub mod arrow_api;
pub mod containment;
pub mod diagnostics;
pub mod error;
pub mod ffi;
pub mod graph;
pub mod index;
pub mod noding;
pub mod options;
pub mod polygonizer;
pub mod tiling;
pub mod types;
pub mod utils;

#[cfg(doctest)]
#[doc = include_str!("../../../docs/guide/getting-started.md")]
mod getting_started_guide {}

#[cfg(feature = "python")]
pub mod python;

#[cfg(test)]
mod polygonizer_tests;

pub use noding::hot_pixel::HotPixelNoder;
pub use noding::validate::ValidatingNoder;
pub use polygonizer::{
    polygonize, polygonize_with_workspace, Polygonizer, PolygonizerResult, PolygonizerWorkspace,
};
pub use tiling::TiledPolygonizer;
pub use types::{Coord3D, EdgeSources, Line3D, Polygon3D};

#[cfg(feature = "geoparquet")]
pub mod geoparquet_api;

#[cfg(feature = "flatgeobuf")]
pub mod flatgeobuf_api;
