use crate::Polygonizer;
use geo_types::{Geometry, Polygon, Rect, Coord};
use geo::bounding_rect::BoundingRect;
use geo::intersects::Intersects;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use geo::Area;

pub struct TiledPolygonizer {
    bbox: Rect<f64>,
    tile_size: f64,
    buffer: f64, // Overlap buffer to ensure polygons are fully captured
    geometries: Vec<Geometry<f64>>,
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
        self.geometries.push(geom);
    }

    pub fn polygonize(&self) -> Vec<Polygon<f64>> {
        // 1. Generate tiles
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

                tiles.push(Rect::new(
                    Coord { x: x0, y: y0 },
                    Coord { x: x1, y: y1 },
                ));
            }
        }

        // 2. Process tiles in parallel or sequential
        let process_tile = |tile_bbox: Rect<f64>| -> Vec<Polygon<f64>> {
            let mut local_poly = Polygonizer::new();
            local_poly.node_input = true;

            // Define buffered bbox
            let buffered_bbox = Rect::new(
                Coord { x: tile_bbox.min().x - self.buffer, y: tile_bbox.min().y - self.buffer },
                Coord { x: tile_bbox.max().x + self.buffer, y: tile_bbox.max().y + self.buffer },
            );

            // Filter geometries intersecting the BUFFERED tile
            let mut relevant_lines = 0;
            for geom in &self.geometries {
                if geom.bounding_rect().map(|b| b.intersects(&buffered_bbox)).unwrap_or(false) {
                    local_poly.add_geometry(geom.clone());
                    relevant_lines += 1;
                }
            }

            if relevant_lines == 0 {
                return Vec::new();
            }

            // Run polygonization
            if let Ok(polys) = local_poly.polygonize() {
                // Ownership check:
                let mut valid_polys = Vec::new();
                for poly in polys {
                    use geo::algorithm::centroid::Centroid;
                    if let Some(pt) = poly.centroid() {
                        let c = pt;
                        let area = poly.unsigned_area();

                        // Filter slivers
                        if area < 1e-6 {
                            continue;
                        }

                        // Check inclusion [min, max)
                        let in_x = c.x() >= tile_bbox.min().x && c.x() < tile_bbox.max().x;
                        let in_y = c.y() >= tile_bbox.min().y && c.y() < tile_bbox.max().y;

                        if in_x && in_y {
                            valid_polys.push(poly);
                        }
                    }
                }
                valid_polys
            } else {
                Vec::new()
            }
        };

        let result_polygons: Vec<Polygon<f64>>;
        #[cfg(feature = "parallel")]
        {
            result_polygons = tiles.into_par_iter().flat_map(process_tile).collect();
        }
        #[cfg(not(feature = "parallel"))]
        {
            result_polygons = tiles.into_iter().flat_map(process_tile).collect();
        }

        result_polygons
    }
}

#[cfg(test)]
#[path = "tiling_tests.rs"]
mod tests;
