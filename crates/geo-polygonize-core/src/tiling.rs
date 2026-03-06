use crate::types::Polygon3D;
use crate::Polygonizer;
use geo::bounding_rect::BoundingRect;
use geo::intersects::Intersects;
use geo_types::{Coord, Geometry, Rect};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

pub struct TiledPolygonizer {
    bbox: Rect<f64>,
    tile_size: f64,
    buffer: f64, // Overlap buffer to ensure polygons are fully captured
    geometries: Vec<(Geometry<f64>, Option<Rect<f64>>)>,
}

impl TiledPolygonizer {
    pub fn new(bbox: Rect<f64>, tile_size: f64) -> Self {
        Self {
            bbox,
            tile_size,
            buffer: 0.0,
            geometries: Vec::new(),
        }
    }

    pub fn with_buffer(mut self, buffer: f64) -> Self {
        self.buffer = buffer;
        self
    }

    pub fn add_geometry(&mut self, geom: Geometry<f64>) {
        let bbox = geom.bounding_rect();
        self.geometries.push((geom, bbox));
    }

    fn process_tile(&self, tile_bbox: Rect<f64>) -> Vec<Polygon3D> {
        let mut local_poly = Polygonizer::new();
        local_poly.node_input = true;

        // Define buffered bbox
        let buffered_bbox = Rect::new(
            Coord {
                x: tile_bbox.min().x - self.buffer,
                y: tile_bbox.min().y - self.buffer,
            },
            Coord {
                x: tile_bbox.max().x + self.buffer,
                y: tile_bbox.max().y + self.buffer,
            },
        );

        // Filter geometries intersecting the BUFFERED tile
        let mut relevant_lines = 0;
        for (geom, bbox) in &self.geometries {
            if bbox.map(|b| b.intersects(&buffered_bbox)).unwrap_or(false) {
                local_poly.add_borrowed_geometry(geom);
                relevant_lines += 1;
            }
        }

        if relevant_lines == 0 {
            return Vec::new();
        }

        // Run polygonization
        if let Ok(result) = local_poly.polygonize() {
            // Ownership check:
            let mut valid_polys = Vec::new();
            for poly in result.polygons {
                // Fast path avoiding geometry allocation
                if let Some(pt) = poly.centroid_2d() {
                    let c = pt;
                    let area = poly.unsigned_area_2d();

                    // Filter slivers
                    if area < 1e-6 {
                        continue;
                    }

                    // Check inclusion [min, max)
                    // For the last tile in a row/col, we include the max boundary to cover the full bbox.
                    let max_x_inclusive = tile_bbox.max().x >= self.bbox.max().x;
                    let max_y_inclusive = tile_bbox.max().y >= self.bbox.max().y;

                    let in_x = if max_x_inclusive {
                        c.x() >= tile_bbox.min().x && c.x() <= tile_bbox.max().x
                    } else {
                        c.x() >= tile_bbox.min().x && c.x() < tile_bbox.max().x
                    };

                    let in_y = if max_y_inclusive {
                        c.y() >= tile_bbox.min().y && c.y() <= tile_bbox.max().y
                    } else {
                        c.y() >= tile_bbox.min().y && c.y() < tile_bbox.max().y
                    };

                    if in_x && in_y {
                        valid_polys.push(poly);
                    }
                }
            }
            valid_polys
        } else {
            Vec::new()
        }
    }

    fn generate_tiles(&self) -> Vec<Rect<f64>> {
        let min = self.bbox.min();
        let max = self.bbox.max();
        let width = max.x - min.x;
        let height = max.y - min.y;

        let cols = (width / self.tile_size).ceil() as usize;
        let rows = (height / self.tile_size).ceil() as usize;

        let mut tiles = Vec::new();
        for r in 0..rows {
            for c in 0..cols {
                let x0 = min.x + c as f64 * self.tile_size;
                let y0 = min.y + r as f64 * self.tile_size;
                let x1 = (x0 + self.tile_size).min(max.x);
                let y1 = (y0 + self.tile_size).min(max.y);

                tiles.push(Rect::new(Coord { x: x0, y: y0 }, Coord { x: x1, y: y1 }));
            }
        }
        tiles
    }

    pub fn polygonize(&self) -> Vec<Polygon3D> {
        let tiles = self.generate_tiles();

        // Process tiles in parallel or sequential
        let result_polygons: Vec<Polygon3D>;
        #[cfg(feature = "parallel")]
        {
            result_polygons = tiles
                .into_par_iter()
                .flat_map(|tile| self.process_tile(tile))
                .collect();
        }
        #[cfg(not(feature = "parallel"))]
        {
            result_polygons = tiles
                .into_iter()
                .flat_map(|tile| self.process_tile(tile))
                .collect();
        }

        result_polygons
    }
}

#[cfg(test)]
#[path = "tiling_tests.rs"]
mod tests;
