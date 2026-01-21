use geo::{Line, Coord};
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};
use std::collections::HashMap;
use crate::noding::snap::SnapNoder;

pub struct UniformGrid {
    /// Flattened grid: cells[row * cols + col] -> List of line indices
    cells: Vec<Vec<usize>>,
    cell_size: f64,
    cols: usize,
    rows: usize,
    bounds_min: Coord<f64>,
}

impl UniformGrid {
    pub fn new(lines: &[Line<f64>]) -> Self {
        if lines.is_empty() {
            return Self::empty();
        }

        // 1. Calculate Bounds & Heuristics
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

        let width = max_x - min_x;
        let height = max_y - min_y;

        // 2. Determine Cell Size
        // Strategy: Target roughly ~4-8 lines per cell for optimal brute-force speed.
        // Formula: sqrt(Area / N) * Tunable_Factor
        let area = width * height;
        // Fallback for zero area (collinear lines)
        let area = if area < 1e-9 { lines.len() as f64 } else { area };

        let target_cell_size = (area / lines.len() as f64).sqrt();
        // Clamp to avoid degenerate grids
        // Ensure cell_size isn't too small relative to the bounds
        let cell_size = target_cell_size.max(width.max(height) / 1000.0).max(1e-6);

        // Adjust for overlapping_circles regression:
        // Too small grid cells might cause ownership check issues with near-boundary intersections?
        // Or perhaps logic is fine, but tuning helps.
        // Let's force cell size slightly larger for now to pass robustness tests if border cases are tricky.
        // let cell_size = cell_size * 2.0;

        // Calculate dimensions
        // Add small epsilon to ensure max boundary falls into a valid cell
        let cols = ((width + 1e-9) / cell_size).ceil() as usize;
        let rows = ((height + 1e-9) / cell_size).ceil() as usize;

        // 3. Initialize & Populate
        // Note: For Wasm, a single flat Vec<Vec> is better than complex trees,
        // but high row/col counts can spike memory. Cap it if necessary.
        let mut grid = Self {
            cells: vec![Vec::new(); cols * rows],
            cell_size,
            cols,
            rows,
            bounds_min: Coord { x: min_x, y: min_y },
        };

        for (i, line) in lines.iter().enumerate() {
            grid.insert(line, i);
        }

        grid
    }

    fn empty() -> Self {
        Self { cells: vec![], cell_size: 1.0, cols: 0, rows: 0, bounds_min: Coord::zero() }
    }

    #[inline]
    fn insert(&mut self, line: &Line<f64>, index: usize) {
        // Find grid range for the line AABB
        let l_min_x = line.start.x.min(line.end.x);
        let l_max_x = line.start.x.max(line.end.x);
        let l_min_y = line.start.y.min(line.end.y);
        let l_max_y = line.start.y.max(line.end.y);

        let col_min = ((l_min_x - self.bounds_min.x) / self.cell_size).floor() as usize;
        let col_max = ((l_max_x - self.bounds_min.x) / self.cell_size).floor() as usize;
        let row_min = ((l_min_y - self.bounds_min.y) / self.cell_size).floor() as usize;
        let row_max = ((l_max_y - self.bounds_min.y) / self.cell_size).floor() as usize;

        // Safety clamp (floating point issues)
        let col_max = col_max.min(self.cols.saturating_sub(1));
        let row_max = row_max.min(self.rows.saturating_sub(1));

        // Ensure min <= max after clamping (in case indices are out of bounds initially)
        let col_min = col_min.min(col_max);
        let row_min = row_min.min(row_max);

        for r in row_min..=row_max {
            for c in col_min..=col_max {
                // Optimization: Avoid pushing if the line barely touches the cell?
                // For noding, conservative (AABB inclusion) is safer and faster.
                self.cells[r * self.cols + c].push(index);
            }
        }
    }

    /// Finds all intersections. Uses "Intersection Ownership" to deduplicate checks.
    pub fn find_splits(&self, lines: &[Line<f64>], snap_noder: &SnapNoder) -> HashMap<usize, Vec<Coord<f64>>> {
        let mut splits: HashMap<usize, Vec<Coord<f64>>> = HashMap::new();

        for r in 0..self.rows {
            for c in 0..self.cols {
                let cell_indices = &self.cells[r * self.cols + c];
                if cell_indices.len() < 2 { continue; }

                // Define current cell bounds
                let cell_min_x = self.bounds_min.x + c as f64 * self.cell_size;
                let cell_min_y = self.bounds_min.y + r as f64 * self.cell_size;
                let cell_max_x = cell_min_x + self.cell_size;
                let cell_max_y = cell_min_y + self.cell_size;

                // Brute force pairs within the cell
                for i in 0..cell_indices.len() {
                    for j in (i + 1)..cell_indices.len() {
                        let idx1 = cell_indices[i];
                        let idx2 = cell_indices[j];

                        // NOTE: If you implemented SoALines, you could insert the SoA check here!
                        // if !soa.intersects(idx1, idx2) { continue; }

                        let l1 = lines[idx1];
                        let l2 = lines[idx2];

                        if let Some(res) = line_intersection(l1, l2) {
                             match res {
                                LineIntersection::SinglePoint { intersection: pt, .. } => {
                                    // OWNERSHIP CHECK:
                                    // A line pair might exist in multiple cells.
                                    // To avoid Duplicate Work: only process if the intersection point
                                    // falls strictly within THIS cell's responsibility.
                                    let is_in_x = pt.x >= cell_min_x && (pt.x < cell_max_x || (c == self.cols - 1 && pt.x <= cell_max_x));
                                    let is_in_y = pt.y >= cell_min_y && (pt.y < cell_max_y || (r == self.rows - 1 && pt.y <= cell_max_y));

                                    if is_in_x && is_in_y {
                                        let snapped = snap_noder.snap(pt);
                                        // Record splits
                                        if snapped != l1.start && snapped != l1.end {
                                            splits.entry(idx1).or_default().push(snapped);
                                        }
                                        if snapped != l2.start && snapped != l2.end {
                                            splits.entry(idx2).or_default().push(snapped);
                                        }
                                    }
                                },
                                LineIntersection::Collinear { intersection: overlap } => {
                                    // Collinear is rare. Just process start/end and let HashMap dedup later.
                                    let p1 = snap_noder.snap(overlap.start);
                                    let p2 = snap_noder.snap(overlap.end);
                                    // Simplified ownership: Check if p1 is in cell
                                    let p1_in = p1.x >= cell_min_x && p1.x < cell_max_x && p1.y >= cell_min_y && p1.y < cell_max_y;
                                    if p1_in || (c==0 && r==0) { // Fallback to process at least once
                                        for p in [p1, p2] {
                                            if p != l1.start && p != l1.end { splits.entry(idx1).or_default().push(p); }
                                            if p != l2.start && p != l2.end { splits.entry(idx2).or_default().push(p); }
                                        }
                                    }
                                }
                             }
                        }
                    }
                }
            }
        }
        splits
    }
}
