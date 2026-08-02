use crate::diagnostics::ExecutionWorkTracker;
use crate::noding::snap::{FloatingIntersectionTrace, SnapNoder};
use crate::noding::CandidatePair;
use crate::options::ExecutionPolicy;
use crate::trace::{TraceCapture, TraceCaptureBudget};
use crate::types::{Coord3D, Line3D};
use crate::utils::soa::SoALines;
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};
use geo::Coord;
use smallvec::SmallVec;
use wide::f64x4;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

fn segment_cells(
    line: &Line3D,
    min_x: f64,
    min_y: f64,
    cell_size: f64,
    cols: usize,
    rows: usize,
) -> SmallVec<[usize; 64]> {
    let mut cells = SmallVec::new();
    if cols == 0 || rows == 0 {
        return cells;
    }

    let cell = |x: f64, y: f64| {
        (
            (((x - min_x) / cell_size).floor() as isize).clamp(0, cols as isize - 1),
            (((y - min_y) / cell_size).floor() as isize).clamp(0, rows as isize - 1),
        )
    };
    let push = |cells: &mut SmallVec<[usize; 64]>, col: isize, row: isize| {
        if col >= 0 && col < cols as isize && row >= 0 && row < rows as isize {
            let index = row as usize * cols + col as usize;
            if cells.last() != Some(&index) {
                cells.push(index);
            }
        }
    };

    let dx = line.end.x - line.start.x;
    let dy = line.end.y - line.start.y;
    let step_x = if dx > 0.0 {
        1
    } else if dx < 0.0 {
        -1
    } else {
        0
    };
    let step_y = if dy > 0.0 {
        1
    } else if dy < 0.0 {
        -1
    } else {
        0
    };
    let on_boundary = |value: f64| (value - value.round()).abs() <= 1e-12;
    let (mut col, mut row) = cell(line.start.x, line.start.y);
    let (end_col, end_row) = cell(line.end.x, line.end.y);
    push(&mut cells, col, row);

    let t_delta_x = if dx == 0.0 {
        f64::INFINITY
    } else {
        cell_size / dx.abs()
    };
    let t_delta_y = if dy == 0.0 {
        f64::INFINITY
    } else {
        cell_size / dy.abs()
    };
    let next_x = min_x + (col + usize::from(step_x > 0) as isize) as f64 * cell_size;
    let next_y = min_y + (row + usize::from(step_y > 0) as isize) as f64 * cell_size;
    let mut t_max_x = if dx == 0.0 {
        f64::INFINITY
    } else {
        ((next_x - line.start.x) / dx).max(0.0)
    };
    let mut t_max_y = if dy == 0.0 {
        f64::INFINITY
    } else {
        ((next_y - line.start.y) / dy).max(0.0)
    };

    while col != end_col || row != end_row {
        let tolerance = f64::EPSILON * t_max_x.abs().max(t_max_y.abs()).max(1.0) * 8.0;
        if t_max_x.min(t_max_y) > 1.0 + tolerance {
            break;
        }
        if t_max_x.is_finite() && t_max_y.is_finite() && (t_max_x - t_max_y).abs() <= tolerance {
            push(&mut cells, col + step_x, row);
            push(&mut cells, col, row + step_y);
            col += step_x;
            row += step_y;
            t_max_x += t_delta_x;
            t_max_y += t_delta_y;
        } else if t_max_x < t_max_y {
            col += step_x;
            t_max_x += t_delta_x;
        } else {
            row += step_y;
            t_max_y += t_delta_y;
        }
        push(&mut cells, col, row);
    }
    push(&mut cells, end_col, end_row);

    let vertical_boundary = dx == 0.0 && on_boundary((line.start.x - min_x) / cell_size);
    let horizontal_boundary = dy == 0.0 && on_boundary((line.start.y - min_y) / cell_size);
    let traversed = cells.clone();
    for index in traversed {
        let cell_col = (index % cols) as isize;
        let cell_row = (index / cols) as isize;
        if vertical_boundary {
            push(&mut cells, cell_col - 1, cell_row);
        }
        if horizontal_boundary {
            push(&mut cells, cell_col, cell_row - 1);
        }
        if vertical_boundary && horizontal_boundary {
            push(&mut cells, cell_col - 1, cell_row - 1);
        }
    }

    for point in [line.start, line.end] {
        let grid_x = (point.x - min_x) / cell_size;
        let grid_y = (point.y - min_y) / cell_size;
        let (point_col, point_row) = cell(point.x, point.y);
        if on_boundary(grid_x) {
            push(&mut cells, point_col - 1, point_row);
        }
        if on_boundary(grid_y) {
            push(&mut cells, point_col, point_row - 1);
        }
        if on_boundary(grid_x) && on_boundary(grid_y) {
            push(&mut cells, point_col - 1, point_row - 1);
        }
    }

    cells.sort_unstable();
    cells.dedup();

    cells
}

fn serial_bounds(
    lines: &[Line3D],
    execution_policy: Option<&ExecutionPolicy>,
) -> crate::Result<(f64, f64, f64, f64)> {
    let mut bounds = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (index, line) in lines.iter().enumerate() {
        if let Some(execution_policy) = execution_policy {
            execution_policy.check_cancelled_every("grid_construction", index)?;
        }
        bounds.0 = bounds.0.min(line.start.x.min(line.end.x));
        bounds.1 = bounds.1.min(line.start.y.min(line.end.y));
        bounds.2 = bounds.2.max(line.start.x.max(line.end.x));
        bounds.3 = bounds.3.max(line.start.y.max(line.end.y));
    }
    Ok(bounds)
}

#[allow(clippy::too_many_arguments)]
fn count_cells(
    lines: &[Line3D],
    min_x: f64,
    min_y: f64,
    cell_size: f64,
    cols: usize,
    rows: usize,
    execution_policy: Option<&ExecutionPolicy>,
    max_cells_per_line: usize,
) -> crate::Result<(Vec<usize>, Vec<usize>)> {
    let cell_count = cols.checked_mul(rows).ok_or_else(|| {
        crate::PolygonizeError::InternalInvariantViolation {
            reason: "uniform-grid cell count overflow".to_string(),
        }
    })?;
    let mut counts = vec![0usize; cell_count];
    let mut global_lines = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if let Some(execution_policy) = execution_policy {
            execution_policy.check_cancelled_every("grid_construction", index)?;
        }
        let cells = segment_cells(line, min_x, min_y, cell_size, cols, rows);
        if cells.len() > max_cells_per_line {
            global_lines.push(index);
            continue;
        }
        for cell in cells {
            counts[cell] = counts[cell].checked_add(1).ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: "uniform-grid cell population overflow".to_string(),
                }
            })?;
        }
    }
    Ok((counts, global_lines))
}

pub struct UniformGrid {
    /// Compressed Sparse Row (CSR) storage
    /// cell_offsets[i]..cell_offsets[i+1] gives the range in cell_items for cell i
    cell_offsets: Vec<usize>,
    /// Flattened list of line indices in each cell
    cell_items: Vec<u32>,
    /// Lines that span too many cells and are handled via brute-force
    pub(crate) global_lines: Vec<usize>,
    cell_size: f64,
    cols: usize,
    rows: usize,
    bounds_min: Coord<f64>,
}

pub(crate) struct UniformGridCellTrace {
    pub(crate) iteration_index: usize,
    pub(crate) row: usize,
    pub(crate) column: usize,
    pub(crate) min: Coord3D,
    pub(crate) max: Coord3D,
    pub(crate) segment_indices: Vec<usize>,
    pub(crate) source_ids: Vec<u32>,
}

pub(crate) struct UniformGridGlobalLineTrace {
    pub(crate) iteration_index: usize,
    pub(crate) segment_index: usize,
    pub(crate) source_id: u32,
}

pub(crate) struct UniformGridCandidateTrace {
    pub(crate) iteration_index: usize,
    pub(crate) scan: &'static str,
    pub(crate) row: Option<usize>,
    pub(crate) column: Option<usize>,
    pub(crate) first_segment: usize,
    pub(crate) second_segment: usize,
    pub(crate) first_source_id: u32,
    pub(crate) second_source_id: u32,
    pub(crate) witness: Option<FloatingIntersectionTrace>,
    pub(crate) owned_by_cell: Option<bool>,
}

impl UniformGrid {
    pub fn new(lines: &[Line3D]) -> Self {
        Self::new_impl(lines, None).expect("unlimited grid construction cannot fail")
    }

    pub(crate) fn new_with_execution_policy(
        lines: &[Line3D],
        execution_policy: &ExecutionPolicy,
    ) -> crate::Result<Self> {
        Self::new_impl(lines, Some(execution_policy))
    }

    fn new_impl(
        lines: &[Line3D],
        execution_policy: Option<&ExecutionPolicy>,
    ) -> crate::Result<Self> {
        if let Some(execution_policy) = execution_policy {
            execution_policy.check_cancelled("grid_construction")?;
        }
        if lines.is_empty() {
            return Ok(Self::empty());
        }

        // 1. Calculate Bounds & Heuristics
        #[cfg(feature = "parallel")]
        let (min_x, min_y, max_x, max_y) = if execution_policy.is_some() {
            serial_bounds(lines, execution_policy)?
        } else {
            lines
                .par_iter()
                .fold(
                    || (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                    |acc, line| {
                        (
                            acc.0.min(line.start.x.min(line.end.x)),
                            acc.1.min(line.start.y.min(line.end.y)),
                            acc.2.max(line.start.x.max(line.end.x)),
                            acc.3.max(line.start.y.max(line.end.y)),
                        )
                    },
                )
                .reduce(
                    || (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                    |a, b| (a.0.min(b.0), a.1.min(b.1), a.2.max(b.2), a.3.max(b.3)),
                )
        };

        #[cfg(not(feature = "parallel"))]
        let (min_x, min_y, max_x, max_y) = serial_bounds(lines, execution_policy)?;

        let width = max_x - min_x;
        let height = max_y - min_y;

        // 2. Determine Cell Size
        // Strategy: Target roughly ~4-8 lines per cell for optimal brute-force speed.
        // Formula: sqrt(Area / N) * Tunable_Factor
        let area = width * height;
        // Fallback for zero area (collinear lines)
        let area = if area < 1e-9 {
            lines.len() as f64
        } else {
            area
        };

        let target_cell_size = (area / lines.len() as f64).sqrt();
        // Clamp to avoid degenerate grids
        // Ensure cell_size isn't too small relative to the bounds
        let mut cell_size = target_cell_size.max(width.max(height) / 1000.0).max(1e-6);

        let mut cols = 0;
        let mut rows = 0;
        let mut counts = Vec::new();
        let mut global_lines = Vec::new();

        // ponytail: cap pathological axis-length traversals; raise only if global fallback profiles worse.
        const MAX_CELLS_PER_LINE: usize = 2048;
        const SKEW_THRESHOLD: usize = 500;
        const MAX_ADAPTIVE_RETRIES: usize = 2;

        // 3. Initialize & Populate (Two-Pass CSR Construction with Adaptive Regrid)
        for loop_idx in 0..=MAX_ADAPTIVE_RETRIES {
            // Calculate dimensions
            // Add small epsilon to ensure max boundary falls into a valid cell
            cols = ((width + 1e-9) / cell_size).ceil() as usize;
            rows = ((height + 1e-9) / cell_size).ceil() as usize;
            let cell_count = cols.checked_mul(rows).ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: "uniform-grid cell count overflow".to_string(),
                }
            })?;

            // Pass 1: Count entries per cell & Identify Global Lines
            #[cfg(feature = "parallel")]
            let (pass_counts, pass_globals) = if execution_policy.is_some() {
                count_cells(
                    lines,
                    min_x,
                    min_y,
                    cell_size,
                    cols,
                    rows,
                    execution_policy,
                    MAX_CELLS_PER_LINE,
                )?
            } else {
                let chunk_size = (lines.len() / rayon::current_num_threads()).max(1);
                let results: Vec<_> = lines
                    .par_chunks(chunk_size)
                    .enumerate()
                    .map(|(chunk_idx, chunk)| {
                        let mut local_counts = vec![0usize; cell_count];
                        let mut local_global = Vec::new();
                        let start_i = chunk_idx * chunk_size;

                        for (local_i, line) in chunk.iter().enumerate() {
                            let i = start_i + local_i;
                            let cells = segment_cells(line, min_x, min_y, cell_size, cols, rows);
                            if cells.len() > MAX_CELLS_PER_LINE {
                                local_global.push(i);
                                continue;
                            }
                            for cell in cells {
                                local_counts[cell] = local_counts[cell].saturating_add(1);
                            }
                        }
                        (local_counts, local_global)
                    })
                    .collect();

                let mut final_counts = vec![0usize; cell_count];
                let mut final_globals = Vec::new();
                for (local_counts, local_global) in results {
                    for i in 0..final_counts.len() {
                        final_counts[i] = final_counts[i].saturating_add(local_counts[i]);
                    }
                    final_globals.extend(local_global);
                }
                (final_counts, final_globals)
            };

            #[cfg(not(feature = "parallel"))]
            let (pass_counts, pass_globals) = count_cells(
                lines,
                min_x,
                min_y,
                cell_size,
                cols,
                rows,
                execution_policy,
                MAX_CELLS_PER_LINE,
            )?;

            counts = pass_counts;
            global_lines = pass_globals;

            // Adaptive check
            let max_cell_count = counts.iter().copied().max().unwrap_or(0);
            if max_cell_count > SKEW_THRESHOLD && loop_idx < MAX_ADAPTIVE_RETRIES {
                // Skew detected: too many lines in a single cell.
                // Refine grid by halving cell size, unless we have too many cells.
                // We limit total cells to avoid memory explosion.
                if cell_count.saturating_mul(4) < 1_000_000 {
                    cell_size /= 2.0;
                    continue; // Retry with finer grid
                }
            }

            // If we're here, either we didn't hit the threshold or we can't refine further.
            break;
        }

        // Prefix Sum to create offsets
        let cell_count = cols.checked_mul(rows).ok_or_else(|| {
            crate::PolygonizeError::InternalInvariantViolation {
                reason: "uniform-grid cell count overflow".to_string(),
            }
        })?;
        let mut cell_offsets = Vec::with_capacity(cell_count.checked_add(1).ok_or_else(|| {
            crate::PolygonizeError::InternalInvariantViolation {
                reason: "uniform-grid offset count overflow".to_string(),
            }
        })?);
        let mut sum = 0usize;
        cell_offsets.push(0);
        for count in counts {
            sum = sum.checked_add(count).ok_or_else(|| {
                crate::PolygonizeError::InternalInvariantViolation {
                    reason: "uniform-grid item count overflow".to_string(),
                }
            })?;
            cell_offsets.push(sum);
        }

        // Pass 2: Populate cell_items
        let mut cell_items = vec![0u32; sum];
        // We need a running tracker of where to insert in each cell.
        // Copy the start offsets to use as current write pointers.
        let mut current_offsets = cell_offsets[0..cell_count].to_vec();

        // Use peekable iterator to skip global lines efficiently
        let mut global_iter = global_lines.iter().peekable();

        for (i, line) in lines.iter().enumerate() {
            if let Some(execution_policy) = execution_policy {
                execution_policy.check_cancelled_every("grid_construction", i)?;
            }
            // Check if this line is global
            if let Some(&&global_idx) = global_iter.peek() {
                if i == global_idx {
                    global_iter.next();
                    continue;
                }
            }

            for cell in segment_cells(line, min_x, min_y, cell_size, cols, rows) {
                let write_pos = current_offsets[cell];
                cell_items[write_pos] = u32::try_from(i).map_err(|_| {
                    crate::PolygonizeError::InternalInvariantViolation {
                        reason: "uniform-grid line index exceeds u32".to_string(),
                    }
                })?;
                current_offsets[cell] = current_offsets[cell].checked_add(1).ok_or_else(|| {
                    crate::PolygonizeError::InternalInvariantViolation {
                        reason: "uniform-grid write offset overflow".to_string(),
                    }
                })?;
            }
        }

        Ok(Self {
            cell_offsets,
            cell_items,
            global_lines,
            cell_size,
            cols,
            rows,
            bounds_min: Coord { x: min_x, y: min_y },
        })
    }

    fn empty() -> Self {
        Self {
            cell_offsets: vec![0],
            cell_items: vec![],
            global_lines: vec![],
            cell_size: 1.0,
            cols: 0,
            rows: 0,
            bounds_min: Coord::zero(),
        }
    }

    pub(crate) fn trace_structure(
        &self,
        lines: &[Line3D],
        iteration_index: usize,
        budget: &mut TraceCaptureBudget,
    ) -> (Vec<UniformGridCellTrace>, Vec<UniformGridGlobalLineTrace>) {
        let mut cells = Vec::new();
        for index in 0..self.rows * self.cols {
            let start = self.cell_offsets[index];
            let end = self.cell_offsets[index + 1];
            if end - start < 2 {
                continue;
            }
            let member_bytes = (end - start)
                .saturating_mul(std::mem::size_of::<usize>() + std::mem::size_of::<u32>());
            let capture_bytes =
                std::mem::size_of::<UniformGridCellTrace>().saturating_add(member_bytes);
            if !budget.take(capture_bytes) {
                break;
            }
            let row = index / self.cols;
            let column = index % self.cols;
            let segment_indices: Vec<_> = self.cell_items[start..end]
                .iter()
                .map(|&segment| segment as usize)
                .collect();
            cells.push(UniformGridCellTrace {
                iteration_index,
                row,
                column,
                min: Coord3D::new(
                    self.bounds_min.x + column as f64 * self.cell_size,
                    self.bounds_min.y + row as f64 * self.cell_size,
                    0.0,
                ),
                max: Coord3D::new(
                    self.bounds_min.x + (column + 1) as f64 * self.cell_size,
                    self.bounds_min.y + (row + 1) as f64 * self.cell_size,
                    0.0,
                ),
                source_ids: segment_indices
                    .iter()
                    .map(|&segment| lines[segment].line_id)
                    .collect(),
                segment_indices,
            });
        }
        let mut global_lines = Vec::new();
        for &segment_index in &self.global_lines {
            let line = UniformGridGlobalLineTrace {
                iteration_index,
                segment_index,
                source_id: lines[segment_index].line_id,
            };
            if !budget.capture(&mut global_lines, line) {
                break;
            }
        }
        (cells, global_lines)
    }

    /// Finds all intersections. Uses "Intersection Ownership" to deduplicate checks.
    /// Returns a flat list of (line_index, point) tuples.
    pub fn find_splits(&self, lines: &[Line3D], snap_noder: &SnapNoder) -> Vec<(usize, Coord3D)> {
        // Instantiate SoA structure for fast AABB checks
        let soa = SoALines::new(lines);

        #[cfg(feature = "parallel")]
        let mut splits = {
            let chunks = std::sync::Mutex::new(Vec::new());
            (0..self.rows * self.cols)
                .into_par_iter()
                .fold(Vec::new, |mut acc, idx| {
                    let start = self.cell_offsets[idx];
                    let end = self.cell_offsets[idx + 1];
                    let cell_indices = &self.cell_items[start..end];

                    if cell_indices.len() >= 2 {
                        let r = idx / self.cols;
                        let c = idx % self.cols;
                        self.process_cell(
                            r,
                            c,
                            cell_indices,
                            lines,
                            snap_noder,
                            &soa,
                            &mut acc,
                            None,
                            0,
                            None,
                        )
                        .expect("unlimited noding cannot fail");
                    }
                    acc
                })
                .for_each(|chunk| chunks.lock().unwrap().push(chunk));
            let chunks = chunks.into_inner().unwrap();
            let mut splits = Vec::with_capacity(chunks.iter().map(Vec::len).sum());
            for chunk in chunks {
                splits.extend(chunk);
            }
            splits
        };

        #[cfg(not(feature = "parallel"))]
        let mut splits = {
            let mut splits = Vec::new();
            for r in 0..self.rows {
                for c in 0..self.cols {
                    let idx = r * self.cols + c;
                    let start = self.cell_offsets[idx];
                    let end = self.cell_offsets[idx + 1];
                    let cell_indices = &self.cell_items[start..end];

                    self.process_cell(
                        r,
                        c,
                        cell_indices,
                        lines,
                        snap_noder,
                        &soa,
                        &mut splits,
                        None,
                        0,
                        None,
                    )
                    .expect("unlimited noding cannot fail");
                }
            }
            splits
        };

        // Process Global Lines (Brute Force against ALL lines)
        if !self.global_lines.is_empty() {
            let global_splits = self.process_global_lines(lines, snap_noder, &soa);
            splits.extend(global_splits);
        }

        splits
    }

    pub(crate) fn find_splits_tracked(
        &self,
        lines: &[Line3D],
        snap_noder: &SnapNoder,
        tracker: &mut ExecutionWorkTracker<'_>,
        iteration_index: usize,
        mut trace_candidates: Option<&mut TraceCapture<'_, UniformGridCandidateTrace>>,
    ) -> crate::Result<Vec<(usize, Coord3D)>> {
        let soa = SoALines::new(lines);
        let mut splits = Vec::new();
        tracker.check_cancelled()?;
        tracker.grid(
            self.rows.saturating_mul(self.cols),
            self.cell_items.len(),
            self.global_lines.len(),
        );
        for idx in 0..self.rows * self.cols {
            let start = self.cell_offsets[idx];
            let end = self.cell_offsets[idx + 1];
            self.process_cell(
                idx / self.cols,
                idx % self.cols,
                &self.cell_items[start..end],
                lines,
                snap_noder,
                &soa,
                &mut splits,
                Some(tracker),
                iteration_index,
                trace_candidates.as_deref_mut(),
            )?;
        }
        if !self.global_lines.is_empty() {
            splits.extend(self.process_global_lines_tracked(
                lines,
                snap_noder,
                tracker,
                iteration_index,
                trace_candidates,
            )?);
        }
        tracker.check_cancelled()?;
        Ok(splits)
    }

    // Broad phase: enumerate AABB-overlapping global-line pairs without calling
    // the exact intersection predicate.
    fn enumerate_global_candidates_for_line(
        &self,
        g_idx: usize,
        lines: &[Line3D],
        soa: &SoALines,
        mut tracker: Option<&mut ExecutionWorkTracker<'_>>,
    ) -> crate::Result<Vec<CandidatePair>> {
        let global_lines = &self.global_lines;
        let query_line = lines[g_idx];
        let q_min_x = f64x4::splat(query_line.start.x.min(query_line.end.x));
        let q_max_x = f64x4::splat(query_line.start.x.max(query_line.end.x));
        let q_min_y = f64x4::splat(query_line.start.y.min(query_line.end.y));
        let q_max_y = f64x4::splat(query_line.start.y.max(query_line.end.y));
        let mut candidates = Vec::new();

        for block_idx in (0..soa.len()).step_by(4) {
            let mask =
                soa.intersects_bbox_batch_splatted(q_min_x, q_max_x, q_min_y, q_max_y, block_idx);
            for k in 0..4 {
                let target_idx = block_idx + k;
                if target_idx >= lines.len()
                    || target_idx == g_idx
                    || (global_lines.binary_search(&target_idx).is_ok() && target_idx < g_idx)
                {
                    continue;
                }
                let overlaps = (mask & (1 << k)) != 0;
                if let Some(tracker) = tracker.as_deref_mut() {
                    tracker.candidate(overlaps)?;
                }
                if overlaps {
                    candidates.push(CandidatePair {
                        first: g_idx,
                        second: target_idx,
                    });
                }
            }
        }

        if let Some(tracker) = tracker {
            tracker.check_cancelled()?;
        }
        Ok(candidates)
    }

    // Exact phase: robust intersection, optional trace capture, and split accumulation.
    fn process_global_candidate(
        &self,
        candidate: CandidatePair,
        lines: &[Line3D],
        snap_noder: &SnapNoder,
        iteration_index: usize,
        trace_candidates: Option<&mut TraceCapture<'_, UniformGridCandidateTrace>>,
    ) -> Vec<(usize, Coord3D)> {
        let first = lines[candidate.first];
        let second = lines[candidate.second];
        let intersection = line_intersection(first.to_line_2d(), second.to_line_2d());
        let witness = intersection.map(|intersection| match intersection {
            LineIntersection::SinglePoint { intersection, .. } => {
                FloatingIntersectionTrace::Point(Coord3D::new(intersection.x, intersection.y, 0.0))
            }
            LineIntersection::Collinear { intersection } => FloatingIntersectionTrace::Collinear(
                Coord3D::new(intersection.start.x, intersection.start.y, 0.0),
                Coord3D::new(intersection.end.x, intersection.end.y, 0.0),
            ),
        });
        if let Some(trace_candidates) = trace_candidates {
            trace_candidates.push(UniformGridCandidateTrace {
                iteration_index,
                scan: "global_line",
                row: None,
                column: None,
                first_segment: candidate.first,
                second_segment: candidate.second,
                first_source_id: first.line_id,
                second_source_id: second.line_id,
                witness,
                owned_by_cell: None,
            });
        }
        let mut splits = Vec::new();
        if let Some(intersection) = intersection {
            snap_noder.handle_intersection(
                intersection,
                candidate.first,
                candidate.second,
                first,
                second,
                |idx, point| splits.push((idx, point)),
            );
        }
        splits
    }

    fn process_global_lines(
        &self,
        lines: &[Line3D],
        snap_noder: &SnapNoder,
        soa: &SoALines,
    ) -> Vec<(usize, Coord3D)> {
        let global_lines = &self.global_lines;
        let process_one_global = |g_idx: usize| -> Vec<(usize, Coord3D)> {
            let candidates = self
                .enumerate_global_candidates_for_line(g_idx, lines, soa, None)
                .expect("unlimited noding cannot fail");
            candidates
                .into_iter()
                .flat_map(|candidate| {
                    self.process_global_candidate(candidate, lines, snap_noder, 0, None)
                })
                .collect()
        };

        #[cfg(feature = "parallel")]
        {
            global_lines
                .par_iter()
                .map(|&g_idx| process_one_global(g_idx))
                .reduce(Vec::new, |mut a, mut b| {
                    a.append(&mut b);
                    a
                })
        }

        #[cfg(not(feature = "parallel"))]
        {
            let mut all_events = Vec::new();
            for &g_idx in global_lines {
                let events = process_one_global(g_idx);
                all_events.extend(events);
            }
            all_events
        }
    }

    fn process_global_lines_tracked(
        &self,
        lines: &[Line3D],
        snap_noder: &SnapNoder,
        tracker: &mut ExecutionWorkTracker<'_>,
        iteration_index: usize,
        mut trace_candidates: Option<&mut TraceCapture<'_, UniformGridCandidateTrace>>,
    ) -> crate::Result<Vec<(usize, Coord3D)>> {
        let mut events = Vec::new();
        let soa = SoALines::new(lines);
        for &left_idx in &self.global_lines {
            let candidates =
                self.enumerate_global_candidates_for_line(left_idx, lines, &soa, Some(tracker))?;
            for candidate in candidates {
                events.extend(self.process_global_candidate(
                    candidate,
                    lines,
                    snap_noder,
                    iteration_index,
                    trace_candidates.as_deref_mut(),
                ));
            }
        }
        Ok(events)
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn handle_collinear(
        &self,
        c: usize,
        r: usize,
        cell_min_x: f64,
        cell_max_x: f64,
        cell_min_y: f64,
        cell_max_y: f64,
        overlap: geo::Line<f64>,
        snap_noder: &SnapNoder,
        res: LineIntersection<f64>,
        idx1: usize,
        idx2: usize,
        l1: Line3D,
        l2: Line3D,
        splits: &mut Vec<(usize, Coord3D)>,
    ) -> bool {
        // Collinear is rare. Just process start/end and let HashMap dedup later.
        // SnapNoder::snap expects Coord3D. Overlap has 2D coords.
        // We construct dummy 3D coords (Z=0).
        let p1 = snap_noder
            .snap(Coord3D::new(overlap.start.x, overlap.start.y, 0.0))
            .to_coord_2d();
        // Simplified ownership: Check if p1 is in cell
        let p1_in =
            p1.x >= cell_min_x && p1.x < cell_max_x && p1.y >= cell_min_y && p1.y < cell_max_y;
        if p1_in || (c == 0 && r == 0) {
            snap_noder.handle_intersection(res, idx1, idx2, l1, l2, |idx, pt| {
                splits.push((idx, pt));
            });
            true
        } else {
            false
        }
    }

    // Broad phase: enumerate AABB-overlapping pairs that share a grid cell.
    fn enumerate_cell_candidates(
        &self,
        cell_indices: &[u32],
        soa: &SoALines,
        mut tracker: Option<&mut ExecutionWorkTracker<'_>>,
    ) -> crate::Result<Vec<CandidatePair>> {
        let mut candidates = Vec::new();
        for i in 0..cell_indices.len() {
            for j in (i + 1)..cell_indices.len() {
                let first = cell_indices[i] as usize;
                let second = cell_indices[j] as usize;
                let overlaps = soa.max_x[first] >= soa.min_x[second]
                    && soa.min_x[first] <= soa.max_x[second]
                    && soa.max_y[first] >= soa.min_y[second]
                    && soa.min_y[first] <= soa.max_y[second];
                if let Some(tracker) = tracker.as_deref_mut() {
                    tracker.candidate(overlaps)?;
                }
                if overlaps {
                    candidates.push(CandidatePair { first, second });
                }
            }
        }
        Ok(candidates)
    }

    // Exact phase: robust intersection, cell ownership, trace capture, and
    // split accumulation.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_cell_candidate(
        &self,
        r: usize,
        c: usize,
        candidate: CandidatePair,
        lines: &[Line3D],
        snap_noder: &SnapNoder,
        splits: &mut Vec<(usize, Coord3D)>,
        iteration_index: usize,
        trace_candidates: Option<&mut TraceCapture<'_, UniformGridCandidateTrace>>,
    ) {
        let cell_min_x = self.bounds_min.x + c as f64 * self.cell_size;
        let cell_min_y = self.bounds_min.y + r as f64 * self.cell_size;
        let cell_max_x = cell_min_x + self.cell_size;
        let cell_max_y = cell_min_y + self.cell_size;
        let idx1 = candidate.first;
        let idx2 = candidate.second;
        let l1 = lines[idx1];
        let l2 = lines[idx2];
        let intersection = line_intersection(l1.to_line_2d(), l2.to_line_2d());
        let witness = intersection.map(|intersection| match intersection {
            LineIntersection::SinglePoint { intersection, .. } => {
                FloatingIntersectionTrace::Point(Coord3D::new(intersection.x, intersection.y, 0.0))
            }
            LineIntersection::Collinear { intersection } => FloatingIntersectionTrace::Collinear(
                Coord3D::new(intersection.start.x, intersection.start.y, 0.0),
                Coord3D::new(intersection.end.x, intersection.end.y, 0.0),
            ),
        });
        let mut owned_by_cell = false;
        if let Some(res) = intersection {
            match res {
                LineIntersection::SinglePoint {
                    intersection: pt, ..
                } => {
                    // A pair may occur in multiple cells; only its owning cell
                    // contributes the exact split events.
                    let is_in_x = pt.x >= cell_min_x
                        && (pt.x < cell_max_x || (c == self.cols - 1 && pt.x <= cell_max_x));
                    let is_in_y = pt.y >= cell_min_y
                        && (pt.y < cell_max_y || (r == self.rows - 1 && pt.y <= cell_max_y));
                    if is_in_x && is_in_y {
                        owned_by_cell = true;
                        snap_noder.handle_intersection(res, idx1, idx2, l1, l2, |idx, pt| {
                            splits.push((idx, pt));
                        });
                    }
                }
                LineIntersection::Collinear {
                    intersection: overlap,
                } => {
                    owned_by_cell = self.handle_collinear(
                        c, r, cell_min_x, cell_max_x, cell_min_y, cell_max_y, overlap, snap_noder,
                        res, idx1, idx2, l1, l2, splits,
                    );
                }
            }
        }
        if let Some(trace_candidates) = trace_candidates {
            trace_candidates.push(UniformGridCandidateTrace {
                iteration_index,
                scan: "cell",
                row: Some(r),
                column: Some(c),
                first_segment: idx1,
                second_segment: idx2,
                first_source_id: l1.line_id,
                second_source_id: l2.line_id,
                witness,
                owned_by_cell: Some(owned_by_cell),
            });
        }
    }

    #[inline]
    #[allow(clippy::too_many_arguments)]
    fn process_cell(
        &self,
        r: usize,
        c: usize,
        cell_indices: &[u32],
        lines: &[Line3D],
        snap_noder: &SnapNoder,
        soa: &SoALines,
        splits: &mut Vec<(usize, Coord3D)>,
        tracker: Option<&mut ExecutionWorkTracker<'_>>,
        iteration_index: usize,
        mut trace_candidates: Option<&mut TraceCapture<'_, UniformGridCandidateTrace>>,
    ) -> crate::Result<()> {
        if cell_indices.len() < 2 {
            return Ok(());
        }

        let candidates = self.enumerate_cell_candidates(cell_indices, soa, tracker)?;
        for candidate in candidates {
            self.process_cell_candidate(
                r,
                c,
                candidate,
                lines,
                snap_noder,
                splits,
                iteration_index,
                trace_candidates.as_deref_mut(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::noding::snap::SnapNoder;
    use crate::types::{Coord3D, Line3D};
    use crate::{CancellationToken, ExecutionPolicy, PolygonizeError};
    use approx::assert_relative_eq;

    fn make_line(x1: f64, y1: f64, x2: f64, y2: f64) -> Line3D {
        Line3D::new(Coord3D::new(x1, y1, 0.0), Coord3D::new(x2, y2, 0.0), 0)
    }

    #[test]
    fn test_empty_grid() {
        let grid = UniformGrid::new(&[]);
        assert_eq!(grid.rows, 0);
        assert_eq!(grid.cols, 0);
        assert!(grid.cell_offsets.len() <= 1);
        assert!(grid.cell_items.is_empty());
    }

    #[test]
    fn test_empty_grid_find_splits() {
        let grid = UniformGrid::new(&[]);
        let noder = SnapNoder::new(1e-6);
        let splits = grid.find_splits(&[], &noder);
        assert!(splits.is_empty());
    }

    #[test]
    fn policy_path_observes_cancellation_before_returning() {
        let token = CancellationToken::new();
        token.cancel();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token),
            ..Default::default()
        };

        assert!(matches!(
            UniformGrid::new(&[]).find_splits_tracked(
                &[],
                &SnapNoder::new(1.0),
                &mut ExecutionWorkTracker::new(Some(&policy), None),
                0,
                None,
            ),
            Err(PolygonizeError::Cancelled { stage }) if stage == "candidate_enumeration"
        ));
    }

    #[test]
    fn global_line_trace_reuses_the_physical_intersection() {
        let lines = vec![
            Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(2.0, 2.0, 0.0), 1),
            Line3D::new(Coord3D::new(0.0, 2.0, 0.0), Coord3D::new(2.0, 0.0, 0.0), 2),
        ];
        let mut grid = UniformGrid::empty();
        grid.global_lines.push(0);
        let mut candidates = Vec::new();
        let mut budget = TraceCaptureBudget::new(usize::MAX);
        let mut trace = TraceCapture::new(&mut candidates, &mut budget);
        let splits = grid
            .find_splits_tracked(
                &lines,
                &SnapNoder::new(0.0),
                &mut ExecutionWorkTracker::new(None, None),
                0,
                Some(&mut trace),
            )
            .unwrap();

        assert_eq!(splits.len(), 2);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].scan, "global_line");
        assert_eq!(candidates[0].first_source_id, 1);
        assert_eq!(candidates[0].second_source_id, 2);
        assert!(matches!(
            candidates[0].witness,
            Some(FloatingIntersectionTrace::Point(point)) if point.x == 1.0 && point.y == 1.0
        ));
    }

    #[test]
    fn grid_construction_polls_during_input_passes() {
        let lines = (0..512)
            .map(|index| make_line(0.0, index as f64, 1.0, index as f64 + 0.5))
            .collect::<Vec<_>>();
        let token = CancellationToken::new();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            cancel_at_work_item: Some((token, 17)),
            ..Default::default()
        };

        assert!(matches!(
            UniformGrid::new_with_execution_policy(&lines, &policy),
            Err(PolygonizeError::Cancelled { stage }) if stage == "grid_construction"
        ));
    }

    #[test]
    fn dense_cell_stops_at_the_first_pair_over_budget() {
        let lines = (0..40)
            .map(|index| make_line(-1.0, index as f64 / 100.0, 1.0, 1.0 - index as f64 / 100.0))
            .collect::<Vec<_>>();
        let grid = UniformGrid::new(&lines);
        let policy = ExecutionPolicy {
            max_candidate_pairs: Some(256),
            ..Default::default()
        };
        let mut stats = crate::NodingWorkStats::default();
        let result = grid.find_splits_tracked(
            &lines,
            &SnapNoder::new(0.0),
            &mut ExecutionWorkTracker::new(Some(&policy), Some(&mut stats)),
            0,
            None,
        );

        assert!(matches!(
            result,
            Err(PolygonizeError::ResourceLimitExceeded {
                stage,
                limit: 256,
                observed: 257,
            }) if stage == "candidate_pairs"
        ));
        assert_eq!(stats.candidate_pairs, 257);
    }

    #[test]
    fn dense_cell_cancellation_latency_is_bounded_in_work_items() {
        let lines = (0..40)
            .map(|index| make_line(-1.0, index as f64 / 100.0, 1.0, 1.0 - index as f64 / 100.0))
            .collect::<Vec<_>>();
        let token = CancellationToken::new();
        let policy = ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            ..Default::default()
        };
        let mut stats = crate::NodingWorkStats::default();
        let result = UniformGrid::new(&lines).find_splits_tracked(
            &lines,
            &SnapNoder::new(0.0),
            &mut ExecutionWorkTracker::new(Some(&policy), Some(&mut stats))
                .cancel_at_candidate(token, 17),
            0,
            None,
        );

        assert!(matches!(
            result,
            Err(PolygonizeError::Cancelled { stage }) if stage == "candidate_enumeration"
        ));
        assert_eq!(
            stats.candidate_pairs,
            crate::options::CANCELLATION_CHECK_INTERVAL
        );
        assert!(stats.candidate_pairs < lines.len() * (lines.len() - 1) / 2);
    }

    #[test]
    fn test_new_with_empty_lines() {
        let lines: Vec<Line3D> = Vec::new();
        let grid = UniformGrid::new(&lines);

        // Ensure that the empty path initializes variables correctly
        assert_eq!(grid.rows, 0);
        assert_eq!(grid.cols, 0);
        assert_eq!(grid.cell_offsets, vec![0]);
        assert_eq!(grid.cell_items.len(), 0);
        assert_eq!(grid.global_lines.len(), 0);
        assert_relative_eq!(grid.cell_size, 1.0);
        assert_relative_eq!(grid.bounds_min.x, 0.0);
        assert_relative_eq!(grid.bounds_min.y, 0.0);
    }

    #[test]
    fn test_grid_dimensions() {
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 0.0),
            make_line(10.0, 0.0, 10.0, 10.0),
            make_line(10.0, 10.0, 0.0, 10.0),
            make_line(0.0, 10.0, 0.0, 0.0),
        ];

        let grid = UniformGrid::new(&lines);

        // Check bounds
        assert_relative_eq!(grid.bounds_min.x, 0.0);
        assert_relative_eq!(grid.bounds_min.y, 0.0);

        // Dimensions should be non-zero
        assert!(grid.rows > 0);
        assert!(grid.cols > 0);
        assert!(grid.cell_size > 0.0);

        // Verify total cells
        assert_eq!(grid.cell_offsets.len(), grid.rows * grid.cols + 1);
    }

    #[test]
    fn test_cell_population() {
        // Create 2 disjoint lines far apart
        let lines = vec![
            make_line(0.0, 0.0, 1.0, 1.0),   // Bottom-left
            make_line(9.0, 9.0, 10.0, 10.0), // Top-right
        ];

        let grid = UniformGrid::new(&lines);

        // Find cells containing line 0
        let mut cells_with_0 = 0;
        let mut cells_with_1 = 0;

        for i in 0..(grid.rows * grid.cols) {
            let start = grid.cell_offsets[i];
            let end = grid.cell_offsets[i + 1];
            let cell = &grid.cell_items[start..end];

            if cell.contains(&(0u32)) {
                cells_with_0 += 1;
            }
            if cell.contains(&(1u32)) {
                cells_with_1 += 1;
            }
        }

        assert!(cells_with_0 > 0, "Line 0 should be in at least one cell");
        assert!(cells_with_1 > 0, "Line 1 should be in at least one cell");

        // Verify they don't share any cells (given the distance and reasonable grid size)
        for i in 0..(grid.rows * grid.cols) {
            let start = grid.cell_offsets[i];
            let end = grid.cell_offsets[i + 1];
            let cell = &grid.cell_items[start..end];
            assert!(
                !(cell.contains(&(0u32)) && cell.contains(&(1u32))),
                "Lines far apart should not share a cell"
            );
        }
    }

    #[test]
    fn test_find_splits_intersection() {
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 10.0), // Diagonal /
            make_line(0.0, 10.0, 10.0, 0.0), // Diagonal \
        ];

        let grid = UniformGrid::new(&lines);
        let noder = SnapNoder::new(0.0); // Exact noding

        let splits = grid.find_splits(&lines, &noder);

        // Expect 2 events: (0, (5,5,0)) and (1, (5,5,0))
        assert_eq!(splits.len(), 2);

        // Order is not guaranteed, sort by index
        let mut sorted = splits.clone();
        sorted.sort_by_key(|k| k.0);

        assert_eq!(sorted[0].0, 0);
        assert_eq!(sorted[1].0, 1);

        let p0 = sorted[0].1;
        let p1 = sorted[1].1;

        assert_relative_eq!(p0.x, 5.0);
        assert_relative_eq!(p0.y, 5.0);
        assert_relative_eq!(p1.x, 5.0);
        assert_relative_eq!(p1.y, 5.0);
    }

    #[test]
    fn test_find_splits_no_intersection() {
        let lines = vec![
            make_line(0.0, 0.0, 10.0, 0.0),
            make_line(0.0, 1.0, 10.0, 1.0),
        ];

        let grid = UniformGrid::new(&lines);
        let noder = SnapNoder::new(0.0);

        let splits = grid.find_splits(&lines, &noder);
        assert!(splits.is_empty());
    }

    #[test]
    fn test_boundary_handling() {
        // Line crossing multiple cells horizontally should stay indexed by its traversed cells.
        let lines = vec![make_line(0.0, 0.0, 100.0, 0.0)];

        let grid = UniformGrid::new(&lines);

        // Ensure we have enough columns to test boundary crossing
        assert!(
            grid.cols > 1,
            "Grid should have multiple columns for this test case"
        );

        let mut cells_with_line = 0;
        for i in 0..(grid.rows * grid.cols) {
            let start = grid.cell_offsets[i];
            let end = grid.cell_offsets[i + 1];
            let cell = &grid.cell_items[start..end];
            if cell.contains(&(0u32)) {
                cells_with_line += 1;
            }
        }

        assert!(cells_with_line > 1);
        assert!(grid.global_lines.is_empty());
    }

    #[test]
    fn test_global_line_intersection() {
        // Scenario:
        // l0: Global line (y=0, x=0..2000)
        // l1: Local line intersecting l0 (x=1000, y=-0.01..0.01)
        // l2: Global line intersecting l0 (x=0..2000, y crossing 0 at x=1000)

        let l0 = make_line(0.0, 0.0, 2000.0, 0.0);
        let l1 = make_line(1000.0, -0.01, 1000.0, 0.01);
        let l2 = make_line(0.0, 1.0, 2000.0, -1.0);

        let lines = vec![l0, l1, l2];
        let grid = UniformGrid::new(&lines);

        assert!(grid.global_lines.is_empty());

        let noder = SnapNoder::new(0.0);
        let splits = grid.find_splits(&lines, &noder);

        // Expected intersections:
        // l0 and l1 at (1000, 0)
        // l0 and l2 at (1000, 0)
        // l1 and l2 at (1000, 0) (triple intersection)

        // Count events for each line
        let events_l0 = splits.iter().filter(|(idx, _)| *idx == 0).count();
        let events_l1 = splits.iter().filter(|(idx, _)| *idx == 1).count();
        let events_l2 = splits.iter().filter(|(idx, _)| *idx == 2).count();

        assert!(
            events_l0 >= 2,
            "l0 should have at least 2 intersection events"
        );
        assert!(
            events_l1 >= 2,
            "l1 should have at least 2 intersection events"
        );
        assert!(
            events_l2 >= 2,
            "l2 should have at least 2 intersection events"
        );

        // Verify coordinates
        for (_, pt) in &splits {
            assert_relative_eq!(pt.x, 1000.0, epsilon = 1e-5);
            assert_relative_eq!(pt.y, 0.0, epsilon = 1e-5);
        }
    }

    #[test]
    fn test_segment_cells_supercover_diagonal() {
        let line = make_line(0.1, 0.1, 9.9, 9.9);
        let cells = segment_cells(&line, 0.0, 0.0, 1.0, 10, 10);
        let reversed = make_line(9.9, 9.9, 0.1, 0.1);

        assert_eq!(cells.len(), 28);
        assert!(cells.contains(&1));
        assert!(cells.contains(&10));
        assert!(cells.contains(&11));
        assert_eq!(cells, segment_cells(&reversed, 0.0, 0.0, 1.0, 10, 10));
    }

    #[test]
    fn test_segment_cells_includes_both_sides_of_grid_boundary() {
        let line = make_line(5.0, 0.1, 5.0, 9.9);
        let cells = segment_cells(&line, 0.0, 0.0, 1.0, 10, 10);

        assert_eq!(cells.len(), 20);
        assert!(cells.iter().all(|index| index % 10 == 4 || index % 10 == 5));
    }

    #[test]
    fn test_adaptive_regrid_skewed_data() {
        let mut lines = Vec::new();
        // Create 600 lines all packed into a tiny area (skewed)
        for i in 0..600 {
            lines.push(make_line(
                0.0,
                0.0,
                0.0001 * (i as f64),
                0.0001 * (i as f64),
            ));
        }
        // Add one line far away to force a large initial grid size
        lines.push(make_line(100.0, 100.0, 101.0, 101.0));

        let grid = UniformGrid::new(&lines);

        // A non-adaptive grid would have one cell with 600 items.
        // But with adaptive regrid, the cell size should have been halved up to 2 times.
        // We can't easily assert on the exact inner cell sizes, but we can verify it succeeds
        // without panicking and has more rows/cols than it initially would have.
        // Initial area is 101*101 = 10201. Target cell size = sqrt(10201/601) ~ 4.12
        // So rows/cols would be 101/4.12 = 25.
        // Halved twice: cell size ~ 1.03, rows/cols ~ 98.
        assert!(grid.rows > 25);
        assert!(grid.cols > 25);
    }
}
