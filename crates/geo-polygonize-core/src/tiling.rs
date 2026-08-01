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
use geo_types::{Coord, Geometry, LineString, Point, Rect};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

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

/// Input geometry that reaches an internal buffered-tile boundary.
///
/// This is conservative evidence that topology may continue through linework
/// outside the halo, including when no local face was reconstructed.
#[derive(Clone, Debug)]
pub struct TileInputBoundaryIssue {
    pub input_geometry_index: usize,
    pub geometry_bbox: Rect<f64>,
    pub unresolved_sides: Vec<TileBoundarySide>,
}

/// An exact-endpoint-connected input component excluded from a tile halo.
///
/// The component envelope intersects the buffered tile, but none of its member
/// geometry envelopes do. This is conservative evidence, not proof that the
/// component contains a face.
#[derive(Clone, Debug)]
pub struct TileExcludedComponentIssue {
    pub input_geometry_indices: Vec<usize>,
    pub component_bbox: Rect<f64>,
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
    /// Inputs that may connect to linework beyond this tile's halo.
    pub input_boundary_issues: Vec<TileInputBoundaryIssue>,
    /// Exact-endpoint-connected components excluded from this tile's halo.
    pub excluded_component_issues: Vec<TileExcludedComponentIssue>,
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
    pub unresolved_input_tile_count: usize,
    /// Input-boundary issue instances across tiles; a geometry may occur more than once.
    pub unresolved_input_geometry_count: usize,
    pub unresolved_component_tile_count: usize,
    /// Excluded-component issue instances across tiles; a component may occur more than once.
    pub unresolved_component_count: usize,
}

/// Experimental tiled output with per-tile and merge diagnostics.
#[derive(Debug)]
pub struct TiledPolygonizeResult {
    pub polygons: Vec<Polygon3D>,
    pub tile_reports: Vec<TileReport>,
    pub stitching_report: StitchingReport,
}

/// Coverage contract requested for experimental tiled polygonization.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum TileCoverageGuarantee {
    /// Return output with any detected coverage issues in the tile reports.
    #[default]
    BestEffort,
    /// Reject output when an owned face reaches an internal buffered-tile boundary.
    ///
    /// This validates reconstructed owned faces only. It cannot detect a region
    /// that is absent because its closing linework fell outside every tile halo.
    ValidateOwnedFaces,
    /// Reject output when either owned-face or input-boundary evidence is present.
    ///
    /// This validates the observed evidence only. It does not certify connected
    /// regions whose geometry never intersected a tile halo.
    ValidateObservedCoverage,
}

/// Failure from experimental tiled polygonization with coverage validation.
#[derive(Debug, Error)]
pub enum TiledPolygonizeError {
    #[error(transparent)]
    Polygonize(#[from] PolygonizeError),
    #[error(
        "tiled coverage validation failed for {unresolved_owned_polygon_count} owned polygons and {unresolved_input_geometry_count} input boundary instances"
    )]
    CoverageIncomplete {
        unresolved_tile_count: usize,
        unresolved_owned_polygon_count: usize,
        unresolved_input_tile_count: usize,
        unresolved_input_geometry_count: usize,
        tile_reports: Vec<TileReport>,
    },
}

/// Experimental tiled output paired with a bounded topology trace.
#[derive(Debug)]
pub struct TracedTiledPolygonizeResultV1 {
    pub result: TiledPolygonizeResult,
    pub trace: TopologyTraceV1,
}

type TileOwnershipDecision = (usize, Option<Coord3D>, bool);
type TileProcessResult = (Vec<Polygon3D>, TileReport, Vec<TileOwnershipDecision>, bool);

#[derive(Debug)]
struct EndpointComponent {
    input_geometry_indices: Vec<usize>,
    bbox: Rect<f64>,
}

fn endpoint_bits(value: f64) -> u64 {
    if value == 0.0 {
        0.0f64.to_bits()
    } else {
        value.to_bits()
    }
}

fn line_string_endpoints(line: &LineString<f64>, endpoints: &mut Vec<Coord<f64>>) {
    if let Some(first) = line.0.first() {
        endpoints.push(*first);
    }
    if let Some(last) = line.0.last() {
        endpoints.push(*last);
    }
}

fn geometry_endpoints(geometry: &Geometry<f64>, endpoints: &mut Vec<Coord<f64>>) {
    match geometry {
        Geometry::LineString(line) => line_string_endpoints(line, endpoints),
        Geometry::MultiLineString(lines) => {
            for line in lines {
                line_string_endpoints(line, endpoints);
            }
        }
        Geometry::Polygon(polygon) => {
            line_string_endpoints(polygon.exterior(), endpoints);
            for ring in polygon.interiors() {
                line_string_endpoints(ring, endpoints);
            }
        }
        Geometry::MultiPolygon(polygons) => {
            for polygon in polygons {
                line_string_endpoints(polygon.exterior(), endpoints);
                for ring in polygon.interiors() {
                    line_string_endpoints(ring, endpoints);
                }
            }
        }
        Geometry::GeometryCollection(collection) => {
            for member in collection {
                geometry_endpoints(member, endpoints);
            }
        }
        _ => {}
    }
}

fn component_root(parents: &mut [usize], index: usize) -> usize {
    let mut root = index;
    while parents[root] != root {
        root = parents[root];
    }
    let mut current = index;
    while parents[current] != current {
        let next = parents[current];
        parents[current] = root;
        current = next;
    }
    root
}

fn join_components(parents: &mut [usize], left: usize, right: usize) {
    let left = component_root(parents, left);
    let right = component_root(parents, right);
    if left != right {
        parents[left.max(right)] = left.min(right);
    }
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

    fn process_tile(
        &self,
        tile_bbox: Rect<f64>,
        endpoint_components: &[EndpointComponent],
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
        let mut input_boundary_issues = Vec::new();
        for (input_geometry_index, (geom, bbox)) in self.geometries.iter().enumerate() {
            if let Some(geometry_bbox) = bbox
                .as_ref()
                .filter(|geometry_bbox| geometry_bbox.intersects(&buffered_bbox))
            {
                local_poly.add_borrowed_geometry(geom);
                relevant_lines += 1;
                let unresolved_sides = self.unresolved_sides(*geometry_bbox, buffered_bbox);
                if !unresolved_sides.is_empty() {
                    input_boundary_issues.push(TileInputBoundaryIssue {
                        input_geometry_index,
                        geometry_bbox: *geometry_bbox,
                        unresolved_sides,
                    });
                }
            }
        }
        let excluded_component_issues = endpoint_components
            .iter()
            .filter(|component| {
                component.bbox.intersects(&buffered_bbox)
                    && component.input_geometry_indices.iter().all(|&index| {
                        self.geometries[index]
                            .1
                            .is_none_or(|bbox| !bbox.intersects(&buffered_bbox))
                    })
            })
            .map(|component| TileExcludedComponentIssue {
                input_geometry_indices: component.input_geometry_indices.clone(),
                component_bbox: component.bbox,
            })
            .collect();

        let mut report = TileReport {
            tile_bbox,
            input_geometry_count: relevant_lines,
            polygon_count: 0,
            owned_polygon_count: 0,
            dangle_count: 0,
            cut_edge_count: 0,
            invalid_ring_count: 0,
            coverage_issues: Vec::new(),
            input_boundary_issues,
            excluded_component_issues,
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
        let unresolved_sides = self.unresolved_sides(polygon_bbox, buffered_bbox);
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

    fn unresolved_sides(
        &self,
        geometry_bbox: Rect<f64>,
        buffered_bbox: Rect<f64>,
    ) -> Vec<TileBoundarySide> {
        let mut unresolved_sides = Vec::new();
        if buffered_bbox.min().x > self.bbox.min().x
            && geometry_bbox.min().x <= buffered_bbox.min().x
        {
            unresolved_sides.push(TileBoundarySide::MinX);
        }
        if buffered_bbox.max().x < self.bbox.max().x
            && geometry_bbox.max().x >= buffered_bbox.max().x
        {
            unresolved_sides.push(TileBoundarySide::MaxX);
        }
        if buffered_bbox.min().y > self.bbox.min().y
            && geometry_bbox.min().y <= buffered_bbox.min().y
        {
            unresolved_sides.push(TileBoundarySide::MinY);
        }
        if buffered_bbox.max().y < self.bbox.max().y
            && geometry_bbox.max().y >= buffered_bbox.max().y
        {
            unresolved_sides.push(TileBoundarySide::MaxY);
        }
        unresolved_sides
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

    fn endpoint_components(&self) -> Vec<EndpointComponent> {
        let mut parents = (0..self.geometries.len()).collect::<Vec<_>>();
        let mut endpoint_owners = HashMap::new();
        let mut endpoints = Vec::new();
        for (geometry_index, (geometry, _)) in self.geometries.iter().enumerate() {
            endpoints.clear();
            geometry_endpoints(geometry, &mut endpoints);
            for endpoint in &endpoints {
                let key = (endpoint_bits(endpoint.x), endpoint_bits(endpoint.y));
                if let Some(previous) = endpoint_owners.insert(key, geometry_index) {
                    join_components(&mut parents, previous, geometry_index);
                }
            }
        }

        let mut members = HashMap::<usize, Vec<usize>>::new();
        for geometry_index in 0..self.geometries.len() {
            let root = component_root(&mut parents, geometry_index);
            members.entry(root).or_default().push(geometry_index);
        }
        let mut components = members
            .into_values()
            .filter(|indices| indices.len() > 1)
            .filter_map(|input_geometry_indices| {
                let mut bounds = input_geometry_indices
                    .iter()
                    .filter_map(|&index| self.geometries[index].1);
                let first = bounds.next()?;
                let bbox = bounds.fold(first, |bbox, next| {
                    Rect::new(
                        Coord {
                            x: bbox.min().x.min(next.min().x),
                            y: bbox.min().y.min(next.min().y),
                        },
                        Coord {
                            x: bbox.max().x.max(next.max().x),
                            y: bbox.max().y.max(next.max().y),
                        },
                    )
                });
                Some(EndpointComponent {
                    input_geometry_indices,
                    bbox,
                })
            })
            .collect::<Vec<_>>();
        components.sort_unstable_by_key(|component| component.input_geometry_indices[0]);
        components
    }

    pub fn polygonize(&self) -> Result<TiledPolygonizeResult> {
        self.polygonize_impl(None)
    }

    pub fn polygonize_with_coverage_guarantee(
        &self,
        guarantee: TileCoverageGuarantee,
    ) -> std::result::Result<TiledPolygonizeResult, TiledPolygonizeError> {
        let result = self.polygonize_impl(None)?;
        let reject = match guarantee {
            TileCoverageGuarantee::BestEffort => false,
            TileCoverageGuarantee::ValidateOwnedFaces => {
                result.stitching_report.unresolved_owned_polygon_count != 0
            }
            TileCoverageGuarantee::ValidateObservedCoverage => {
                result.stitching_report.unresolved_owned_polygon_count != 0
                    || result.stitching_report.unresolved_input_geometry_count != 0
            }
        };
        if reject {
            return Err(TiledPolygonizeError::CoverageIncomplete {
                unresolved_tile_count: result.stitching_report.unresolved_tile_count,
                unresolved_owned_polygon_count: result
                    .stitching_report
                    .unresolved_owned_polygon_count,
                unresolved_input_tile_count: result.stitching_report.unresolved_input_tile_count,
                unresolved_input_geometry_count: result
                    .stitching_report
                    .unresolved_input_geometry_count,
                tile_reports: result.tile_reports,
            });
        }
        Ok(result)
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
        trace: Option<&mut TraceRecorderV1>,
    ) -> Result<TiledPolygonizeResult> {
        self.validate()?;
        let tiles = self.generate_tiles();
        let endpoint_components = self.endpoint_components();
        self.polygonize_tiles(tiles, &endpoint_components, trace)
    }

    fn polygonize_tiles(
        &self,
        tiles: Vec<Rect<f64>>,
        endpoint_components: &[EndpointComponent],
        mut trace: Option<&mut TraceRecorderV1>,
    ) -> Result<TiledPolygonizeResult> {
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
                    self.process_tile(tile, endpoint_components, capture_byte_limit)?;
                let trace = trace.as_deref_mut().expect("tile trace exists");
                for issue in &report.input_boundary_issues {
                    if !trace.record_tile_input_boundary(tile_index, issue)? {
                        break;
                    }
                }
                for issue in &report.coverage_issues {
                    if !trace.record_tile_owned_face_boundary(tile_index, issue)? {
                        break;
                    }
                }
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
                .map(|tile| self.process_tile(tile, endpoint_components, None))
                .collect();
            #[cfg(not(feature = "parallel"))]
            let tile_results: Vec<_> = tiles
                .into_iter()
                .map(|tile| self.process_tile(tile, endpoint_components, None))
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
        let unresolved_input_tile_count = tile_reports
            .iter()
            .filter(|report| !report.input_boundary_issues.is_empty())
            .count();
        let unresolved_input_geometry_count = tile_reports.iter().fold(0usize, |total, report| {
            total.saturating_add(report.input_boundary_issues.len())
        });
        let unresolved_component_tile_count = tile_reports
            .iter()
            .filter(|report| !report.excluded_component_issues.is_empty())
            .count();
        let unresolved_component_count = tile_reports.iter().fold(0usize, |total, report| {
            total.saturating_add(report.excluded_component_issues.len())
        });
        Ok(TiledPolygonizeResult {
            polygons,
            tile_reports,
            stitching_report: StitchingReport {
                merged_polygon_count,
                duplicate_polygon_count: merged_polygon_count - output_polygon_count,
                output_polygon_count,
                unresolved_tile_count,
                unresolved_owned_polygon_count,
                unresolved_input_tile_count,
                unresolved_input_geometry_count,
                unresolved_component_tile_count,
                unresolved_component_count,
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
