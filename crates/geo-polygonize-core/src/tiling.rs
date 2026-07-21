use crate::options::{DedupPolicy, TileOwnershipPolicy};
use crate::polygonizer::apply_determinism;
use crate::types::Polygon3D;
use crate::{PolygonizeError, Polygonizer, PolygonizerOptions, Result};
use geo::bounding_rect::BoundingRect;
use geo::intersects::Intersects;
use geo::InteriorPoint;
use geo_types::{Coord, Geometry, Point, Rect};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Experimental tiled polygonization.
///
/// Equivalence with untiled output is not guaranteed.
pub struct TiledPolygonizer<'a> {
    bbox: Rect<f64>,
    tile_size: f64,
    buffer: f64, // Overlap buffer to ensure polygons are fully captured
    geometries: Vec<(&'a Geometry<f64>, Option<Rect<f64>>)>,
    ownership_policy: TileOwnershipPolicy,
    dedup_policy: DedupPolicy,
    options: PolygonizerOptions,
}

impl<'a> TiledPolygonizer<'a> {
    pub fn new(bbox: Rect<f64>, tile_size: f64) -> Self {
        let options = PolygonizerOptions {
            node_input: true,
            ..Default::default()
        };
        Self {
            bbox,
            tile_size,
            buffer: 0.0,
            geometries: Vec::new(),
            ownership_policy: TileOwnershipPolicy::Centroid,
            dedup_policy: DedupPolicy::KeepAll,
            options,
        }
    }

    pub fn with_buffer(mut self, buffer: f64) -> Self {
        self.buffer = buffer;
        self
    }

    pub fn with_ownership_policy(mut self, policy: TileOwnershipPolicy) -> Self {
        self.ownership_policy = policy;
        self
    }

    pub fn with_dedup_policy(mut self, policy: DedupPolicy) -> Self {
        self.dedup_policy = policy;
        self
    }

    pub fn with_options(mut self, options: PolygonizerOptions) -> Self {
        self.options = options;
        self
    }

    pub fn add_geometry(&mut self, geom: &'a Geometry<f64>) {
        let bbox = geom.bounding_rect();
        self.geometries.push((geom, bbox));
    }

    fn process_tile(&self, tile_bbox: Rect<f64>) -> Result<Vec<Polygon3D>> {
        let mut local_poly = Polygonizer::with_options(self.options.clone());

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
            return Ok(Vec::new());
        }

        // Run polygonization
        let result = local_poly.polygonize()?;
        // Ownership check:
        let mut valid_polys = Vec::new();
        for poly in result.polygons {
            if let Some(pt) = self.ownership_point(&poly) {
                let c = pt;

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
        Ok(valid_polys)
    }

    fn ownership_point(&self, poly: &Polygon3D) -> Option<Point<f64>> {
        match self.ownership_policy {
            TileOwnershipPolicy::Centroid => poly.centroid_2d(),
            TileOwnershipPolicy::RepresentativePointInsidePolygon
            | TileOwnershipPolicy::CanonicalBoundaryHash => poly.to_polygon_2d().interior_point(),
            TileOwnershipPolicy::LexicographicMinVertex => poly
                .exterior
                .iter()
                .min_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)))
                .map(|coord| Point::new(coord.x, coord.y)),
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

    pub fn polygonize(&self) -> Result<Vec<Polygon3D>> {
        self.validate()?;
        let tiles = self.generate_tiles();

        // Process tiles in parallel or sequential
        let tile_results: Vec<Result<Vec<Polygon3D>>>;
        #[cfg(feature = "parallel")]
        {
            tile_results = tiles
                .into_par_iter()
                .map(|tile| self.process_tile(tile))
                .collect();
        }
        #[cfg(not(feature = "parallel"))]
        {
            tile_results = tiles
                .into_iter()
                .map(|tile| self.process_tile(tile))
                .collect();
        }
        let tile_polygons = tile_results.into_iter().collect::<Result<Vec<_>>>()?;
        let result_polygons: Vec<Polygon3D> = tile_polygons.into_iter().flatten().collect();

        let polygons = match self.dedup_policy {
            DedupPolicy::KeepAll => result_polygons,
            DedupPolicy::CanonicalRingHash => {
                use std::collections::HashSet;
                use std::hash::{DefaultHasher, Hash, Hasher};

                let mut unique_polygons = Vec::new();
                let mut seen_hashes = HashSet::new();

                fn hash_ring(ring: &[crate::types::Coord3D]) -> u64 {
                    if ring.is_empty() {
                        return 0;
                    }
                    if ring.len() == 1 {
                        let mut h = DefaultHasher::new();
                        ring[0].x.to_bits().hash(&mut h);
                        ring[0].y.to_bits().hash(&mut h);
                        ring[0].z.to_bits().hash(&mut h);
                        return h.finish();
                    }

                    let unique_len = ring.len() - 1;

                    // compute forward minimum
                    let mut min_idx_fwd = 0;
                    for i in 1..unique_len {
                        let a = &ring[i];
                        let b = &ring[min_idx_fwd];
                        if a.x
                            .total_cmp(&b.x)
                            .then(a.y.total_cmp(&b.y))
                            .then(a.z.total_cmp(&b.z))
                            == std::cmp::Ordering::Less
                        {
                            min_idx_fwd = i;
                        }
                    }
                    let mut h_fwd = DefaultHasher::new();
                    for i in 0..=unique_len {
                        let idx = (min_idx_fwd + i) % unique_len;
                        let c = &ring[idx];
                        c.x.to_bits().hash(&mut h_fwd);
                        c.y.to_bits().hash(&mut h_fwd);
                        c.z.to_bits().hash(&mut h_fwd);
                    }

                    // compute backward minimum
                    let mut min_idx_rev = 0;
                    let mut rev_ring = ring[0..unique_len].to_vec();
                    rev_ring.reverse();
                    for i in 1..unique_len {
                        let a = &rev_ring[i];
                        let b = &rev_ring[min_idx_rev];
                        if a.x
                            .total_cmp(&b.x)
                            .then(a.y.total_cmp(&b.y))
                            .then(a.z.total_cmp(&b.z))
                            == std::cmp::Ordering::Less
                        {
                            min_idx_rev = i;
                        }
                    }
                    let mut h_rev = DefaultHasher::new();
                    for i in 0..=unique_len {
                        let idx = (min_idx_rev + i) % unique_len;
                        let c = &rev_ring[idx];
                        c.x.to_bits().hash(&mut h_rev);
                        c.y.to_bits().hash(&mut h_rev);
                        c.z.to_bits().hash(&mut h_rev);
                    }

                    std::cmp::min(h_fwd.finish(), h_rev.finish())
                }

                for poly in result_polygons {
                    let mut hasher = DefaultHasher::new();

                    let ext_hash = hash_ring(&poly.exterior);
                    ext_hash.hash(&mut hasher);

                    let mut interior_hashes = Vec::new();
                    for interior in &poly.interiors {
                        interior_hashes.push(hash_ring(interior));
                    }

                    interior_hashes.sort_unstable();
                    for h in interior_hashes {
                        h.hash(&mut hasher);
                    }

                    let hash_val = hasher.finish();
                    if seen_hashes.insert(hash_val) {
                        unique_polygons.push(poly);
                    }
                }

                unique_polygons
            }
        };
        let mut dangles = Vec::new();
        let mut cut_edges = Vec::new();
        let mut invalid_rings = Vec::new();
        Ok(apply_determinism(
            polygons,
            &mut dangles,
            &mut cut_edges,
            &mut invalid_rings,
            &self.options,
        ))
    }

    fn validate(&self) -> Result<()> {
        self.options.validate()?;
        if !self.tile_size.is_finite() || self.tile_size <= 0.0 {
            return Err(PolygonizeError::InvalidArgumentType {
                field: "tile_size".to_string(),
                expected: "a finite positive number".to_string(),
                actual: self.tile_size.to_string(),
            });
        }
        if !self.buffer.is_finite() || self.buffer < 0.0 {
            return Err(PolygonizeError::InvalidArgumentType {
                field: "buffer".to_string(),
                expected: "a finite non-negative number".to_string(),
                actual: self.buffer.to_string(),
            });
        }
        let min = self.bbox.min();
        let max = self.bbox.max();
        if ![min.x, min.y, max.x, max.y].iter().all(|v| v.is_finite())
            || min.x >= max.x
            || min.y >= max.y
        {
            return Err(PolygonizeError::InvalidGeometry {
                reason: "tile bounding box must be finite with positive width and height"
                    .to_string(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tiling_tests.rs"]
mod tests;
