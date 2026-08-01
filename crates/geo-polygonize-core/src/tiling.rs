use crate::options::{DedupPolicy, ExecutionPolicy, TileOwnershipPolicy};
use crate::polygonizer::{apply_determinism, canonicalize_ring};
use crate::trace::{
    TopologyTraceV1, TraceByteLimitsV1, TraceCaptureBudget, TraceLevelV1, TraceRecorderV1,
    TraceStageV1,
};
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

/// An internal buffered-tile boundary reached by an owned face.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileBoundarySide {
    MinX,
    MaxX,
    MinY,
    MaxY,
}

/// Evidence that an owned face extends to an unresolved buffered-tile boundary.
#[derive(Clone, Debug)]
pub struct TileCoverageIssue {
    pub polygon_index: usize,
    pub polygon_bbox: Rect<f64>,
    pub unresolved_sides: Vec<TileBoundarySide>,
    pub representative_source_line_ids: Vec<u32>,
    /// Complete source IDs when aggregate provenance was requested.
    pub aggregate_source_line_ids: Vec<u32>,
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
    /// Definite halo insufficiency observed for owned faces in this tile.
    pub coverage_issues: Vec<TileCoverageIssue>,
}

/// Counts from merging and deduplicating owned tile polygons.
///
/// These counts do not certify that the configured buffer was sufficient.
#[derive(Debug)]
pub struct StitchingReport {
    pub merged_polygon_count: usize,
    pub duplicate_polygon_count: usize,
    pub output_polygon_count: usize,
    pub unresolved_tile_count: usize,
    pub unresolved_owned_polygon_count: usize,
}

/// Experimental tiled output with per-tile and merge diagnostics.
#[derive(Debug)]
pub struct TiledPolygonizeResult {
    pub polygons: Vec<Polygon3D>,
    pub tile_reports: Vec<TileReport>,
    pub stitching_report: StitchingReport,
}

/// Experimental tiled output paired with a bounded topology trace.
#[derive(Debug)]
pub struct TracedTiledPolygonizeResultV1 {
    pub result: TiledPolygonizeResult,
    pub trace: TopologyTraceV1,
}

type TileOwnershipDecision = (usize, Option<Coord3D>, bool);
type TileProcessResult = (Vec<Polygon3D>, TileReport, Vec<TileOwnershipDecision>, bool);

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

    fn process_tile(
        &self,
        tile_bbox: Rect<f64>,
        capture_byte_limit: Option<usize>,
    ) -> Result<TileProcessResult> {
        let mut capture_budget = capture_byte_limit.map(TraceCaptureBudget::new);
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
            coverage_issues: Vec::new(),
        };
        if relevant_lines == 0 {
            return Ok((Vec::new(), report, Vec::new(), false));
        }

        // Run polygonization
        let result = local_poly.polygonize()?;
        report.polygon_count = result.polygons.len();
        report.dangle_count = result.dangles.len();
        report.cut_edge_count = result.cut_edges.len();
        report.invalid_ring_count = result.invalid_rings.len();
        // Ownership check:
        let mut valid_polys = Vec::new();
        let mut ownership_decisions = Vec::new();
        for (polygon_index, poly) in result.polygons.into_iter().enumerate() {
            let ownership_point = self.ownership_point(&poly);
            let owned = ownership_point.is_some_and(|c| {
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
                in_x && in_y
            });
            if let Some(budget) = capture_budget.as_mut() {
                budget.capture(
                    &mut ownership_decisions,
                    (
                        polygon_index,
                        ownership_point.map(|point| Coord3D::new(point.x(), point.y(), 0.0)),
                        owned,
                    ),
                );
            }
            if owned {
                if let Some(issue) = self.coverage_issue(polygon_index, &poly, buffered_bbox) {
                    report.coverage_issues.push(issue);
                }
                valid_polys.push(poly);
            }
        }
        report.owned_polygon_count = valid_polys.len();
        Ok((
            valid_polys,
            report,
            ownership_decisions,
            capture_budget.is_some_and(|budget| budget.truncated()),
        ))
    }

    fn ownership_point(&self, poly: &Polygon3D) -> Option<Point<f64>> {
        match self.ownership_policy {
            TileOwnershipPolicy::Centroid => poly.centroid_2d(),
            TileOwnershipPolicy::RepresentativePointInsidePolygon => {
                poly.to_polygon_2d().interior_point()
            }
            TileOwnershipPolicy::LexicographicMinVertex => poly
                .exterior
                .iter()
                .min_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)))
                .map(|coord| Point::new(coord.x, coord.y)),
        }
    }

    fn coverage_issue(
        &self,
        polygon_index: usize,
        poly: &Polygon3D,
        buffered_bbox: Rect<f64>,
    ) -> Option<TileCoverageIssue> {
        let first = poly.exterior.first()?;
        let mut min_x = first.x;
        let mut min_y = first.y;
        let mut max_x = first.x;
        let mut max_y = first.y;
        for coordinate in &poly.exterior[1..] {
            min_x = min_x.min(coordinate.x);
            min_y = min_y.min(coordinate.y);
            max_x = max_x.max(coordinate.x);
            max_y = max_y.max(coordinate.y);
        }
        let polygon_bbox = Rect::new(Coord { x: min_x, y: min_y }, Coord { x: max_x, y: max_y });
        let mut unresolved_sides = Vec::new();
        if buffered_bbox.min().x > self.bbox.min().x && min_x <= buffered_bbox.min().x {
            unresolved_sides.push(TileBoundarySide::MinX);
        }
        if buffered_bbox.max().x < self.bbox.max().x && max_x >= buffered_bbox.max().x {
            unresolved_sides.push(TileBoundarySide::MaxX);
        }
        if buffered_bbox.min().y > self.bbox.min().y && min_y <= buffered_bbox.min().y {
            unresolved_sides.push(TileBoundarySide::MinY);
        }
        if buffered_bbox.max().y < self.bbox.max().y && max_y >= buffered_bbox.max().y {
            unresolved_sides.push(TileBoundarySide::MaxY);
        }
        if unresolved_sides.is_empty() {
            return None;
        }
        let mut representative_source_line_ids = poly
            .exterior_ids
            .iter()
            .chain(poly.interiors_ids.iter().flatten())
            .copied()
            .collect::<Vec<_>>();
        representative_source_line_ids.sort_unstable();
        representative_source_line_ids.dedup();
        Some(TileCoverageIssue {
            polygon_index,
            polygon_bbox,
            unresolved_sides,
            representative_source_line_ids,
            aggregate_source_line_ids: poly.boundary_source_line_ids.clone(),
        })
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
        self.polygonize_impl(None)
    }

    pub fn polygonize_with_trace(
        &self,
        level: TraceLevelV1,
        byte_limit: usize,
    ) -> Result<TracedTiledPolygonizeResultV1> {
        self.polygonize_with_trace_limits(level, TraceByteLimitsV1::total(byte_limit))
    }

    pub fn polygonize_with_trace_limits(
        &self,
        level: TraceLevelV1,
        limits: TraceByteLimitsV1,
    ) -> Result<TracedTiledPolygonizeResultV1> {
        let mut trace = TraceRecorderV1::new_with_limits(Some(level), limits, &self.options)
            .expect("trace enabled");
        let result = self.polygonize_impl(Some(&mut trace))?;
        Ok(TracedTiledPolygonizeResultV1 {
            result,
            trace: trace.finish(),
        })
    }

    fn polygonize_impl(
        &self,
        mut trace: Option<&mut TraceRecorderV1>,
    ) -> Result<TiledPolygonizeResult> {
        self.validate()?;
        let tiles = self.generate_tiles();
        let trace_ownership = trace
            .as_ref()
            .is_some_and(|trace| trace.records_stage(TraceStageV1::Output));

        let mut tile_polygons = Vec::with_capacity(tiles.len());
        let mut tile_reports = Vec::with_capacity(tiles.len());
        if trace_ownership {
            for (tile_index, tile) in tiles.into_iter().enumerate() {
                let capture_byte_limit = trace.as_ref().and_then(|trace| {
                    trace
                        .records_stage(TraceStageV1::Output)
                        .then(|| trace.capture_byte_limit(TraceStageV1::Output))
                });
                let (polygons, report, ownership_decisions, capture_truncated) =
                    self.process_tile(tile, capture_byte_limit)?;
                let trace = trace.as_deref_mut().expect("tile trace exists");
                for (polygon_index, ownership_point, owned) in ownership_decisions {
                    trace.record_tile_ownership(
                        tile_index,
                        polygon_index,
                        ownership_point,
                        owned,
                    )?;
                }
                if capture_truncated {
                    trace.mark_capture_truncated(TraceStageV1::Output);
                }
                tile_polygons.push(polygons);
                tile_reports.push(report);
            }
        } else {
            #[cfg(feature = "parallel")]
            let tile_results: Vec<_> = tiles
                .into_par_iter()
                .map(|tile| self.process_tile(tile, None))
                .collect();
            #[cfg(not(feature = "parallel"))]
            let tile_results: Vec<_> = tiles
                .into_iter()
                .map(|tile| self.process_tile(tile, None))
                .collect();

            for result in tile_results {
                let (polygons, report, _, _) = result?;
                tile_polygons.push(polygons);
                tile_reports.push(report);
            }
        }
        let result_polygons: Vec<Polygon3D> = tile_polygons.into_iter().flatten().collect();
        let merged_polygon_count = result_polygons.len();

        let polygons = match self.dedup_policy {
            DedupPolicy::KeepAll => {
                if let Some(trace) = trace.as_deref_mut() {
                    for polygon_index in 0..result_polygons.len() {
                        trace.record_tile_dedup(polygon_index, true);
                    }
                }
                result_polygons
            }
            DedupPolicy::CanonicalRingHash => {
                let mut unique_polygons = Vec::new();
                let mut seen = HashSet::new();

                for (polygon_index, poly) in result_polygons.into_iter().enumerate() {
                    let retained = seen.insert(canonical_polygon_key(&poly));
                    if let Some(trace) = trace.as_deref_mut() {
                        trace.record_tile_dedup(polygon_index, retained);
                    }
                    if retained {
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
            &ExecutionPolicy::default(),
            None,
        )
        .expect("default execution policy cannot cancel");
        let output_polygon_count = polygons.len();
        let unresolved_tile_count = tile_reports
            .iter()
            .filter(|report| !report.coverage_issues.is_empty())
            .count();
        let unresolved_owned_polygon_count = tile_reports
            .iter()
            .map(|report| report.coverage_issues.len())
            .sum();
        Ok(TiledPolygonizeResult {
            polygons,
            tile_reports,
            stitching_report: StitchingReport {
                merged_polygon_count,
                duplicate_polygon_count: merged_polygon_count - output_polygon_count,
                output_polygon_count,
                unresolved_tile_count,
                unresolved_owned_polygon_count,
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
