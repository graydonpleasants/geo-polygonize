use crate::containment::ContainmentForest;
use crate::diagnostics::PolygonizerDiagnostics;
use crate::error::Result;
use crate::graph::PlanarGraph;
use crate::noding::advanced::AdvancedNoder;
use crate::noding::snap::SnapNoder;
use crate::options::DiagnosticsOptions;
use crate::options::{DeterminismOptions, PolygonizerOptions};
use crate::types::{Coord3D, Line3D, Polygon3D};
use crate::utils::simd::SimdRing;
use crate::utils::z_order_index;
use geo::Contains;
use geo_types::{Coord, Geometry, Polygon};

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

#[derive(Clone, Debug, serde::Serialize)]
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

/// A stable, explicit entrypoint across all bindings to polygonize via `PolygonizerOptions`.
pub fn polygonize_with_options(
    lines: &[Line3D],
    options: &PolygonizerOptions,
) -> Result<PolygonizerResult> {
    let mut polygonizer = Polygonizer::with_options(options.clone());
    polygonizer.add_lines(lines.to_vec());
    polygonizer.polygonize()
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

            match self.options.noding.backend {
                crate::options::NodingBackend::Snap => {
                    let noder = SnapNoder::new(self.options.snap_grid_size)
                        .with_snap_strategy(self.options.snap_strategy.clone());
                    segments = noder.node(all_segments);
                }
                crate::options::NodingBackend::Advanced => {
                    let noder = AdvancedNoder::new();
                    segments = noder.node(all_segments);
                }
            }
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
        let t_ring_extraction_start = get_time();
        let (shells, holes, invalid_rings_candidates) = extract_and_classify_rings(rings_with_ids);

        if let Some(ref mut d) = diag {
            d.phase_times.ring_extraction = get_elapsed(t_ring_extraction_start);
            d.shell_count = shells.len();
            d.hole_count = holes.len();
            d.invalid_ring_count = invalid_rings_candidates.len();
        }

        let t_containment_start = get_time();
        // 5. Establish Topology
        let (shells, shell_holes, shell_holes_ids) =
            establish_topology(shells, holes, &self.options);

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

        result = apply_determinism(result, &mut dangles, &mut invalid_rings, &self.options);

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

pub(crate) fn canonicalize_ring(ring: &mut Vec<Coord3D>, mut ids: Option<&mut Vec<u32>>) {
    if ring.is_empty() {
        return;
    }
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
        new_ring.push(new_ring[0]);
        *ring = new_ring;

        if let Some(ref mut ids_vec) = ids {
            if !ids_vec.is_empty() {
                let mut new_ids = Vec::with_capacity(ids_vec.len());
                new_ids.extend_from_slice(&ids_vec[min_idx..]);
                new_ids.extend_from_slice(&ids_vec[0..min_idx]);
                **ids_vec = new_ids;
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

fn extract_and_classify_rings(
    rings_with_ids: Vec<(Vec<Coord3D>, Vec<u32>)>,
) -> (Vec<Polygon3D>, Vec<Polygon3D>, Vec<Polygon3D>) {
    let mut shells = Vec::with_capacity(rings_with_ids.len() / 2);
    let mut holes = Vec::with_capacity(rings_with_ids.len() / 2);
    let mut invalid_rings_candidates = Vec::new();

    for (ring_coords, ring_ids) in rings_with_ids {
        let poly3d = Polygon3D::new(ring_coords, vec![], ring_ids, vec![]);
        let area = poly3d.signed_area_2d();

        if !area.is_finite() || area.abs() < 1e-9 {
            invalid_rings_candidates.push(poly3d);
            continue;
        }

        if area > 0.0 {
            shells.push(poly3d);
        } else {
            holes.push(poly3d);
        }
    }

    (shells, holes, invalid_rings_candidates)
}

#[allow(clippy::type_complexity)]
fn establish_topology(
    mut shells: Vec<Polygon3D>,
    holes: Vec<Polygon3D>,
    options: &PolygonizerOptions,
) -> (Vec<Polygon3D>, Vec<Vec<Vec<Coord3D>>>, Vec<Vec<Vec<u32>>>) {
    let mut forest = ContainmentForest::new(&shells, &options.containment.index_backend);

    if options.extract_only_polygonal {
        let keep_mask = forest.filter_polygonal(&shells, &options.containment.touch_policy);
        let removed_count = keep_mask.iter().filter(|&&keep| !keep).count();

        if removed_count > 0 {
            let mut new_shells = Vec::new();

            for (keep, s) in keep_mask.into_iter().zip(shells) {
                if keep {
                    new_shells.push(s);
                }
            }
            shells = new_shells;
            forest = ContainmentForest::new(&shells, &options.containment.index_backend);
        }
    }

    let process_hole_assignment = |hole_3d: Polygon3D| -> Option<(usize, Vec<Coord3D>, Vec<u32>)> {
        let best_shell_idx =
            forest.assign_hole(&hole_3d, &shells, &options.containment.touch_policy);
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

    let mut shell_holes: Vec<Vec<Vec<Coord3D>>> = vec![vec![]; shells.len()];
    let mut shell_holes_ids: Vec<Vec<Vec<u32>>> = vec![vec![]; shells.len()];

    for (idx, hole_coords, hole_ids) in assignments {
        shell_holes[idx].push(hole_coords);
        shell_holes_ids[idx].push(hole_ids);
    }

    (shells, shell_holes, shell_holes_ids)
}

fn construct_final_polygons(
    shells: Vec<Polygon3D>,
    shell_holes: Vec<Vec<Vec<Coord3D>>>,
    shell_holes_ids: Vec<Vec<Vec<u32>>>,
    options: &PolygonizerOptions,
) -> Vec<Polygon3D> {
    let mut result = Vec::with_capacity(shells.len());
    for ((shell, mut holes), mut holes_ids) in shells
        .into_iter()
        .zip(shell_holes.into_iter())
        .zip(shell_holes_ids.into_iter())
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

        if p.unsigned_area_2d() > 1e-6 {
            result.push(p);
        }
    }
    result
}

fn apply_determinism(
    mut result: Vec<Polygon3D>,
    dangles: &mut [Vec<Coord3D>],
    invalid_rings: &mut Vec<Vec<Coord3D>>,
    options: &PolygonizerOptions,
) -> Vec<Polygon3D> {
    if options.determinism.canonical_sort {
        let use_stable_tie_breaks = options.determinism.stable_tie_breaks;
        if use_stable_tie_breaks {
            let mut result_with_cache: Vec<_> = result
                .into_iter()
                .map(|p| {
                    let area = p.exterior_unsigned_area_2d();
                    let b = bounding_rect_3d(&p.exterior).unwrap_or(geo::Rect::new(
                        geo::Coord { x: 0.0, y: 0.0 },
                        geo::Coord { x: 0.0, y: 0.0 },
                    ));
                    (p, area, b)
                })
                .collect();

            result_with_cache.sort_unstable_by(|(p1, area1, b1), (p2, area2, b2)| {
                area2.total_cmp(area1).then_with(|| {
                    b1.min()
                        .x
                        .total_cmp(&b2.min().x)
                        .then(b1.min().y.total_cmp(&b2.min().y))
                        .then(p1.interiors.len().cmp(&p2.interiors.len()))
                })
            });

            result = result_with_cache.into_iter().map(|(p, _, _)| p).collect();
        } else {
            let mut result_with_cache: Vec<_> = result
                .into_iter()
                .map(|p| {
                    let area = p.exterior_unsigned_area_2d();
                    (p, area)
                })
                .collect();

            result_with_cache.sort_unstable_by(|(_, area1), (_, area2)| area2.total_cmp(area1));

            result = result_with_cache.into_iter().map(|(p, _)| p).collect();
        }

        if options.determinism.canonical_ring_rotation {
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

pub fn rings_share_edge(shell: &[Coord3D], hole: &[Coord3D], eps: f64) -> bool {
    if shell.len() < 2 || hole.len() < 2 {
        return false;
    }

    // Bolt optimization: using .windows(2) is faster than loop indexing
    // because it eliminates O(N*M) array bounds checks in this hot inner loop.
    for shell_edge in shell.windows(2) {
        let a1 = shell_edge[0].to_coord_2d();
        let a2 = shell_edge[1].to_coord_2d();
        for hole_edge in hole.windows(2) {
            let b1 = hole_edge[0].to_coord_2d();
            let b2 = hole_edge[1].to_coord_2d();
            if segments_overlap_with_length(a1, a2, b1, b2, eps) {
                return true;
            }
        }
    }

    false
}

pub fn rings_touch_at_vertex(shell: &[Coord3D], hole: &[Coord3D], eps: f64) -> bool {
    let eps_sq = eps * eps;
    for s_pt in shell {
        for h_pt in hole {
            let dx = s_pt.x - h_pt.x;
            let dy = s_pt.y - h_pt.y;
            if dx * dx + dy * dy <= eps_sq {
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

    (overlap_end - overlap_start) * a_len_sq.sqrt() > eps
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
}
