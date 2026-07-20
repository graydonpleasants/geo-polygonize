use crate::containment::ContainmentForest;
use crate::diagnostics::{ContainmentStats, PolygonizerDiagnostics};
use crate::error::{PolygonizeError, Result};
use crate::graph::{ExtractedRing, PlanarGraph};
use crate::noding::advanced::AdvancedNoder;
use crate::noding::snap::SnapNoder;
use crate::noding::validate::ValidatingNoder;
use crate::options::{NodingGuarantee, PolygonizerOptions, PrecisionModel, SnapStrategy};
use crate::types::{Coord3D, Line3D, Polygon3D, RingGraphIdentity};
use crate::utils::simd::SimdRing;
use crate::utils::z_order_index;
use float_next_after::NextAfter;
use geo::Contains;
use geo_types::{Coord, Geometry, Polygon};
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

fn get_time() -> Instant {
    Instant::now()
}

fn get_elapsed(start: Instant) -> std::time::Duration {
    start.elapsed()
}

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// A robust polygonizer that reconstructs polygons from a set of lines (3D supported).
pub struct Polygonizer {
    graph: PlanarGraph,
    options: PolygonizerOptions,
    input_lines: Vec<Line3D>,
}

/// Reusable allocation storage for stateless polygonization calls.
///
/// Geometry and result state are cleared after every call; only allocation capacity is retained.
#[derive(Default)]
pub struct PolygonizerWorkspace {
    graph: PlanarGraph,
}

impl PolygonizerWorkspace {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PolygonizerResult {
    pub polygons: Vec<Polygon3D>,
    pub dangles: Vec<Vec<Coord3D>>,
    pub cut_edges: Vec<Vec<Coord3D>>,
    pub invalid_rings: Vec<Vec<Coord3D>>,
    pub diagnostics: Option<PolygonizerDiagnostics>,
}

impl Default for Polygonizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Polygonize an owned stream of line segments using canonical options.
pub fn polygonize(
    lines: impl IntoIterator<Item = Line3D>,
    options: &PolygonizerOptions,
) -> Result<PolygonizerResult> {
    Polygonizer::with_options(options.clone()).polygonize_owned(lines.into_iter().collect())
}

/// Polygonize a borrowed slice while reusing graph allocations between calls.
pub fn polygonize_with_workspace(
    lines: &[Line3D],
    options: &PolygonizerOptions,
    workspace: &mut PolygonizerWorkspace,
) -> Result<PolygonizerResult> {
    let mut runner = Polygonizer {
        graph: std::mem::take(&mut workspace.graph),
        options: options.clone(),
        input_lines: Vec::new(),
    };
    let result = runner.polygonize_owned(lines.to_vec());
    runner.graph.clear();
    workspace.graph = runner.graph;
    result
}

/// Compatibility alias for the canonical [`polygonize`] entrypoint.
#[deprecated(note = "use polygonize or polygonize_with_workspace")]
pub fn polygonize_with_options(
    lines: &[Line3D],
    options: &PolygonizerOptions,
) -> Result<PolygonizerResult> {
    polygonize(lines.iter().copied(), options)
}

impl Polygonizer {
    /// Creates a new `Polygonizer` with default configuration.
    pub fn new() -> Self {
        Self {
            graph: PlanarGraph::new(),
            options: PolygonizerOptions::default(),
            input_lines: Vec::new(),
        }
    }
    /// Creates a new `Polygonizer` with specific options.
    pub fn with_options(options: PolygonizerOptions) -> Self {
        Self {
            graph: PlanarGraph::new(),
            options,
            input_lines: Vec::new(),
        }
    }

    pub fn options(&self) -> &PolygonizerOptions {
        &self.options
    }

    pub fn options_mut(&mut self) -> &mut PolygonizerOptions {
        &mut self.options
    }

    /// Compatibility shorthand for setting floating or fixed precision.
    ///
    /// # Arguments
    ///
    /// * `grid_size` - Zero selects floating precision; a positive value selects a fixed grid.
    #[deprecated(note = "use with_precision_model")]
    pub fn with_snap_grid(mut self, grid_size: f64) -> Self {
        self.options.precision_model = PrecisionModel::from_grid_size(grid_size);
        self
    }

    pub fn with_precision_model(mut self, precision_model: PrecisionModel) -> Self {
        self.options.precision_model = precision_model;
        self
    }

    /// Adds a 2D geometry to the graph (Z=0).
    pub fn add_geometry(&mut self, geom: Geometry<f64>) {
        extract_segments(&geom, &mut self.input_lines);
    }

    /// Adds a 2D geometry to the graph (Z=0) from a reference.
    pub fn add_borrowed_geometry(&mut self, geom: &Geometry<f64>) {
        extract_segments(geom, &mut self.input_lines);
    }

    /// Adds explicit 3D lines.
    pub fn add_lines(&mut self, lines: Vec<Line3D>) {
        self.input_lines.extend(lines);
    }

    fn build_graph(
        &mut self,
        mut all_segments: Vec<Line3D>,
        diagnostics: Option<&mut PolygonizerDiagnostics>,
    ) -> Result<()> {
        self.graph.clear();
        let grid_size = self.options.precision_model.grid_size();
        let mut segments;

        if self.options.node_input {
            let mut pre_snap_vertex_candidates = 0;
            if self.options.pre_snap_tolerance > 0.0 {
                let (snapped, candidates) = SnapNoder::pre_snap_to_reference_vertices_with_stats(
                    &all_segments,
                    self.options.pre_snap_tolerance,
                );
                all_segments = snapped;
                pre_snap_vertex_candidates = candidates;
            }

            // Sort by 2D coordinates
            all_segments.sort_unstable_by(|a, b| {
                a.start
                    .x
                    .total_cmp(&b.start.x)
                    .then(a.start.y.total_cmp(&b.start.y))
            });
            // Dedup based on 3D equality? or 2D?
            // SnapNoder will handle dedup.
            all_segments.dedup_by(|a, b| {
                a.start.x == b.start.x && a.start.y == b.start.y
                && a.end.x == b.end.x && a.end.y == b.end.y
                // Ignore Z for initial dedup of "same projected line" if that's what we want?
                // Probably better to keep exact duplicates removed.
                && a.start.z == b.start.z && a.end.z == b.end.z
            });

            // OPTIMIZATION: Spatial Sort (Z-Order 2D)
            let mut numbered_lines: Vec<(u64, Line3D)> = all_segments
                .iter()
                .map(|l| (z_order_index(l.start.to_coord_2d()), *l))
                .collect();

            // Unstable sort is faster and sufficient
            numbered_lines.sort_unstable_by_key(|k| k.0);

            all_segments = numbered_lines.into_iter().map(|k| k.1).collect();

            match self.options.noding.backend {
                crate::options::NodingBackend::Snap => {
                    let noder = SnapNoder::new(grid_size)
                        .with_snap_strategy(self.options.snap_strategy.clone());
                    // ponytail: one source coordinate per snapped node preserves connectivity;
                    // use per-line restoration only if #798 can avoid reopening gaps.
                    let restore_coordinates =
                        matches!(self.options.snap_strategy, SnapStrategy::GeosCompat)
                            .then(|| restored_coordinates(&noder, &all_segments));
                    if let Some(diagnostics) = diagnostics {
                        let (noded, stats, mut work_stats) = noder.node_with_stats(all_segments);
                        work_stats.pre_snap_vertex_candidates = pre_snap_vertex_candidates;
                        diagnostics.intersection_stats.interpolated_intersections = stats
                            .iter()
                            .map(|iteration| iteration.intersections_found)
                            .sum();
                        diagnostics.noding_iterations = stats;
                        diagnostics.intersection_stats.exact_intersections =
                            work_stats.exact_intersection_calls;
                        diagnostics.noding_work_stats = work_stats;
                        segments = noded;
                    } else {
                        segments = noder.node(all_segments);
                    }
                    if let Some(coordinates) = restore_coordinates {
                        restore_noded_coordinates(&mut segments, &coordinates);
                    }
                }
                crate::options::NodingBackend::Advanced => {
                    let noder = AdvancedNoder::new();
                    if let Some(diagnostics) = diagnostics {
                        let (noded, stats, mut work_stats) =
                            SnapNoder::new(0.0).node_with_stats(all_segments);
                        work_stats.pre_snap_vertex_candidates = pre_snap_vertex_candidates;
                        diagnostics.intersection_stats.interpolated_intersections = stats
                            .iter()
                            .map(|iteration| iteration.intersections_found)
                            .sum();
                        diagnostics.noding_iterations = stats;
                        diagnostics.intersection_stats.exact_intersections =
                            work_stats.exact_intersection_calls;
                        diagnostics.noding_work_stats = work_stats;
                        segments = noded;
                    } else {
                        segments = noder.node(all_segments);
                    }
                }
            }
        } else {
            if grid_size > 0.0 {
                let snapper = SnapNoder::new(grid_size)
                    .with_snap_strategy(self.options.snap_strategy.clone());
                for line in &mut all_segments {
                    line.start = snapper.snap(line.start);
                    line.end = snapper.snap(line.end);
                }
            }
            segments = all_segments;
        }

        if matches!(self.options.noding.guarantee, NodingGuarantee::Validate) {
            ValidatingNoder::new().validate(&segments)?;
        }

        // Use bulk load
        self.graph.bulk_load(segments);

        Ok(())
    }

    /// Computes the polygons.
    /// This is the main entry point.
    ///
    /// Returns a `PolygonizerResult` containing polygons and dangles.
    pub fn polygonize(&mut self) -> Result<PolygonizerResult> {
        self.polygonize_owned(self.input_lines.clone())
    }

    fn polygonize_owned(&mut self, input_lines: Vec<Line3D>) -> Result<PolygonizerResult> {
        self.options.validate()?;
        validate_lines(&input_lines)?;

        let mut diag = if self.options.diagnostics.enabled || self.options.diagnostics.timings {
            let d = PolygonizerDiagnostics {
                input_segment_count: input_lines.len(),
                ..Default::default()
            };
            Some(d)
        } else {
            None
        };

        let t_ingest_start = get_time();
        self.build_graph(
            input_lines,
            if self.options.diagnostics.enabled {
                diag.as_mut()
            } else {
                None
            },
        )?;
        if let Some(ref mut d) = diag {
            d.phase_times.ingest_and_node = get_elapsed(t_ingest_start);
            d.noded_segment_count = self.graph.edges.len();
        }

        let t_graph_build_start = get_time();
        // 1. Sort edges (Geometry Graph operation)
        self.graph.sort_edges();

        // 2. Prune dangles
        let mut dangles = self.graph.prune_dangles();

        // 3. Find rings (3D)
        let rings_with_ids = self
            .graph
            .get_edge_rings_with_graph_ids(self.options.node_input);

        // 3b. Find cut edges
        let mut cut_edges = self.graph.get_cut_edges();

        if let Some(ref mut d) = diag {
            d.phase_times.graph_build = get_elapsed(t_graph_build_start);
            d.ring_count = rings_with_ids.len();
            d.cut_edge_count = cut_edges.len();
            d.dangle_count = dangles.len();
        }

        // 4. Classify Rings (Shell vs Hole)
        let t_ring_extraction_start = get_time();
        let (shells, holes, invalid_rings_candidates) =
            extract_and_classify_rings(rings_with_ids, self.options.node_input);

        if let Some(ref mut d) = diag {
            d.phase_times.ring_extraction = get_elapsed(t_ring_extraction_start);
            d.shell_count = shells.len();
            d.hole_count = holes.len();
        }

        let t_containment_start = get_time();
        // 5. Establish Topology
        let (
            shells,
            shell_holes,
            shell_holes_ids,
            unassigned_hole_count,
            unassigned_hole_area,
            containment_stats,
        ) = establish_topology(shells, holes, &self.options);

        // 6. Construct Final Polygons
        // Ensure we don't crash on NaNs during processing
        let mut invalid_rings = if invalid_rings_candidates.is_empty() {
            Vec::new()
        } else {
            let shells_2d: Vec<Polygon<f64>> = shells.iter().map(|s| s.to_polygon_2d()).collect();
            process_invalid_rings(invalid_rings_candidates, &shells_2d)
        };

        let mut result =
            construct_final_polygons(shells, shell_holes, shell_holes_ids, &self.options);

        result = apply_determinism(
            result,
            &mut dangles,
            &mut cut_edges,
            &mut invalid_rings,
            &self.options,
        );

        if let Some(ref mut d) = diag {
            d.phase_times.containment = get_elapsed(t_containment_start);
            d.unassigned_hole_count = unassigned_hole_count;
            d.unassigned_hole_area = unassigned_hole_area;
            d.containment_stats = containment_stats;
            d.invalid_ring_count = invalid_rings.len();
            // output_flatten time could be measured here if we had a separate pass, but we'll leave it 0 or record what we have
        }
        Ok(PolygonizerResult {
            polygons: result,
            dangles,
            cut_edges,
            invalid_rings,
            diagnostics: diag,
        })
    }
}

fn validate_lines(lines: &[Line3D]) -> Result<()> {
    if lines.iter().any(|line| {
        [
            line.start.x,
            line.start.y,
            line.start.z,
            line.end.x,
            line.end.y,
            line.end.z,
        ]
        .iter()
        .any(|value| !value.is_finite())
    }) {
        return Err(PolygonizeError::InvalidGeometry {
            reason: "line coordinates must be finite".to_string(),
        });
    }
    Ok(())
}

fn restored_coordinates(noder: &SnapNoder, lines: &[Line3D]) -> HashMap<(u64, u64), Coord3D> {
    let mut coordinates = HashMap::with_capacity(lines.len() * 2);
    for coord in lines.iter().flat_map(|line| [line.start, line.end]) {
        let snapped = noder.snap(coord);
        let key = coordinate_key(snapped);
        coordinates
            .entry(key)
            .and_modify(|current: &mut Coord3D| {
                let distance = (coord.x - snapped.x).powi(2) + (coord.y - snapped.y).powi(2);
                let current_distance =
                    (current.x - snapped.x).powi(2) + (current.y - snapped.y).powi(2);
                if distance.total_cmp(&current_distance).is_lt()
                    || (distance == current_distance
                        && (coord.x, coord.y, coord.z) < (current.x, current.y, current.z))
                {
                    *current = coord;
                }
            })
            .or_insert(coord);
    }
    coordinates
}

fn coordinate_key(coord: Coord3D) -> (u64, u64) {
    let x = if coord.x == 0.0 { 0.0 } else { coord.x };
    let y = if coord.y == 0.0 { 0.0 } else { coord.y };
    (x.to_bits(), y.to_bits())
}

fn restore_noded_coordinates(lines: &mut [Line3D], coordinates: &HashMap<(u64, u64), Coord3D>) {
    for line in lines {
        line.start = coordinates
            .get(&coordinate_key(line.start))
            .copied()
            .unwrap_or(line.start);
        line.end = coordinates
            .get(&coordinate_key(line.end))
            .copied()
            .unwrap_or(line.end);
    }
}

pub(crate) fn canonicalize_ring(ring: &mut Vec<Coord3D>, mut ids: Option<&mut Vec<u32>>) {
    if ring.is_empty() {
        return;
    }
    let n = if ring.len() > 1 && ring.first() == ring.last() {
        ring.len() - 1
    } else {
        ring.len()
    };

    // Bolt optimization: using `ring[..n].iter().enumerate().min_by(...)` instead of `(0..n).min_by(...)`
    // eliminates bounds-checking overhead while preserving readability.
    let min_idx = ring[..n]
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            a.x.total_cmp(&b.x)
                .then(a.y.total_cmp(&b.y))
                .then(a.z.total_cmp(&b.z))
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    if min_idx > 0 {
        ring[..n].rotate_left(min_idx);
        if n < ring.len() {
            ring[n] = ring[0];
        } else {
            ring.push(ring[0]);
        }

        if let Some(ref mut ids_vec) = ids {
            if !ids_vec.is_empty() {
                ids_vec.rotate_left(min_idx);
            }
        }
    }
}

pub(crate) fn canonicalize_open_line(line: &mut [Coord3D]) {
    if line.len() > 1 {
        let first = line.first().unwrap();
        let last = line.last().unwrap();
        let cmp = last
            .x
            .total_cmp(&first.x)
            .then(last.y.total_cmp(&first.y))
            .then(last.z.total_cmp(&first.z));
        if cmp == std::cmp::Ordering::Less {
            line.reverse();
        }
    }
}

type ClassifiedRing = (Polygon3D, Option<RingGraphIdentity>);

pub(crate) fn extract_and_classify_rings(
    rings_with_ids: Vec<ExtractedRing>,
    include_graph_ids: bool,
) -> (Vec<ClassifiedRing>, Vec<ClassifiedRing>, Vec<Polygon3D>) {
    let mut shells = Vec::with_capacity(rings_with_ids.len() / 2);
    let mut holes = Vec::with_capacity(rings_with_ids.len() / 2);
    let mut invalid_rings_candidates = Vec::new();

    for ring in rings_with_ids {
        let poly3d = Polygon3D::new(ring.coords, vec![], ring.line_ids, vec![]);
        let area = poly3d.signed_area_2d();

        if !has_three_distinct_xy(&poly3d.exterior) || !area.is_finite() || area == 0.0 {
            invalid_rings_candidates.push(poly3d);
            continue;
        }

        let ring = (
            poly3d,
            include_graph_ids.then(|| RingGraphIdentity::new(ring.edge_keys, ring.node_ids)),
        );
        if area > 0.0 {
            shells.push(ring);
        } else {
            holes.push(ring);
        }
    }

    (shells, holes, invalid_rings_candidates)
}

#[allow(clippy::type_complexity)]
fn establish_topology(
    shells: Vec<ClassifiedRing>,
    holes: Vec<ClassifiedRing>,
    options: &PolygonizerOptions,
) -> (
    Vec<Polygon3D>,
    Vec<Vec<Vec<Coord3D>>>,
    Vec<Vec<Vec<u32>>>,
    usize,
    f64,
    ContainmentStats,
) {
    let (mut shells, mut shell_graph_ids): (Vec<_>, Vec<_>) = shells.into_iter().unzip();
    let collect_stats = options.diagnostics.enabled;
    let mut containment_stats = ContainmentStats::default();
    if collect_stats {
        containment_stats.prepared_shells = shells.len();
    }
    let mut forest = ContainmentForest::new_with_graph_ids(&shells, &shell_graph_ids);

    if options.extract_only_polygonal {
        let keep_mask = if collect_stats {
            let (keep_mask, stats) =
                forest.filter_polygonal_with_stats(&shells, &options.containment.touch_policy);
            containment_stats.merge(stats);
            keep_mask
        } else {
            forest.filter_polygonal(&shells, &options.containment.touch_policy)
        };
        let removed_count = keep_mask.iter().filter(|&&keep| !keep).count();

        if removed_count > 0 {
            if collect_stats {
                forest.record_locator_reuse(&mut containment_stats);
            }
            let mut new_shells = Vec::new();
            let mut new_shell_graph_ids = Vec::new();

            for ((keep, shell), graph_ids) in keep_mask.into_iter().zip(shells).zip(shell_graph_ids)
            {
                if keep {
                    new_shells.push(shell);
                    new_shell_graph_ids.push(graph_ids);
                }
            }
            shells = new_shells;
            shell_graph_ids = new_shell_graph_ids;
            if collect_stats {
                containment_stats.prepared_shells += shells.len();
            }
            forest = ContainmentForest::new_with_graph_ids(&shells, &shell_graph_ids);
        }
    }

    let process_hole_assignment = |(hole_3d, graph_ids): (
        Polygon3D,
        Option<RingGraphIdentity>,
    )|
     -> (Option<usize>, Polygon3D, ContainmentStats) {
            let (best_shell_idx, stats) = if collect_stats {
                forest.assign_hole_with_graph_ids_and_stats(
                    &hole_3d,
                    graph_ids.as_ref(),
                    &shells,
                    &options.containment.touch_policy,
                )
            } else {
                (
                    forest.assign_hole_with_graph_ids(
                        &hole_3d,
                        graph_ids.as_ref(),
                        &shells,
                        &options.containment.touch_policy,
                    ),
                    ContainmentStats::default(),
                )
            };
            (best_shell_idx, hole_3d, stats)
        };

    let assignments: Vec<_>;
    #[cfg(feature = "parallel")]
    {
        assignments = holes.into_par_iter().map(process_hole_assignment).collect();
    }
    #[cfg(not(feature = "parallel"))]
    {
        assignments = holes.into_iter().map(process_hole_assignment).collect();
    }

    let mut shell_holes: Vec<Vec<Vec<Coord3D>>> = vec![vec![]; shells.len()];
    let mut shell_holes_ids: Vec<Vec<Vec<u32>>> = vec![vec![]; shells.len()];
    let mut unassigned_hole_count = 0;
    let mut unassigned_hole_area = 0.0;

    for (idx, hole, stats) in assignments {
        containment_stats.merge(stats);
        if let Some(idx) = idx {
            shell_holes[idx].push(hole.exterior);
            shell_holes_ids[idx].push(hole.exterior_ids);
        } else {
            unassigned_hole_count += 1;
            unassigned_hole_area += hole.exterior_unsigned_area_2d();
        }
    }
    if collect_stats {
        forest.record_locator_reuse(&mut containment_stats);
    }

    (
        shells,
        shell_holes,
        shell_holes_ids,
        unassigned_hole_count,
        unassigned_hole_area,
        containment_stats,
    )
}

pub(crate) fn construct_final_polygons(
    shells: Vec<Polygon3D>,
    shell_holes: Vec<Vec<Vec<Coord3D>>>,
    shell_holes_ids: Vec<Vec<Vec<u32>>>,
    options: &PolygonizerOptions,
) -> Vec<Polygon3D> {
    let mut result = Vec::with_capacity(shells.len());
    for ((shell, mut holes), mut holes_ids) in
        shells.into_iter().zip(shell_holes).zip(shell_holes_ids)
    {
        let mut exterior = shell.exterior;
        let mut exterior_ids = shell.exterior_ids;

        if options.determinism.canonical_ring_rotation {
            canonicalize_ring(&mut exterior, Some(&mut exterior_ids));
            for (h, h_ids) in holes.iter_mut().zip(holes_ids.iter_mut()) {
                canonicalize_ring(h, Some(h_ids));
            }
        }

        if options.determinism.canonical_sort {
            let mut combined_holes: Vec<_> = holes
                .into_iter()
                .zip(holes_ids)
                .map(|(h, hi)| {
                    let area = Polygon3D::ring_signed_area_2d(&h).abs();
                    (h, hi, area)
                })
                .collect();

            let use_stable_tie_breaks = options.determinism.stable_tie_breaks;
            if use_stable_tie_breaks {
                let mut combined_holes_with_bbox: Vec<_> = combined_holes
                    .into_iter()
                    .map(|(h, hi, area)| {
                        let b = bounding_rect_3d(&h).unwrap_or(geo::Rect::new(
                            geo::Coord { x: 0.0, y: 0.0 },
                            geo::Coord { x: 0.0, y: 0.0 },
                        ));
                        (h, hi, area, b)
                    })
                    .collect();
                combined_holes_with_bbox.sort_unstable_by(
                    |(h1, _, area1, b1), (h2, _, area2, b2)| {
                        area2.total_cmp(area1).then_with(|| {
                            b1.min()
                                .x
                                .total_cmp(&b2.min().x)
                                .then(b1.min().y.total_cmp(&b2.min().y))
                                .then(h1.len().cmp(&h2.len()))
                        })
                    },
                );

                let mut h = Vec::with_capacity(combined_holes_with_bbox.len());
                let mut hi = Vec::with_capacity(combined_holes_with_bbox.len());
                for (hole, ids, _, _) in combined_holes_with_bbox {
                    h.push(hole);
                    hi.push(ids);
                }
                holes = h;
                holes_ids = hi;
            } else {
                combined_holes
                    .sort_unstable_by(|(_, _, area1), (_, _, area2)| area2.total_cmp(area1));

                let mut h = Vec::with_capacity(combined_holes.len());
                let mut hi = Vec::with_capacity(combined_holes.len());
                for (hole, ids, _) in combined_holes {
                    h.push(hole);
                    hi.push(ids);
                }
                holes = h;
                holes_ids = hi;
            }
        }

        let mut p = Polygon3D::new(exterior, holes, exterior_ids, holes_ids);

        if options.provenance.enabled {
            let mut b_ids = Vec::new();
            if options.provenance.include_boundary_line_ids {
                for &id in &p.exterior_ids {
                    if id != 0 {
                        b_ids.push(id as u64);
                    }
                }
                for hole_ids in &p.interiors_ids {
                    for &id in hole_ids {
                        if id != 0 {
                            b_ids.push(id as u64);
                        }
                    }
                }
                b_ids.sort_unstable();
                b_ids.dedup();
            }

            p.provenance = Some(crate::types::PolygonProvenance {
                boundary_line_ids: b_ids,
                input_profile_id: options.input_profile_id.clone(),
            });
        }

        let area = p.unsigned_area_2d();
        if area.is_finite()
            && area > 0.0
            && options
                .output_filter
                .minimum_face_area
                .is_none_or(|minimum| area >= minimum)
        {
            result.push(p);
        }
    }
    result
}

fn has_three_distinct_xy(coords: &[Coord3D]) -> bool {
    let mut distinct = [(0.0, 0.0); 3];
    let mut count = 0;
    for coord in coords {
        let point = (coord.x, coord.y);
        if !distinct[..count].contains(&point) {
            distinct[count] = point;
            count += 1;
            if count == 3 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod topology_tests {
    use super::*;

    #[test]
    fn establish_topology_reports_unassigned_holes() {
        let hole = Polygon3D::new(
            vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(0.0, 10.0, 0.0),
                Coord3D::new(10.0, 10.0, 0.0),
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
            ],
            vec![],
            vec![1, 2, 3, 4],
            vec![],
        );

        let (_, _, _, unassigned_count, unassigned_area, _) =
            establish_topology(vec![], vec![(hole, None)], &PolygonizerOptions::default());

        assert_eq!(unassigned_count, 1);
        assert!((unassigned_area - 100.0).abs() < 1e-9);
    }

    #[test]
    fn reorder_polygons_applies_sorted_old_indices() {
        let polygon =
            |label| Polygon3D::new(vec![Coord3D::new(label, 0.0, 0.0)], vec![], vec![], vec![]);
        let mut polygons = vec![polygon(0.0), polygon(1.0), polygon(2.0)];

        reorder_polygons(&mut polygons, [2, 0, 1].into_iter());

        let labels: Vec<_> = polygons.iter().map(|p| p.exterior[0].x).collect();
        assert_eq!(labels, [2.0, 0.0, 1.0]);
    }
}

pub(crate) fn apply_determinism(
    mut result: Vec<Polygon3D>,
    dangles: &mut [Vec<Coord3D>],
    cut_edges: &mut [Vec<Coord3D>],
    invalid_rings: &mut Vec<Vec<Coord3D>>,
    options: &PolygonizerOptions,
) -> Vec<Polygon3D> {
    if options.determinism.canonical_sort {
        let use_stable_tie_breaks = options.determinism.stable_tie_breaks;
        if use_stable_tie_breaks {
            let mut sort_keys: Vec<_> = result
                .iter()
                .enumerate()
                .map(|(index, p)| {
                    let area = p.exterior_unsigned_area_2d();
                    let b = bounding_rect_3d(&p.exterior).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    (index, area, b, p.interiors.len())
                })
                .collect();

            sort_keys.sort_unstable_by(|(_, area1, b1, holes1), (_, area2, b2, holes2)| {
                area2.total_cmp(area1).then_with(|| {
                    b1.min()
                        .x
                        .total_cmp(&b2.min().x)
                        .then(b1.min().y.total_cmp(&b2.min().y))
                        .then(holes1.cmp(holes2))
                })
            });

            reorder_polygons(
                &mut result,
                sort_keys.into_iter().map(|(index, _, _, _)| index),
            );
        } else {
            let mut sort_keys: Vec<_> = result
                .iter()
                .enumerate()
                .map(|(index, p)| {
                    let area = p.exterior_unsigned_area_2d();
                    (index, area)
                })
                .collect();

            sort_keys.sort_unstable_by(|(_, area1), (_, area2)| area2.total_cmp(area1));

            reorder_polygons(&mut result, sort_keys.into_iter().map(|(index, _)| index));
        }

        if options.determinism.canonical_ring_rotation {
            for d in dangles.iter_mut() {
                canonicalize_open_line(d);
            }
            for edge in cut_edges.iter_mut() {
                canonicalize_open_line(edge);
            }
            for ir in invalid_rings.iter_mut() {
                canonicalize_ring(ir, None);
            }
        }

        if use_stable_tie_breaks {
            // Bolt optimization: Schwartzian Transform for dangles.
            // Caches the bounding boxes of the open lines to avoid redundant
            // O(N) evaluations of `bounding_rect_3d` during the O(K log K) sort closure.
            let mut dangles_with_cache: Vec<_> = dangles
                .iter_mut()
                .map(|l| {
                    let b = bounding_rect_3d(l).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    (std::mem::take(l), b)
                })
                .collect();

            dangles_with_cache.sort_unstable_by(|(l1, b1), (l2, b2)| {
                b1.min()
                    .x
                    .total_cmp(&b2.min().x)
                    .then(b1.min().y.total_cmp(&b2.min().y))
                    .then(l1.len().cmp(&l2.len()))
            });

            for (i, (l, _)) in dangles_with_cache.into_iter().enumerate() {
                dangles[i] = l;
            }
            sort_open_lines(cut_edges);
        }

        let mut combined_invalid: Vec<_> = invalid_rings
            .drain(..)
            .map(|r| {
                let area = Polygon3D::ring_signed_area_2d(&r).abs();
                (r, area)
            })
            .collect();

        if use_stable_tie_breaks {
            let mut invalid_with_bbox: Vec<_> = combined_invalid
                .into_iter()
                .map(|(r, area)| {
                    let b = bounding_rect_3d(&r).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    (r, area, b)
                })
                .collect();

            invalid_with_bbox.sort_unstable_by(|(r1, area1, b1), (r2, area2, b2)| {
                area2.total_cmp(area1).then_with(|| {
                    b1.min()
                        .x
                        .total_cmp(&b2.min().x)
                        .then(b1.min().y.total_cmp(&b2.min().y))
                        .then(r1.len().cmp(&r2.len()))
                })
            });
            invalid_rings.extend(invalid_with_bbox.into_iter().map(|(r, _, _)| r));
        } else {
            combined_invalid.sort_unstable_by(|(_, area1), (_, area2)| area2.total_cmp(area1));
            invalid_rings.extend(combined_invalid.into_iter().map(|(r, _)| r));
        }
    }
    result
}

fn reorder_polygons(result: &mut [Polygon3D], old_indices: impl Iterator<Item = usize>) {
    let mut destinations = vec![0; result.len()];
    for (new_index, old_index) in old_indices.enumerate() {
        destinations[old_index] = new_index;
    }
    for index in 0..destinations.len() {
        while destinations[index] != index {
            let destination = destinations[index];
            result.swap(index, destination);
            destinations.swap(index, destination);
        }
    }
}

fn sort_open_lines(lines: &mut [Vec<Coord3D>]) {
    let mut with_cache: Vec<_> = lines
        .iter_mut()
        .map(|l| {
            let b = bounding_rect_3d(l).unwrap_or(geo::Rect::new(
                geo::Coord { x: 0.0, y: 0.0 },
                geo::Coord { x: 0.0, y: 0.0 },
            ));
            (std::mem::take(l), b)
        })
        .collect();

    with_cache.sort_unstable_by(|(l1, b1), (l2, b2)| {
        b1.min()
            .x
            .total_cmp(&b2.min().x)
            .then(b1.min().y.total_cmp(&b2.min().y))
            .then(l1.len().cmp(&l2.len()))
    });

    for (i, (line, _)) in with_cache.into_iter().enumerate() {
        lines[i] = line;
    }
}

pub(crate) fn process_invalid_rings(
    rings: Vec<Polygon3D>,
    valid_shells_2d: &[Polygon<f64>],
) -> Vec<Vec<Coord3D>> {
    let mut processable = Vec::new();
    let mut others = Vec::new();

    for ring in rings {
        if ring
            .exterior
            .iter()
            .all(|c| c.x.is_finite() && c.y.is_finite())
        {
            processable.push(ring);
        } else {
            others.push(ring);
        }
    }

    // Sort by 2D bbox area in ascending order using Schwartzian Transform
    let mut processable_with_areas: Vec<_> = processable
        .into_iter()
        .map(|ring| {
            let area = if ring.exterior.is_empty() {
                0.0
            } else {
                let mut min_x = ring.exterior[0].x;
                let mut max_x = ring.exterior[0].x;
                let mut min_y = ring.exterior[0].y;
                let mut max_y = ring.exterior[0].y;
                for c in &ring.exterior[1..] {
                    if c.x < min_x {
                        min_x = c.x;
                    }
                    if c.x > max_x {
                        max_x = c.x;
                    }
                    if c.y < min_y {
                        min_y = c.y;
                    }
                    if c.y > max_y {
                        max_y = c.y;
                    }
                }
                (max_x - min_x) * (max_y - min_y)
            };
            (ring, area)
        })
        .collect();

    processable_with_areas.sort_unstable_by(|(_, area_a), (_, area_b)| area_a.total_cmp(area_b));

    let processable: Vec<_> = processable_with_areas.into_iter().map(|(r, _)| r).collect();

    struct RingPair {
        p3d: Polygon3D,
        p2d: Polygon<f64>,
    }

    let mut accepted: Vec<RingPair> = Vec::new();

    for ring in processable {
        let p2d = ring.to_polygon_2d();
        // Outer invalid rings are discarded if their linework is entirely contained
        // by an already-processed (smaller) invalid ring or a valid ring.
        let contains_invalid = accepted.iter().any(|existing| p2d.contains(&existing.p2d));
        let contains_valid = valid_shells_2d.iter().any(|valid| p2d.contains(valid));

        if !contains_invalid && !contains_valid {
            accepted.push(RingPair { p3d: ring, p2d });
        }
    }

    let mut result: Vec<Vec<Coord3D>> = accepted.into_iter().map(|rp| rp.p3d.exterior).collect();
    result.extend(others.into_iter().map(|p| p.exterior));

    result
}

pub fn bounding_rect_3d(coords: &[Coord3D]) -> Option<geo::Rect<f64>> {
    if coords.is_empty() {
        return None;
    }
    let mut min_x = coords[0].x;
    let mut max_x = coords[0].x;
    let mut min_y = coords[0].y;
    let mut max_y = coords[0].y;
    for c in &coords[1..] {
        if c.x < min_x {
            min_x = c.x;
        }
        if c.x > max_x {
            max_x = c.x;
        }
        if c.y < min_y {
            min_y = c.y;
        }
        if c.y > max_y {
            max_y = c.y;
        }
    }
    Some(geo::Rect::new(
        geo::Coord { x: min_x, y: min_y },
        geo::Coord { x: max_x, y: max_y },
    ))
}

pub fn guaranteed_interior_probe(coords: &[Coord3D]) -> Option<geo_types::Point<f64>> {
    if coords.len() < 4 {
        return None;
    }

    let unique_n = coords.len().saturating_sub(1);
    if unique_n < 3 {
        return None;
    }

    let area = Polygon3D::ring_signed_area_2d(coords);
    if !area.is_finite() || area == 0.0 {
        return None;
    }

    let locator = SimdRing::new_3d(coords);
    let bbox = bounding_rect_3d(coords);
    let diagonal = bbox
        .map(|bbox| {
            let dx = bbox.max().x - bbox.min().x;
            let dy = bbox.max().y - bbox.min().y;
            (dx * dx + dy * dy).sqrt()
        })
        .unwrap_or(1.0);
    guaranteed_interior_probe_prepared(coords, area, diagonal, &locator)
}

pub(crate) fn guaranteed_interior_probe_prepared(
    coords: &[Coord3D],
    signed_area: f64,
    diagonal: f64,
    locator: &SimdRing,
) -> Option<geo_types::Point<f64>> {
    if coords.len() < 4 || !signed_area.is_finite() || signed_area == 0.0 {
        return None;
    }

    let unique_n = coords.len().saturating_sub(1);
    if unique_n < 3 {
        return None;
    }

    let eps = diagonal * 1e-9;

    for i in 0..unique_n {
        let prev = coords[(i + unique_n - 1) % unique_n];
        let curr = coords[i];
        let next = coords[(i + 1) % unique_n];

        let in_edge = Coord {
            x: curr.x - prev.x,
            y: curr.y - prev.y,
        };
        let out_edge = Coord {
            x: next.x - curr.x,
            y: next.y - curr.y,
        };

        let in_len = (in_edge.x * in_edge.x + in_edge.y * in_edge.y).sqrt();
        let out_len = (out_edge.x * out_edge.x + out_edge.y * out_edge.y).sqrt();
        if in_len == 0.0 || out_len == 0.0 {
            continue;
        }

        let turn = in_edge.x * out_edge.y - in_edge.y * out_edge.x;
        let convex = if signed_area > 0.0 {
            turn > 0.0
        } else {
            turn < 0.0
        };
        if !convex {
            continue;
        }

        let to_prev = Coord {
            x: (prev.x - curr.x) / in_len,
            y: (prev.y - curr.y) / in_len,
        };
        let to_next = Coord {
            x: (next.x - curr.x) / out_len,
            y: (next.y - curr.y) / out_len,
        };

        let bisector = Coord {
            x: to_prev.x + to_next.x,
            y: to_prev.y + to_next.y,
        };
        let bisector_len = (bisector.x * bisector.x + bisector.y * bisector.y).sqrt();
        if bisector_len == 0.0 {
            continue;
        }

        let bisector_unit = Coord {
            x: bisector.x / bisector_len,
            y: bisector.y / bisector_len,
        };

        for sign in [1.0, -1.0] {
            let offset = |value: f64, direction: f64| {
                let candidate = value + direction * eps;
                if candidate == value && direction != 0.0 {
                    value.next_after(if direction > 0.0 {
                        f64::INFINITY
                    } else {
                        f64::NEG_INFINITY
                    })
                } else {
                    candidate
                }
            };
            let candidate = Coord {
                x: offset(curr.x, sign * bisector_unit.x),
                y: offset(curr.y, sign * bisector_unit.y),
            };
            if locator.contains(candidate) {
                return Some(geo_types::Point(candidate));
            }
        }
    }

    Some(geo_types::Point(coords[0].to_coord_2d()))
}

pub fn rings_share_edge(shell: &[Coord3D], hole: &[Coord3D], eps: f64) -> bool {
    rings_share_edge_with_count(shell, hole, eps).0
}

pub(crate) fn rings_share_edge_with_count(
    shell: &[Coord3D],
    hole: &[Coord3D],
    eps: f64,
) -> (bool, usize) {
    if shell.len() < 2 || hole.len() < 2 {
        return (false, 0);
    }

    let mut pair_checks = 0;
    // Bolt optimization: using .windows(2) is faster than loop indexing
    // because it eliminates O(N*M) array bounds checks in this hot inner loop.
    for shell_edge in shell.windows(2) {
        let a1 = shell_edge[0].to_coord_2d();
        let a2 = shell_edge[1].to_coord_2d();
        for hole_edge in hole.windows(2) {
            pair_checks += 1;
            let b1 = hole_edge[0].to_coord_2d();
            let b2 = hole_edge[1].to_coord_2d();
            if segments_overlap_with_length(a1, a2, b1, b2, eps) {
                return (true, pair_checks);
            }
        }
    }

    (false, pair_checks)
}

pub fn rings_touch_at_vertex(shell: &[Coord3D], hole: &[Coord3D], eps: f64) -> bool {
    rings_touch_at_vertex_with_count(shell, hole, eps).0
}

pub(crate) fn rings_touch_at_vertex_with_count(
    shell: &[Coord3D],
    hole: &[Coord3D],
    eps: f64,
) -> (bool, usize) {
    let eps_sq = eps * eps;
    let mut pair_checks = 0;
    for s_pt in shell {
        for h_pt in hole {
            pair_checks += 1;
            let dx = s_pt.x - h_pt.x;
            let dy = s_pt.y - h_pt.y;
            if dx * dx + dy * dy <= eps_sq {
                return (true, pair_checks);
            }
        }
    }
    (false, pair_checks)
}

fn segments_overlap_with_length(
    a1: Coord<f64>,
    a2: Coord<f64>,
    b1: Coord<f64>,
    b2: Coord<f64>,
    eps: f64,
) -> bool {
    let ax = a2.x - a1.x;
    let ay = a2.y - a1.y;
    let a_len_sq = ax * ax + ay * ay;
    if a_len_sq <= eps * eps {
        return false;
    }

    // Collinearity checks for segment B endpoints against segment A line
    // using squared comparisons to avoid costly `.sqrt()`
    let cross_b1 = ax * (b1.y - a1.y) - ay * (b1.x - a1.x);
    let cross_b2 = ax * (b2.y - a1.y) - ay * (b2.x - a1.x);
    let tol_sq = eps * eps * a_len_sq;
    if cross_b1 * cross_b1 > tol_sq || cross_b2 * cross_b2 > tol_sq {
        return false;
    }

    let t1 = ((b1.x - a1.x) * ax + (b1.y - a1.y) * ay) / a_len_sq;
    let t2 = ((b2.x - a1.x) * ax + (b2.y - a1.y) * ay) / a_len_sq;

    let min_t = t1.min(t2);
    let max_t = t1.max(t2);
    let overlap_start = 0.0_f64.max(min_t);
    let overlap_end = 1.0_f64.min(max_t);

    if overlap_end <= overlap_start {
        return false;
    }

    let overlap_len = overlap_end - overlap_start;
    overlap_len * overlap_len * a_len_sq > eps * eps
}

fn extract_segments(geom: &Geometry<f64>, out: &mut Vec<Line3D>) {
    let mut stack = smallvec::SmallVec::<[&Geometry<f64>; 16]>::new();
    stack.push(geom);
    while let Some(current) = stack.pop() {
        match current {
            Geometry::LineString(ls) => {
                let len = ls.0.len().saturating_sub(1);
                out.reserve(len);
                out.extend(ls.lines().map(Line3D::from));
            }
            Geometry::MultiLineString(mls) => {
                for ls in &mls.0 {
                    let len = ls.0.len().saturating_sub(1);
                    out.reserve(len);
                    out.extend(ls.lines().map(Line3D::from));
                }
            }
            Geometry::Polygon(poly) => {
                let ext = poly.exterior();
                let len = ext.0.len().saturating_sub(1);
                out.reserve(len);
                out.extend(ext.lines().map(Line3D::from));
                for interior in poly.interiors() {
                    let len = interior.0.len().saturating_sub(1);
                    out.reserve(len);
                    out.extend(interior.lines().map(Line3D::from));
                }
            }
            Geometry::MultiPolygon(mpoly) => {
                for poly in mpoly {
                    let ext = poly.exterior();
                    let len = ext.0.len().saturating_sub(1);
                    out.reserve(len);
                    out.extend(ext.lines().map(Line3D::from));
                    for interior in poly.interiors() {
                        let len = interior.0.len().saturating_sub(1);
                        out.reserve(len);
                        out.extend(interior.lines().map(Line3D::from));
                    }
                }
            }
            Geometry::GeometryCollection(gc) => {
                stack.extend(gc.0.iter().rev());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::LineString;

    fn square_lines(x: f64, y: f64, size: f64, first_id: u32) -> Vec<Line3D> {
        let points = [
            Coord3D::new(x, y, 0.0),
            Coord3D::new(x + size, y, 0.0),
            Coord3D::new(x + size, y + size, 0.0),
            Coord3D::new(x, y + size, 0.0),
        ];
        (0..4)
            .map(|i| Line3D::new(points[i], points[(i + 1) % 4], first_id + i as u32))
            .collect()
    }

    #[test]
    fn workspace_and_builder_rebuild_cleanly() {
        for node_input in [false, true] {
            let mut options = PolygonizerOptions {
                node_input,
                ..Default::default()
            };
            let first = square_lines(0.0, 0.0, 1.0, 1);
            let second = square_lines(2.0, 0.0, 1.0, 5);

            let mut workspace = PolygonizerWorkspace::new();
            assert_eq!(
                polygonize_with_workspace(&first, &options, &mut workspace)
                    .unwrap()
                    .polygons
                    .len(),
                1
            );
            assert!(workspace.graph.edges.is_empty());
            assert_eq!(
                polygonize_with_workspace(&second, &options, &mut workspace)
                    .unwrap()
                    .polygons
                    .len(),
                1
            );
            assert!(workspace.graph.edges.is_empty());

            let mut builder = Polygonizer::with_options(std::mem::take(&mut options));
            builder.add_lines(first);
            assert_eq!(builder.polygonize().unwrap().polygons.len(), 1);
            assert_eq!(builder.polygonize().unwrap().polygons.len(), 1);
            builder.add_lines(second);
            assert_eq!(builder.polygonize().unwrap().polygons.len(), 2);
        }
    }

    #[test]
    fn small_and_large_offset_faces_are_preserved_and_filter_is_explicit() {
        let options = PolygonizerOptions::default();
        for lines in [
            square_lines(0.0, 0.0, 1e-4, 1),
            square_lines(-73.0, 40.0, 1e-7, 1),
            square_lines(1e12, 1e12, 1e-3, 1),
        ] {
            assert_eq!(polygonize(lines, &options).unwrap().polygons.len(), 1);
        }

        for origin in [0.0, 1e12] {
            let mut nested = square_lines(origin, origin, 1.0, 1);
            nested.extend(square_lines(origin + 0.25, origin + 0.25, 0.5, 5));
            assert_eq!(polygonize(nested, &options).unwrap().polygons.len(), 2);
        }

        let lines = square_lines(0.0, 0.0, 1.0, 1);
        let mut filtered = PolygonizerOptions::default();
        filtered.output_filter.minimum_face_area = Some(1.0);
        assert_eq!(
            polygonize(lines.iter().copied(), &filtered)
                .unwrap()
                .polygons
                .len(),
            1
        );
        filtered.output_filter.minimum_face_area = Some(1.01);
        assert!(polygonize(lines, &filtered).unwrap().polygons.is_empty());

        let collinear = [
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 1),
            Line3D::new(Coord3D::new(1.0, 0.0, 0.0), Coord3D::new(2.0, 0.0, 0.0), 2),
            Line3D::new(Coord3D::new(2.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 3),
        ];
        assert!(polygonize(collinear, &options).unwrap().polygons.is_empty());
    }

    #[test]
    fn fixed_precision_applies_without_noding() {
        let lines = square_lines(0.04, 0.04, 1.0, 1);
        let options = PolygonizerOptions {
            precision_model: PrecisionModel::FixedGrid { grid_size: 0.1 },
            ..Default::default()
        };

        let result = polygonize(lines, &options).unwrap();
        assert_eq!(result.polygons.len(), 1);
        assert_eq!(result.polygons[0].unsigned_area_2d(), 1.0);
        assert!(result.polygons[0]
            .exterior
            .iter()
            .all(|coord| [0.0, 1.0].contains(&coord.x) && [0.0, 1.0].contains(&coord.y)));
    }

    #[test]
    fn validation_policy_checks_the_post_noding_segments() {
        let crossing = vec![
            Line3D::new(Coord3D::new(-1.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 1),
            Line3D::new(Coord3D::new(0.0, -1.0, 0.0), Coord3D::new(0.0, 1.0, 0.0), 2),
        ];
        let validated = PolygonizerOptions {
            node_input: true,
            noding: crate::options::NodingOptions {
                guarantee: NodingGuarantee::Validate,
                ..Default::default()
            },
            ..Default::default()
        };
        polygonize(crossing.clone(), &validated).unwrap();

        let validate_only = PolygonizerOptions {
            node_input: false,
            ..validated
        };
        assert!(matches!(
            polygonize(crossing, &validate_only),
            Err(PolygonizeError::NodingValidationFailure { .. })
        ));
    }

    #[test]
    fn non_finite_coordinates_are_rejected_centrally() {
        let line = Line3D::new(
            Coord3D::new(f64::NAN, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            1,
        );
        assert!(matches!(
            polygonize([line], &PolygonizerOptions::default()),
            Err(PolygonizeError::InvalidGeometry { .. })
        ));
    }

    #[test]
    fn test_with_precision_model() {
        let polygonizer =
            Polygonizer::new().with_precision_model(PrecisionModel::FixedGrid { grid_size: 0.123 });
        assert_eq!(
            polygonizer.options().precision_model,
            PrecisionModel::FixedGrid { grid_size: 0.123 }
        );
    }

    #[test]
    fn geos_compat_preserves_source_coordinates() {
        let points = [
            Coord3D::new(0.04, 0.04, 0.0),
            Coord3D::new(10.04, 0.04, 0.0),
            Coord3D::new(10.04, 10.04, 0.0),
            Coord3D::new(0.04, 10.04, 0.0),
        ];
        let lines = (0..4)
            .map(|i| Line3D::new(points[i], points[(i + 1) % 4], i as u32))
            .collect::<Vec<_>>();
        let options = PolygonizerOptions {
            node_input: true,
            precision_model: PrecisionModel::FixedGrid { grid_size: 0.1 },
            snap_strategy: SnapStrategy::GeosCompat,
            ..Default::default()
        };

        let result = polygonize(lines.iter().copied(), &options).unwrap();

        assert_eq!(result.polygons.len(), 1);
        assert!(points.iter().all(|point| result.polygons[0]
            .exterior
            .iter()
            .any(|actual| actual == point)));
    }

    #[test]
    fn geos_compat_drops_sub_grid_face_from_near_coincident_endpoints() {
        let lines = [
            Line3D::new(
                Coord3D::new(139131.39157939597, 332081.47584308265, 0.0),
                Coord3D::new(139138.25173987588, 332179.7525957378, 0.0),
                1,
            ),
            Line3D::new(
                Coord3D::new(139128.6709763695, 332081.6202812562, 0.0),
                Coord3D::new(139131.39157939597, 332081.4758430829, 0.0),
                2,
            ),
            Line3D::new(
                Coord3D::new(139131.39157939597, 332081.4758430829, 0.0),
                Coord3D::new(139134.1124077957, 332081.33571445255, 0.0),
                3,
            ),
        ];

        let result =
            polygonize(lines.iter().copied(), &PolygonizerOptions::cfb_robust_v1()).unwrap();

        assert!(result.polygons.is_empty());
    }

    #[test]
    fn test_add_lines() {
        let mut polygonizer = Polygonizer::new();
        assert!(polygonizer.input_lines.is_empty());

        let l1 = Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 0);
        let l2 = Line3D::new(Coord3D::new(1.0, 0.0, 0.0), Coord3D::new(1.0, 1.0, 0.0), 1);

        polygonizer.add_lines(vec![l1, l2]);

        assert_eq!(polygonizer.input_lines.len(), 2);
        assert_eq!(polygonizer.input_lines[0].start.x, 0.0);
        assert_eq!(polygonizer.input_lines[1].end.y, 1.0);
    }

    #[test]
    fn test_bounding_rect_3d() {
        // 1. Empty slice
        assert_eq!(bounding_rect_3d(&[]), None);

        // 2. Single coordinate
        let single = vec![Coord3D::new(5.0, 10.0, 15.0)];
        let rect1 = bounding_rect_3d(&single).unwrap();
        assert_eq!(rect1.min().x, 5.0);
        assert_eq!(rect1.max().x, 5.0);
        assert_eq!(rect1.min().y, 10.0);
        assert_eq!(rect1.max().y, 10.0);

        // 3. Multiple coordinates (Z should be ignored)
        let coords = vec![
            Coord3D::new(1.0, 2.0, 100.0),
            Coord3D::new(-5.0, 8.0, -50.0),
            Coord3D::new(10.0, -3.0, 0.0),
            Coord3D::new(4.0, 12.0, 20.0),
        ];
        let rect2 = bounding_rect_3d(&coords).unwrap();
        assert_eq!(rect2.min().x, -5.0);
        assert_eq!(rect2.max().x, 10.0);
        assert_eq!(rect2.min().y, -3.0);
        assert_eq!(rect2.max().y, 12.0);

        // 4. Multiple coordinates with same X/Y
        let coords2 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 10.0),
            Coord3D::new(0.0, 0.0, -10.0),
        ];
        let rect3 = bounding_rect_3d(&coords2).unwrap();
        assert_eq!(rect3.min().x, 0.0);
        assert_eq!(rect3.max().x, 0.0);
        assert_eq!(rect3.min().y, 0.0);
        assert_eq!(rect3.max().y, 0.0);
    }

    #[test]
    fn test_rings_share_edge() {
        let shell = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];

        // 1. Identical edge (opposite direction)
        let hole1 = vec![
            Coord3D::new(2.0, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(1.0, 1.0, 0.0),
            Coord3D::new(2.0, 1.0, 0.0),
            Coord3D::new(2.0, 0.0, 0.0),
        ];
        // shell has (0,0)->(10,0). hole1 has (2,0)->(1,0) which is on the same line.
        assert!(rings_share_edge(&shell, &hole1, 1e-10));

        // 2. Partial overlap
        let hole2 = vec![
            Coord3D::new(-1.0, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(1.0, -1.0, 0.0),
            Coord3D::new(-1.0, -1.0, 0.0),
            Coord3D::new(-1.0, 0.0, 0.0),
        ];
        // shell (0,0)->(10,0). hole2 (-1,0)->(1,0). Overlap is (0,0)->(1,0).
        assert!(rings_share_edge(&shell, &hole2, 1e-10));

        // 3. Touching at vertex only
        let hole3 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(-1.0, -1.0, 0.0),
            Coord3D::new(0.0, -1.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        assert!(!rings_share_edge(&shell, &hole3, 1e-10));

        // 4. Disjoint but collinear
        let hole4 = vec![
            Coord3D::new(11.0, 0.0, 0.0),
            Coord3D::new(12.0, 0.0, 0.0),
            Coord3D::new(12.0, 1.0, 0.0),
            Coord3D::new(11.0, 1.0, 0.0),
            Coord3D::new(11.0, 0.0, 0.0),
        ];
        assert!(!rings_share_edge(&shell, &hole4, 1e-10));

        // 5. Parallel but not collinear
        let hole5 = vec![
            Coord3D::new(0.0, -1.0, 0.0),
            Coord3D::new(10.0, -1.0, 0.0),
            Coord3D::new(10.0, -2.0, 0.0),
            Coord3D::new(0.0, -2.0, 0.0),
            Coord3D::new(0.0, -1.0, 0.0),
        ];
        assert!(!rings_share_edge(&shell, &hole5, 1e-10));

        // 6. Shared edge with epsilon (within distance)
        let hole6 = vec![
            Coord3D::new(0.0, 1e-11, 0.0),
            Coord3D::new(10.0, 1e-11, 0.0),
            Coord3D::new(10.0, 1.0, 0.0),
            Coord3D::new(0.0, 1.0, 0.0),
            Coord3D::new(0.0, 1e-11, 0.0),
        ];
        assert!(rings_share_edge(&shell, &hole6, 1e-10));

        // 7. Shared edge with epsilon (just outside distance)
        let hole7 = vec![
            Coord3D::new(1.0, 1e-9, 0.0),
            Coord3D::new(9.0, 1e-9, 0.0),
            Coord3D::new(9.0, 1.0, 0.0),
            Coord3D::new(1.0, 1.0, 0.0),
            Coord3D::new(1.0, 1e-9, 0.0),
        ];
        assert!(!rings_share_edge(&shell, &hole7, 1e-10));

        // 8. Short segment bug case
        let shell_short = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.01, 0.0, 0.0),
            Coord3D::new(0.01, 0.01, 0.0),
            Coord3D::new(0.0, 0.01, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        let hole_short = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.01, 0.0, 0.0),
            Coord3D::new(0.01, -0.01, 0.0),
            Coord3D::new(0.0, -0.01, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        // Overlap is (0,0) -> (0.01, 0), length 0.01.
        // If eps is 0.1, it should NOT share edge.
        assert!(!rings_share_edge(&shell_short, &hole_short, 0.1));
        // If eps is 0.001, it SHOULD share edge.
        assert!(rings_share_edge(&shell_short, &hole_short, 0.001));
    }

    #[test]
    fn test_guaranteed_interior_probe() {
        // 1. Less than 4 points
        let coords1 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(0.0, 1.0, 0.0),
        ];
        assert_eq!(guaranteed_interior_probe(&coords1), None);

        // 2. Less than 3 unique points
        let coords2 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        assert_eq!(guaranteed_interior_probe(&coords2), None);

        // 3. Zero area / collinear
        let coords3 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(2.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        assert_eq!(guaranteed_interior_probe(&coords3), None);

        // 4. Simple convex polygon
        let coords4 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        let probe4 = guaranteed_interior_probe(&coords4).expect("Should find a point");
        let coords4_2d: Vec<_> = coords4.iter().map(|c| c.to_coord_2d()).collect();
        let p4 = Polygon::new(LineString::from(coords4_2d), vec![]);
        assert!(p4.contains(&probe4));

        // 5. Highly concave polygon where centroid is outside
        // A "U" shape:
        // (0, 0) to (10, 0) to (10, 10) to (8, 10) to (8, 2) to (2, 2) to (2, 10) to (0, 10) to (0, 0)
        let coords5 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(8.0, 10.0, 0.0),
            Coord3D::new(8.0, 2.0, 0.0),
            Coord3D::new(2.0, 2.0, 0.0),
            Coord3D::new(2.0, 10.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        let probe5 =
            guaranteed_interior_probe(&coords5).expect("Should find a point via bisector fallback");
        let coords5_2d: Vec<_> = coords5.iter().map(|c| c.to_coord_2d()).collect();
        let p5 = Polygon::new(LineString::from(coords5_2d), vec![]);
        assert!(p5.contains(&probe5));
    }

    #[test]
    fn test_rings_touch_at_vertex() {
        let shell = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];

        // 1. Exact touch at one vertex (0,0)
        let hole1 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(-1.0, -1.0, 0.0),
            Coord3D::new(1.0, -1.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        assert!(rings_touch_at_vertex(&shell, &hole1, 1e-10));

        // 2. Touch within epsilon of vertex (10,10)
        let hole2 = vec![
            Coord3D::new(10.0 + 1e-11, 10.0, 0.0),
            Coord3D::new(11.0, 11.0, 0.0),
            Coord3D::new(11.0, 10.0, 0.0),
            Coord3D::new(10.0 + 1e-11, 10.0, 0.0),
        ];
        assert!(rings_touch_at_vertex(&shell, &hole2, 1e-10));

        // 3. No touch (just outside epsilon of vertex 10,10)
        let hole3 = vec![
            Coord3D::new(10.0 + 1e-9, 10.0, 0.0),
            Coord3D::new(11.0, 11.0, 0.0),
            Coord3D::new(11.0, 10.0, 0.0),
            Coord3D::new(10.0 + 1e-9, 10.0, 0.0),
        ];
        assert!(!rings_touch_at_vertex(&shell, &hole3, 1e-10));

        // 4. Touch at multiple vertices
        let hole4 = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(5.0, -5.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
        ];
        assert!(rings_touch_at_vertex(&shell, &hole4, 1e-10));

        // 5. Disjoint
        let hole5 = vec![
            Coord3D::new(20.0, 20.0, 0.0),
            Coord3D::new(21.0, 20.0, 0.0),
            Coord3D::new(21.0, 21.0, 0.0),
            Coord3D::new(20.0, 20.0, 0.0),
        ];
        assert!(!rings_touch_at_vertex(&shell, &hole5, 1e-10));

        // 6. Empty rings
        assert!(!rings_touch_at_vertex(&[], &hole1, 1e-10));
        assert!(!rings_touch_at_vertex(&shell, &[], 1e-10));
    }
}
