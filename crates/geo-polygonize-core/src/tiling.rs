use crate::options::{DedupPolicy, TileOwnershipPolicy};
use crate::polygonizer::{apply_determinism, canonicalize_ring};
use crate::types::{Coord3D, Polygon3D};
use crate::{PolygonizeError, Polygonizer, PolygonizerOptions, Result};
use geo::bounding_rect::BoundingRect;
use geo::intersects::Intersects;
use geo::InteriorPoint;
use geo_types::{Coord, Geometry, Point, Rect};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::collections::HashSet;

fn canonical_ring_key(ring: &[Coord3D]) -> Vec<[u64; 3]> {
    let key = |mut ring: Vec<Coord3D>| {
        canonicalize_ring(&mut ring, None);
        ring.into_iter()
            .map(|coord| [coord.x.to_bits(), coord.y.to_bits(), coord.z.to_bits()])
            .collect::<Vec<_>>()
    };
    key(ring.to_vec()).min(key(ring.iter().rev().copied().collect()))
}

fn canonical_polygon_key(poly: &Polygon3D) -> (Vec<[u64; 3]>, Vec<Vec<[u64; 3]>>) {
    let mut interiors = poly
        .interiors
        .iter()
        .map(|ring| canonical_ring_key(ring))
        .collect::<Vec<_>>();
    interiors.sort_unstable();
    (canonical_ring_key(&poly.exterior), interiors)
}

/// Observed work and topology output for one tile.
#[derive(Debug)]
pub struct TileReport {
    pub tile_bbox: Rect<f64>,
    /// Geometries whose bounds intersected the buffered tile.
    pub input_geometry_count: usize,
    /// Polygons produced before tile ownership filtering.
    pub polygon_count: usize,
    pub owned_polygon_count: usize,
    pub dangle_count: usize,
    pub cut_edge_count: usize,
    pub invalid_ring_count: usize,
}

/// Counts from merging and deduplicating owned tile polygons.
///
/// These counts do not certify that the configured buffer was sufficient.
#[derive(Debug)]
pub struct StitchingReport {
    pub merged_polygon_count: usize,
    pub duplicate_polygon_count: usize,
    pub output_polygon_count: usize,
}

/// Experimental tiled output with per-tile and merge diagnostics.
#[derive(Debug)]
pub struct TiledPolygonizeResult {
    pub polygons: Vec<Polygon3D>,
    pub tile_reports: Vec<TileReport>,
    pub stitching_report: StitchingReport,
}

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

    fn process_tile(&self, tile_bbox: Rect<f64>) -> Result<(Vec<Polygon3D>, TileReport)> {
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

        let mut report = TileReport {
            tile_bbox,
            input_geometry_count: relevant_lines,
            polygon_count: 0,
            owned_polygon_count: 0,
            dangle_count: 0,
            cut_edge_count: 0,
            invalid_ring_count: 0,
        };
        if relevant_lines == 0 {
            return Ok((Vec::new(), report));
        }

        // Run polygonization
        let result = local_poly.polygonize()?;
        report.polygon_count = result.polygons.len();
        report.dangle_count = result.dangles.len();
        report.cut_edge_count = result.cut_edges.len();
        report.invalid_ring_count = result.invalid_rings.len();
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
        report.owned_polygon_count = valid_polys.len();
        Ok((valid_polys, report))
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

    pub fn polygonize(&self) -> Result<TiledPolygonizeResult> {
        self.validate()?;
        let tiles = self.generate_tiles();

        // Process tiles in parallel or sequential
        let tile_results: Vec<Result<(Vec<Polygon3D>, TileReport)>>;
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
        let (tile_polygons, tile_reports): (Vec<_>, Vec<_>) = tile_results
            .into_iter()
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip();
        let result_polygons: Vec<Polygon3D> = tile_polygons.into_iter().flatten().collect();
        let merged_polygon_count = result_polygons.len();

        let polygons = match self.dedup_policy {
            DedupPolicy::KeepAll => result_polygons,
            DedupPolicy::CanonicalRingHash => {
                let mut unique_polygons = Vec::new();
                let mut seen = HashSet::new();

                for poly in result_polygons {
                    if seen.insert(canonical_polygon_key(&poly)) {
                        unique_polygons.push(poly);
                    }
                }

                unique_polygons
            }
        };
        let mut dangles = Vec::new();
        let mut cut_edges = Vec::new();
        let mut invalid_rings = Vec::new();
        let polygons = apply_determinism(
            polygons,
            &mut dangles,
            &mut cut_edges,
            &mut invalid_rings,
            &self.options,
        );
        let output_polygon_count = polygons.len();
        Ok(TiledPolygonizeResult {
            polygons,
            tile_reports,
            stitching_report: StitchingReport {
                merged_polygon_count,
                duplicate_polygon_count: merged_polygon_count - output_polygon_count,
                output_polygon_count,
            },
        })
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
