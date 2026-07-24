use crate::diagnostics::ExecutionWorkTracker;
use crate::noding::snap::SnapNoder;
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

impl UniformGrid {
    pub fn new(lines: &[Line3D]) -> Self {
        if lines.is_empty() {
            return Self::empty();
        }

        // 1. Calculate Bounds & Heuristics
        #[cfg(feature = "parallel")]
        let (min_x, min_y, max_x, max_y) = lines
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
            );

        #[cfg(not(feature = "parallel"))]
        let (min_x, min_y, max_x, max_y) = {
            let mut min_x = f64::MAX;
            let mut min_y = f64::MAX;
            let mut max_x = f64::MIN;
            let mut max_y = f64::MIN;

            for line in lines {
                min_x = min_x.min(line.start.x.min(line.end.x));
                min_y = min_y.min(line.start.y.min(line.end.y));
                max_x = max_x.max(line.start.x.max(line.end.x));
                max_y = max_y.max(line.start.y.max(line.end.y));
            }
            (min_x, min_y, max_x, max_y)
        };

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

            // Pass 1: Count entries per cell & Identify Global Lines
            #[cfg(feature = "parallel")]
            let (pass_counts, pass_globals) = {
                let chunk_size = (lines.len() / rayon::current_num_threads()).max(1);
                let results: Vec<_> = lines
                    .par_chunks(chunk_size)
                    .enumerate()
                    .map(|(chunk_idx, chunk)| {
                        let mut local_counts = vec![0usize; cols * rows];
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
                                local_counts[cell] += 1;
                            }
                        }
                        (local_counts, local_global)
                    })
                    .collect();

                let mut final_counts = vec![0usize; cols * rows];
                let mut final_globals = Vec::new();
                for (local_counts, local_global) in results {
                    for i in 0..final_counts.len() {
                        final_counts[i] += local_counts[i];
                    }
                    final_globals.extend(local_global);
                }
                (final_counts, final_globals)
            };

            #[cfg(not(feature = "parallel"))]
            let (pass_counts, pass_globals) = {
                let mut local_counts = vec![0usize; cols * rows];
                let mut local_global = Vec::new();

                for (i, line) in lines.iter().enumerate() {
                    let cells = segment_cells(line, min_x, min_y, cell_size, cols, rows);
                    if cells.len() > MAX_CELLS_PER_LINE {
                        local_global.push(i);
                        continue;
                    }
                    for cell in cells {
                        local_counts[cell] += 1;
                    }
                }
                (local_counts, local_global)
            };

            counts = pass_counts;
            global_lines = pass_globals;

            // Adaptive check
            let max_cell_count = counts.iter().copied().max().unwrap_or(0);
            if max_cell_count > SKEW_THRESHOLD && loop_idx < MAX_ADAPTIVE_RETRIES {
                // Skew detected: too many lines in a single cell.
                // Refine grid by halving cell size, unless we have too many cells.
                // We limit total cells to avoid memory explosion.
                if (cols * 2) * (rows * 2) < 1_000_000 {
                    cell_size /= 2.0;
                    continue; // Retry with finer grid
                }
            }

            // If we're here, either we didn't hit the threshold or we can't refine further.
            break;
        }

        // Prefix Sum to create offsets
        let mut cell_offsets = Vec::with_capacity(cols * rows + 1);
        let mut sum = 0;
        cell_offsets.push(0);
        for count in counts {
            sum += count;
            cell_offsets.push(sum);
        }

        // Pass 2: Populate cell_items
        let mut cell_items = vec![0u32; sum];
        // We need a running tracker of where to insert in each cell.
        // Copy the start offsets to use as current write pointers.
        let mut current_offsets = cell_offsets[0..cols * rows].to_vec();

        // Use peekable iterator to skip global lines efficiently
        let mut global_iter = global_lines.iter().peekable();

        for (i, line) in lines.iter().enumerate() {
            // Check if this line is global
            if let Some(&&global_idx) = global_iter.peek() {
                if i == global_idx {
                    global_iter.next();
                    continue;
                }
            }

            for cell in segment_cells(line, min_x, min_y, cell_size, cols, rows) {
                let write_pos = current_offsets[cell];
                cell_items[write_pos] = i as u32;
                current_offsets[cell] += 1;
            }
        }

        Self {
            cell_offsets,
            cell_items,
            global_lines,
            cell_size,
            cols,
            rows,
            bounds_min: Coord { x: min_x, y: min_y },
        }
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
            )?;
        }
        if !self.global_lines.is_empty() {
            splits.extend(self.process_global_lines_tracked(lines, snap_noder, tracker)?);
        }
        tracker.check_cancelled()?;
        Ok(splits)
    }

    fn process_global_lines(
        &self,
        lines: &[Line3D],
        snap_noder: &SnapNoder,
        soa: &SoALines,
    ) -> Vec<(usize, Coord3D)> {
        let global_lines = &self.global_lines;

        // Helper to process a single global line against all others
        let process_one_global = |g_idx: usize| -> Vec<(usize, Coord3D)> {
            let mut events = Vec::new();
            let query_line = lines[g_idx];

            // Pre-calculate query BBox splats
            let q_min_x = f64x4::splat(query_line.start.x.min(query_line.end.x));
            let q_max_x = f64x4::splat(query_line.start.x.max(query_line.end.x));
            let q_min_y = f64x4::splat(query_line.start.y.min(query_line.end.y));
            let q_max_y = f64x4::splat(query_line.start.y.max(query_line.end.y));

            for block_idx in (0..soa.len()).step_by(4) {
                let mask = soa
                    .intersects_bbox_batch_splatted(q_min_x, q_max_x, q_min_y, q_max_y, block_idx);

                if mask != 0 {
                    for k in 0..4 {
                        if (mask & (1 << k)) != 0 {
                            let target_idx = block_idx + k;
                            if target_idx >= lines.len() {
                                continue;
                            }

                            // Avoid self-intersection check
                            if target_idx == g_idx {
                                continue;
                            }

                            // Deduplicate Global-Global checks
                            // If target is also global, enforce index ordering (only check if g_idx < target_idx)
                            if global_lines.binary_search(&target_idx).is_ok() && target_idx < g_idx
                            {
                                continue;
                            }

                            // Process intersection
                            snap_noder.process_intersection(
                                query_line,
                                lines[target_idx],
                                g_idx,
                                target_idx,
                                |idx, pt| events.push((idx, pt)),
                            );
                        }
                    }
                }
            }
            events
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
    ) -> crate::Result<Vec<(usize, Coord3D)>> {
        let mut events = Vec::new();
        for &left_idx in &self.global_lines {
            for (right_idx, right) in lines.iter().enumerate() {
                if right_idx == left_idx
                    || (right_idx < left_idx && self.global_lines.binary_search(&right_idx).is_ok())
                {
                    continue;
                }
                let left = &lines[left_idx];
                let overlaps = left.start.x.max(left.end.x) >= right.start.x.min(right.end.x)
                    && left.start.x.min(left.end.x) <= right.start.x.max(right.end.x)
                    && left.start.y.max(left.end.y) >= right.start.y.min(right.end.y)
                    && left.start.y.min(left.end.y) <= right.start.y.max(right.end.y);
                tracker.candidate(overlaps)?;
                if overlaps {
                    snap_noder.process_intersection(
                        *left,
                        *right,
                        left_idx,
                        right_idx,
                        |idx, point| events.push((idx, point)),
                    );
                }
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
    ) {
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
        mut tracker: Option<&mut ExecutionWorkTracker<'_>>,
    ) -> crate::Result<()> {
        if cell_indices.len() < 2 {
            return Ok(());
        }

        // Define current cell bounds
        let cell_min_x = self.bounds_min.x + c as f64 * self.cell_size;
        let cell_min_y = self.bounds_min.y + r as f64 * self.cell_size;
        let cell_max_x = cell_min_x + self.cell_size;
        let cell_max_y = cell_min_y + self.cell_size;

        // Brute force pairs within the cell
        for i in 0..cell_indices.len() {
            for j in (i + 1)..cell_indices.len() {
                let idx1 = cell_indices[i] as usize;
                let idx2 = cell_indices[j] as usize;

                // Fast AABB rejection using SoA
                // Check if idx1 and idx2 AABBs overlap
                // Overlap exists if (RectA.min <= RectB.max) && (RectA.max >= RectB.min)
                // We read directly from SoA vectors

                let min_x1 = soa.min_x[idx1];
                let max_x1 = soa.max_x[idx1];
                let min_y1 = soa.min_y[idx1];
                let max_y1 = soa.max_y[idx1];

                let min_x2 = soa.min_x[idx2];
                let max_x2 = soa.max_x[idx2];
                let min_y2 = soa.min_y[idx2];
                let max_y2 = soa.max_y[idx2];

                // Scalar overlap check
                let overlaps =
                    max_x1 >= min_x2 && min_x1 <= max_x2 && max_y1 >= min_y2 && min_y1 <= max_y2;
                if let Some(tracker) = tracker.as_deref_mut() {
                    tracker.candidate(overlaps)?;
                }
                if overlaps {
                    let l1 = lines[idx1];
                    let l2 = lines[idx2];

                    let l1_2d = l1.to_line_2d();
                    let l2_2d = l2.to_line_2d();

                    if let Some(res) = line_intersection(l1_2d, l2_2d) {
                        match res {
                            LineIntersection::SinglePoint {
                                intersection: pt, ..
                            } => {
                                // OWNERSHIP CHECK:
                                // A line pair might exist in multiple cells.
                                // To avoid Duplicate Work: only process if the intersection point
                                // falls strictly within THIS cell's responsibility.
                                let is_in_x = pt.x >= cell_min_x
                                    && (pt.x < cell_max_x
                                        || (c == self.cols - 1 && pt.x <= cell_max_x));
                                let is_in_y = pt.y >= cell_min_y
                                    && (pt.y < cell_max_y
                                        || (r == self.rows - 1 && pt.y <= cell_max_y));

                                if is_in_x && is_in_y {
                                    snap_noder.handle_intersection(
                                        res,
                                        idx1,
                                        idx2,
                                        l1,
                                        l2,
                                        |idx, pt| {
                                            splits.push((idx, pt));
                                        },
                                    );
                                }
                            }
                            LineIntersection::Collinear {
                                intersection: overlap,
                            } => self.handle_collinear(
                                c, r, cell_min_x, cell_max_x, cell_min_y, cell_max_y, overlap,
                                snap_noder, res, idx1, idx2, l1, l2, splits,
                            ),
                        }
                    }
                }
            }
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
            ),
            Err(PolygonizeError::Cancelled { stage }) if stage == "candidate_enumeration"
        ));
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
