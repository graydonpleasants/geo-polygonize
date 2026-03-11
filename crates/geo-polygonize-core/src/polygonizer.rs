use crate::diagnostics::PolygonizerDiagnostics;
use crate::options::DiagnosticsOptions;
use crate::error::Result;
use crate::graph::PlanarGraph;
use crate::noding::snap::SnapNoder;
use crate::options::{DeterminismOptions, PolygonizerOptions};
use crate::types::{Coord3D, Line3D, Polygon3D};
use crate::utils::simd::SimdRing;
use crate::utils::z_order_index;
use geo::Contains;
use geo_types::{Coord, Geometry, Polygon};
use rstar::{RTree, RTreeObject, AABB};

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(not(target_arch = "wasm32"))]
fn get_time() -> Option<Instant> {
    Some(Instant::now())
}

#[cfg(target_arch = "wasm32")]
fn get_time() -> Option<()> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn get_elapsed(start: Option<Instant>) -> std::time::Duration {
    start.map(|s| s.elapsed()).unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn get_elapsed(_start: Option<()>) -> std::time::Duration {
    std::time::Duration::default()
}

#[cfg(feature = "parallel")]
use rayon::prelude::*;

// Wrapper for Polygon indexable by rstar (2D)
struct IndexedEnvelope {
    aabb: AABB<[f64; 2]>,
    index: usize,
}

impl RTreeObject for IndexedEnvelope {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb
    }
}

/// A robust polygonizer that reconstructs polygons from a set of lines (3D supported).
pub struct Polygonizer {
    graph: PlanarGraph,
    // Configuration
    pub check_valid_rings: bool,
    pub options: PolygonizerOptions,

    // Legacy fields maintained for backward compatibility wrappers during transition
    pub node_input: bool,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: bool,
    pub determinism: DeterminismOptions,
    pub diagnostics_options: DiagnosticsOptions,

    // Buffer for explicit line segments (3D)
    input_lines: Vec<Line3D>,
    dirty: bool,
}

pub struct PolygonizerResult {
    pub polygons: Vec<Polygon3D>,
    pub dangles: Vec<Vec<Coord3D>>,
    pub invalid_rings: Vec<Vec<Coord3D>>,
    pub diagnostics: Option<PolygonizerDiagnostics>,
}

impl Default for Polygonizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Polygonizer {
    /// Creates a new `Polygonizer` with default configuration.
    pub fn new() -> Self {
        Self {
            graph: PlanarGraph::new(),
            check_valid_rings: true,
            options: PolygonizerOptions::default(),
            node_input: false,
            snap_grid_size: 1e-10, // Default tolerance
            extract_only_polygonal: false,
            determinism: DeterminismOptions::default(),
            diagnostics_options: DiagnosticsOptions::default(),
            input_lines: Vec::new(),
            dirty: false,
        }
    }

    /// Creates a new `Polygonizer` with specific options.
    pub fn with_options(options: PolygonizerOptions) -> Self {
        Self {
            graph: PlanarGraph::new(),
            check_valid_rings: true,
            node_input: options.node_input,
            snap_grid_size: options.snap_grid_size,
            extract_only_polygonal: options.extract_only_polygonal,
            determinism: options.determinism.clone(),
            diagnostics_options: options.diagnostics.clone(),
            options,
            input_lines: Vec::new(),
            dirty: false,
        }
    }

    /// Sets the snap grid size for noding.
    ///
    /// # Arguments
    ///
    /// * `grid_size` - The size of the grid cells. Smaller values mean higher precision but potential for robustness issues if too small.
    pub fn with_snap_grid(mut self, grid_size: f64) -> Self {
        self.snap_grid_size = grid_size;
        self
    }

    /// Adds a 2D geometry to the graph (Z=0).
    pub fn add_geometry(&mut self, geom: Geometry<f64>) {
        extract_segments(&geom, &mut self.input_lines);
        self.dirty = true;
    }

    /// Adds a 2D geometry to the graph (Z=0) from a reference.
    pub fn add_borrowed_geometry(&mut self, geom: &Geometry<f64>) {
        extract_segments(geom, &mut self.input_lines);
        self.dirty = true;
    }

    /// Adds explicit 3D lines.
    pub fn add_lines(&mut self, lines: Vec<Line3D>) {
        self.input_lines.extend(lines);
        self.dirty = true;
    }

    fn build_graph(&mut self) -> Result<()> {
        if !self.dirty {
            return Ok(());
        }

        // Sync legacy wrapper fields back into options before executing
        self.options.node_input = self.node_input;
        self.options.snap_grid_size = self.snap_grid_size;
        self.options.extract_only_polygonal = self.extract_only_polygonal;
        self.options.determinism = self.determinism.clone();
        self.options.diagnostics = self.diagnostics_options.clone();

        let mut all_segments: Vec<Line3D> = self.input_lines.clone();

        let segments;

        if self.options.node_input {
            // Sort by 2D coordinates
            all_segments.sort_by(|a, b| {
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

            let noder = SnapNoder::new(self.options.snap_grid_size);
            segments = noder.node(all_segments);
        } else {
            segments = all_segments;
        }

        // Use bulk load
        self.graph.bulk_load(segments);

        self.dirty = false;
        Ok(())
    }

    /// Computes the polygons.
    /// This is the main entry point.
    ///
    /// Returns a `PolygonizerResult` containing polygons and dangles.
    pub fn polygonize(&mut self) -> Result<PolygonizerResult> {
        // Sync legacy wrapper fields back into options before executing (if not called by build_graph)
        self.options.node_input = self.node_input;
        self.options.snap_grid_size = self.snap_grid_size;
        self.options.extract_only_polygonal = self.extract_only_polygonal;
        self.options.determinism = self.determinism.clone();
        self.options.diagnostics = self.diagnostics_options.clone();

        let mut diag = if self.options.diagnostics.enabled {
            let d = PolygonizerDiagnostics {
                input_segment_count: self.input_lines.len(),
                ..Default::default()
            };
            Some(d)
        } else {
            None
        };

        let t_ingest_start = get_time();
        self.build_graph()?;
        if let Some(ref mut d) = diag {
            d.phase_times.ingest_and_node = get_elapsed(t_ingest_start);
        }

        let t_graph_build_start = get_time();
        // 1. Sort edges (Geometry Graph operation)
        self.graph.sort_edges();

        // 2. Prune dangles
        let mut dangles = self.graph.prune_dangles();

        // 3. Find rings (3D)
        let rings_with_ids = self.graph.get_edge_rings();

        // 3b. Find cut edges
        let mut cut_edges = self.graph.get_cut_edges();

        if let Some(ref mut d) = diag {
            d.phase_times.graph_build = get_elapsed(t_graph_build_start);
            d.ring_count = rings_with_ids.len();
            d.cut_edge_count = cut_edges.len();
            // Note: dangles length here does not include cut_edges yet
            d.dangle_count = dangles.len() + cut_edges.len();
        }

        dangles.append(&mut cut_edges);

        // 4. Classify Rings (Shell vs Hole)
        let mut shells = Vec::new();
        let mut holes = Vec::new();
        let mut invalid_rings_candidates = Vec::new();

        let t_ring_extraction_start = get_time();
        shells.reserve(rings_with_ids.len() / 2);
        holes.reserve(rings_with_ids.len() / 2);

        for (ring_coords, ring_ids) in rings_with_ids {
            // Create Polygon3D
            let poly3d = Polygon3D::new(ring_coords, vec![], ring_ids, vec![]);
            let area = poly3d.signed_area_2d();

            if !area.is_finite() || area.abs() < 1e-9 {
                invalid_rings_candidates.push(poly3d);
                continue;
            }

            if area > 0.0 {
                // CCW -> Shell
                shells.push(poly3d);
            } else {
                // CW -> Hole
                holes.push(poly3d);
            }
        }

        if let Some(ref mut d) = diag {
            d.phase_times.ring_extraction = get_elapsed(t_ring_extraction_start);
            d.shell_count = shells.len();
            d.hole_count = holes.len();
            d.invalid_ring_count = invalid_rings_candidates.len();
        }

        let t_containment_start = get_time();
        // 5. Establish Topology

        // Precompute 2D shells for spatial index and SIMD

        let mut simd_shells: Vec<SimdRing>;
        #[cfg(feature = "parallel")]
        {
            simd_shells = shells
                .par_iter()
                .map(|s| SimdRing::new_3d(&s.exterior))
                .collect();
        }
        #[cfg(not(feature = "parallel"))]
        {
            simd_shells = shells
                .iter()
                .map(|s| SimdRing::new_3d(&s.exterior))
                .collect();
        }

        // Build RTree for shells
        let mut indexed_shells = Vec::with_capacity(shells.len());
        for (i, shell) in shells.iter().enumerate() {
            if let Some(bbox) = bounding_rect_3d(&shell.exterior) {
                let aabb =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
                indexed_shells.push(IndexedEnvelope { aabb, index: i });
            }
        }
        let mut tree = RTree::bulk_load(indexed_shells);

        // Filter shells
        if self.options.extract_only_polygonal {
            let mut keep_mask = vec![true; shells.len()];
            let mut removed_count = 0;

            // Precompute probe points
            let probe_points: Vec<Option<geo_types::Point<f64>>> = shells
                .iter()
                .map(|s| guaranteed_interior_probe(&s.exterior))
                .collect();

            let mut container_counts = vec![0; shells.len()];

            for (i, shell) in shells.iter().enumerate() {
                let bbox = match bounding_rect_3d(&shell.exterior) {
                    Some(b) => b,
                    None => {
                        keep_mask[i] = false;
                        removed_count += 1;
                        continue;
                    }
                };
                let aabb =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

                let candidates = tree.locate_in_envelope_intersecting(&aabb);
                let probe = probe_points[i];

                if let Some(probe_pt) = probe {
                    for cand in candidates {
                        let j = cand.index;
                        if i == j {
                            continue;
                        }

                        // Check if shell[i] is inside shell[j]
                        let simd_shell = &simd_shells[j];

                        if simd_shell.contains(probe_pt.0) {
                            let area_i = shell.unsigned_area_2d();
                            let area_j = shells[j].unsigned_area_2d();

                            // If i is strictly contained inside j, increment container count
                            if (area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i))
                                && !rings_share_edge(&shells[j].exterior, &shell.exterior, 1e-10)
                            {
                                container_counts[i] += 1;
                            }
                        }
                    }
                } else {
                    keep_mask[i] = false;
                    removed_count += 1;
                }
            }

            for i in 0..shells.len() {
                if keep_mask[i] && container_counts[i] % 2 != 0 {
                    keep_mask[i] = false;
                    removed_count += 1;
                }
            }

            if removed_count > 0 {
                let mut new_shells = Vec::new();

                for (keep, s) in keep_mask.into_iter().zip(shells) {
                    if keep {
                        new_shells.push(s);
                    } else {
                        // We do not need to track discarded edges from 2D shells for topological hole assignment
                        // because we already established nesting correctly using the container counts.
                    }
                }
                shells = new_shells;

                // Rebuild helper structures
                #[cfg(feature = "parallel")]
                {
                    simd_shells = shells
                        .par_iter()
                        .map(|s| SimdRing::new_3d(&s.exterior))
                        .collect();
                }
                #[cfg(not(feature = "parallel"))]
                {
                    simd_shells = shells
                        .iter()
                        .map(|s| SimdRing::new_3d(&s.exterior))
                        .collect();
                }

                let mut indexed_shells = Vec::with_capacity(shells.len());
                for (i, shell) in shells.iter().enumerate() {
                    if let Some(bbox) = bounding_rect_3d(&shell.exterior) {
                        let aabb = AABB::from_corners(
                            [bbox.min().x, bbox.min().y],
                            [bbox.max().x, bbox.max().y],
                        );
                        indexed_shells.push(IndexedEnvelope { aabb, index: i });
                    }
                }
                tree = RTree::bulk_load(indexed_shells);
            }
        }

        // Process hole assignment
        let process_hole_assignment =
            |hole_3d: Polygon3D| -> Option<(usize, Vec<Coord3D>, Vec<u32>)> {
                let bbox = bounding_rect_3d(&hole_3d.exterior)?;
                let hole_aabb =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

                let candidates = tree.locate_in_envelope_intersecting(&hole_aabb);

                let mut best_shell_idx = None;
                let mut min_area = f64::MAX;

                let probe_point = guaranteed_interior_probe(&hole_3d.exterior)?;

                for cand in candidates {
                    let idx = cand.index;
                    let simd_shell = &simd_shells[idx];

                    if simd_shell.contains(probe_point.0) {
                        let area = shells[idx].unsigned_area_2d();
                        let hole_area = hole_3d.unsigned_area_2d();

                        if area > hole_area + 1e-6
                            && area < min_area
                            && !rings_share_edge(&shells[idx].exterior, &hole_3d.exterior, 1e-10)
                        {
                            min_area = area;
                            best_shell_idx = Some(idx);
                        }
                    }
                }

                best_shell_idx.map(|idx| (idx, hole_3d.exterior, hole_3d.exterior_ids))
            };

        let assignments: Vec<_>;
        #[cfg(feature = "parallel")]
        {
            assignments = holes
                .into_par_iter()
                .filter_map(process_hole_assignment)
                .collect();
        }
        #[cfg(not(feature = "parallel"))]
        {
            assignments = holes
                .into_iter()
                .filter_map(process_hole_assignment)
                .collect();
        }

        // Group holes by shell
        let mut shell_holes: Vec<Vec<Vec<Coord3D>>> = vec![vec![]; shells.len()];
        let mut shell_holes_ids: Vec<Vec<Vec<u32>>> = vec![vec![]; shells.len()];

        for (idx, hole_coords, hole_ids) in assignments {
            shell_holes[idx].push(hole_coords);
            shell_holes_ids[idx].push(hole_ids);
        }

        // Helper to rotate a closed ring to its canonical start coordinate (lexicographically smallest)
        // while preserving the association with edge IDs.
        let canonicalize_ring = |ring: &mut Vec<Coord3D>, ids: Option<&mut Vec<u32>>| {
            if ring.is_empty() {
                return;
            }
            // A closed ring has first == last point. Ignore the duplicate last point for finding min.
            let n = if ring.len() > 1 && ring.first() == ring.last() {
                ring.len() - 1
            } else {
                ring.len()
            };

            let min_idx = (0..n)
                .min_by(|&i, &j| {
                    ring[i]
                        .x
                        .total_cmp(&ring[j].x)
                        .then(ring[i].y.total_cmp(&ring[j].y))
                        .then(ring[i].z.total_cmp(&ring[j].z))
                })
                .unwrap_or(0);

            if min_idx > 0 {
                let mut new_ring = Vec::with_capacity(ring.len());
                new_ring.extend_from_slice(&ring[min_idx..n]);
                new_ring.extend_from_slice(&ring[0..min_idx]);
                // Re-close the ring
                new_ring.push(new_ring[0]);
                *ring = new_ring;

                // Rotate the IDs.
                // A closed ring of N vertices (with first==last) corresponds to N-1 edges (and N-1 IDs).
                if let Some(ids_vec) = ids {
                    if !ids_vec.is_empty() {
                        let mut new_ids = Vec::with_capacity(ids_vec.len());
                        new_ids.extend_from_slice(&ids_vec[min_idx..]);
                        new_ids.extend_from_slice(&ids_vec[0..min_idx]);
                        *ids_vec = new_ids;
                    }
                }
            }
        };

        // Helper to canonicalize open linestrings (like dangles) by reversing them if
        // their last coordinate is lexicographically smaller than their first coordinate.
        let canonicalize_open_line = |line: &mut Vec<Coord3D>| {
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
        };

        // 6. Construct Final Polygons
        // Ensure we don't crash on NaNs during processing
        let mut invalid_rings = if invalid_rings_candidates.is_empty() {
            Vec::new()
        } else {
            let shells_2d: Vec<Polygon<f64>> = shells.iter().map(|s| s.to_polygon_2d()).collect();
            process_invalid_rings(invalid_rings_candidates, &shells_2d)
        };

        let mut result = Vec::with_capacity(shells.len());
        for ((shell, mut holes), mut holes_ids) in shells
            .into_iter()
            .zip(shell_holes.into_iter())
            .zip(shell_holes_ids.into_iter())
        {
            let mut exterior = shell.exterior;
            let mut exterior_ids = shell.exterior_ids;

            if self.options.determinism.canonical_ring_rotation {
                canonicalize_ring(&mut exterior, Some(&mut exterior_ids));
                for (h, h_ids) in holes.iter_mut().zip(holes_ids.iter_mut()) {
                    canonicalize_ring(h, Some(h_ids));
                }
            }

            if self.options.determinism.canonical_sort {
                let mut combined_holes: Vec<_> =
                    holes.into_iter().zip(holes_ids.into_iter()).collect();
                let use_stable_tie_breaks = self.options.determinism.stable_tie_breaks;
                combined_holes.sort_by(|(h1, _), (h2, _)| {
                    let area1 = Polygon3D::ring_signed_area_2d(h1).abs();
                    let area2 = Polygon3D::ring_signed_area_2d(h2).abs();
                    // Sort holes by area (descending)
                    area2.total_cmp(&area1).then_with(|| {
                        if !use_stable_tie_breaks {
                            return std::cmp::Ordering::Equal;
                        }
                        // For stable tie breaking, sort by bounding box bottom-left, then number of points
                        let b1 = bounding_rect_3d(h1).unwrap_or(geo::Rect::new(
                            geo::Coord { x: 0.0, y: 0.0 },
                            geo::Coord { x: 0.0, y: 0.0 },
                        ));
                        let b2 = bounding_rect_3d(h2).unwrap_or(geo::Rect::new(
                            geo::Coord { x: 0.0, y: 0.0 },
                            geo::Coord { x: 0.0, y: 0.0 },
                        ));
                        b1.min()
                            .x
                            .total_cmp(&b2.min().x)
                            .then(b1.min().y.total_cmp(&b2.min().y))
                            .then(h1.len().cmp(&h2.len()))
                    })
                });
                let (h, hi): (Vec<_>, Vec<_>) = combined_holes.into_iter().unzip();
                holes = h;
                holes_ids = hi;
            }

            let p = Polygon3D::new(exterior, holes, exterior_ids, holes_ids);

            // Check area of 2D projection
            if p.unsigned_area_2d() > 1e-6 {
                result.push(p);
            }
        }

        if self.options.determinism.canonical_sort {
            let use_stable_tie_breaks = self.options.determinism.stable_tie_breaks;
            result.sort_by(|p1, p2| {
                let area1 = p1.unsigned_area_2d();
                let area2 = p2.unsigned_area_2d();
                // Sort polygons by area (descending)
                area2.total_cmp(&area1).then_with(|| {
                    if !use_stable_tie_breaks {
                        return std::cmp::Ordering::Equal;
                    }
                    let b1 = bounding_rect_3d(&p1.exterior).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    let b2 = bounding_rect_3d(&p2.exterior).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    b1.min()
                        .x
                        .total_cmp(&b2.min().x)
                        .then(b1.min().y.total_cmp(&b2.min().y))
                        .then(p1.interiors.len().cmp(&p2.interiors.len()))
                })
            });

            if self.options.determinism.canonical_ring_rotation {
                for d in dangles.iter_mut() {
                    canonicalize_open_line(d);
                }
                for ir in invalid_rings.iter_mut() {
                    canonicalize_ring(ir, None);
                }
            }

            if use_stable_tie_breaks {
                dangles.sort_by(|l1, l2| {
                    let b1 = bounding_rect_3d(l1).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    let b2 = bounding_rect_3d(l2).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    b1.min()
                        .x
                        .total_cmp(&b2.min().x)
                        .then(b1.min().y.total_cmp(&b2.min().y))
                        .then(l1.len().cmp(&l2.len()))
                });
            }

            invalid_rings.sort_by(|r1, r2| {
                let area1 = Polygon3D::ring_signed_area_2d(r1).abs();
                let area2 = Polygon3D::ring_signed_area_2d(r2).abs();
                area2.total_cmp(&area1).then_with(|| {
                    if !use_stable_tie_breaks {
                        return std::cmp::Ordering::Equal;
                    }
                    let b1 = bounding_rect_3d(r1).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    let b2 = bounding_rect_3d(r2).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    b1.min()
                        .x
                        .total_cmp(&b2.min().x)
                        .then(b1.min().y.total_cmp(&b2.min().y))
                        .then(r1.len().cmp(&r2.len()))
                })
            });
        }

        if let Some(ref mut d) = diag {
            d.phase_times.containment = get_elapsed(t_containment_start);
            // output_flatten time could be measured here if we had a separate pass, but we'll leave it 0 or record what we have
        }
        Ok(PolygonizerResult {
            polygons: result,
            dangles,
            invalid_rings,
            diagnostics: diag,
        })
    }
}

fn process_invalid_rings(
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

    // Sort by 2D bbox area in ascending order
    processable.sort_by(|a, b| {
        let get_bbox_area = |ring: &Polygon3D| {
            if ring.exterior.is_empty() {
                return 0.0;
            }
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
        let area_a = get_bbox_area(a);
        let area_b = get_bbox_area(b);
        area_a
            .partial_cmp(&area_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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

fn bounding_rect_3d(coords: &[Coord3D]) -> Option<geo::Rect<f64>> {
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

fn guaranteed_interior_probe(coords: &[Coord3D]) -> Option<geo_types::Point<f64>> {
    if coords.len() < 4 {
        return None;
    }

    let unique_n = coords.len().saturating_sub(1);
    if unique_n < 3 {
        return None;
    }

    let area = Polygon3D::ring_signed_area_2d(coords);
    if !area.is_finite() || area.abs() < 1e-12 {
        return None;
    }

    let hole_simd = SimdRing::new_3d(coords);
    let diag = bounding_rect_3d(coords)
        .map(|b| {
            let dx = b.max().x - b.min().x;
            let dy = b.max().y - b.min().y;
            (dx * dx + dy * dy).sqrt()
        })
        .unwrap_or(1.0);
    let eps = (diag * 1e-9).max(1e-10);

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
        if in_len < 1e-12 || out_len < 1e-12 {
            continue;
        }

        let turn = in_edge.x * out_edge.y - in_edge.y * out_edge.x;
        let convex = if area > 0.0 {
            turn > 1e-12
        } else {
            turn < -1e-12
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
        if bisector_len < 1e-12 {
            continue;
        }

        let bisector_unit = Coord {
            x: bisector.x / bisector_len,
            y: bisector.y / bisector_len,
        };

        for sign in [1.0, -1.0] {
            let candidate = Coord {
                x: curr.x + sign * bisector_unit.x * eps,
                y: curr.y + sign * bisector_unit.y * eps,
            };
            if hole_simd.contains(candidate) {
                return Some(geo_types::Point(candidate));
            }
        }
    }

    Some(geo_types::Point(coords[0].to_coord_2d()))
}

fn rings_share_edge(shell: &[Coord3D], hole: &[Coord3D], eps: f64) -> bool {
    if shell.len() < 2 || hole.len() < 2 {
        return false;
    }

    let shell_n = shell.len() - 1;
    let hole_n = hole.len() - 1;

    for i in 0..shell_n {
        let a1 = shell[i].to_coord_2d();
        let a2 = shell[i + 1].to_coord_2d();
        for j in 0..hole_n {
            let b1 = hole[j].to_coord_2d();
            let b2 = hole[j + 1].to_coord_2d();
            if segments_overlap_with_length(a1, a2, b1, b2, eps) {
                return true;
            }
        }
    }

    false
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
    let cross_b1 = ax * (b1.y - a1.y) - ay * (b1.x - a1.x);
    let cross_b2 = ax * (b2.y - a1.y) - ay * (b2.x - a1.x);
    let tol = eps * a_len_sq.sqrt();
    if cross_b1.abs() > tol || cross_b2.abs() > tol {
        return false;
    }

    let t1 = ((b1.x - a1.x) * ax + (b1.y - a1.y) * ay) / a_len_sq;
    let t2 = ((b2.x - a1.x) * ax + (b2.y - a1.y) * ay) / a_len_sq;

    let min_t = t1.min(t2);
    let max_t = t1.max(t2);
    let overlap_start = 0.0_f64.max(min_t);
    let overlap_end = 1.0_f64.min(max_t);

    overlap_end - overlap_start > eps
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

    #[test]
    fn test_with_snap_grid() {
        let polygonizer = Polygonizer::new().with_snap_grid(0.123);
        assert_eq!(polygonizer.snap_grid_size, 0.123);
    }

    #[test]
    fn test_add_lines() {
        let mut polygonizer = Polygonizer::new();
        assert!(!polygonizer.dirty);
        assert!(polygonizer.input_lines.is_empty());

        let l1 = Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 0);
        let l2 = Line3D::new(Coord3D::new(1.0, 0.0, 0.0), Coord3D::new(1.0, 1.0, 0.0), 1);

        polygonizer.add_lines(vec![l1, l2]);

        assert!(polygonizer.dirty);
        assert_eq!(polygonizer.input_lines.len(), 2);
        assert_eq!(polygonizer.input_lines[0].start.x, 0.0);
        assert_eq!(polygonizer.input_lines[1].end.y, 1.0);
    }
}
