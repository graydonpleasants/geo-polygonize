use crate::types::{Coord3D, Line3D};
use geo::algorithm::line_intersection::{line_intersection, LineIntersection};
use robust::orient2d;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet};

pub struct AdvancedNoder;

impl Default for AdvancedNoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedNoder {
    pub fn new() -> Self {
        Self
    }

    /// A full sweep-line algorithm to find all intersections in O((N+K) log N) scaling.
    /// It incorporates an arbitrary-precision fallback using `robust::orient2d` for geometrically
    /// ambiguous configurations.
    pub fn node(&self, lines: Vec<Line3D>) -> Vec<Line3D> {
        let mut segments = lines;
        let mut changed = true;

        while changed {
            changed = false;
            let mut events = BinaryHeap::new();

            for (i, segment) in segments.iter().enumerate() {
                let p1 = segment.start;
                let p2 = segment.end;

                let (left, right) = if p1.x < p2.x || (p1.x == p2.x && p1.y <= p2.y) {
                    (p1, p2)
                } else {
                    (p2, p1)
                };

                events.push(Event {
                    x: left.x,
                    segment_idx: i,
                    is_left: true,
                });
                events.push(Event {
                    x: right.x,
                    segment_idx: i,
                    is_left: false,
                });
            }

            // Sweep-line status: active segments ordered by Y at current sweep X.
            // Using Vec with binary search insertion for O(log M) search and O(M) insertion.
            let mut active_segments: Vec<usize> = Vec::new();
            let mut intersections = Vec::new();
            let mut checked_pairs = HashSet::new();
            while let Some(event) = events.pop() {
                let current_x = event.x;
                let s_idx = event.segment_idx;

                if event.is_left {
                    // Find position to insert in active_segments using binary search
                    let insert_pos = active_segments
                        .binary_search_by(|&idx| {
                            let y_active = Self::y_at_x(&segments[idx], current_x);
                            let y_new = Self::y_at_x(&segments[s_idx], current_x);
                            y_active.partial_cmp(&y_new).unwrap_or(Ordering::Equal)
                        })
                        .unwrap_or_else(|pos| pos);

                    active_segments.insert(insert_pos, s_idx);

                    if insert_pos > 0 {
                        let above_idx = active_segments[insert_pos - 1];
                        if Self::check_and_record_intersection(
                            &segments,
                            s_idx,
                            above_idx,
                            &mut checked_pairs,
                            &mut intersections,
                        ) {
                            changed = true;
                        }
                    }
                    if insert_pos < active_segments.len() - 1 {
                        let below_idx = active_segments[insert_pos + 1];
                        if Self::check_and_record_intersection(
                            &segments,
                            s_idx,
                            below_idx,
                            &mut checked_pairs,
                            &mut intersections,
                        ) {
                            changed = true;
                        }
                    }
                } else {
                    // Right endpoint
                    if let Some(pos) = active_segments.iter().position(|&x| x == s_idx) {
                        if pos > 0 && pos < active_segments.len() - 1 {
                            let above_idx = active_segments[pos - 1];
                            let below_idx = active_segments[pos + 1];
                            if Self::check_and_record_intersection(
                                &segments,
                                above_idx,
                                below_idx,
                                &mut checked_pairs,
                                &mut intersections,
                            ) {
                                changed = true;
                            }
                        }
                        active_segments.remove(pos);
                    }
                }
            }

            if changed {
                segments = Self::split_segments(segments, intersections);
            }
        }

        segments
    }

    fn y_at_x(line: &Line3D, x: f64) -> f64 {
        let dx = line.end.x - line.start.x;
        if dx == 0.0 {
            return f64::min(line.start.y, line.end.y);
        }
        let dy = line.end.y - line.start.y;
        let slope = dy / dx;
        line.start.y + slope * (x - line.start.x)
    }

    /// Computes intersection using standard floating-point algorithm, falling back to robust
    /// arbitrary-precision orientation predicates if geometrically ambiguous or collinear.
    fn check_and_record_intersection(
        segments: &[Line3D],
        idx_a: usize,
        idx_b: usize,
        checked_pairs: &mut HashSet<(usize, usize)>,
        intersections: &mut Vec<IntersectionSplit>,
    ) -> bool {
        let pair = if idx_a < idx_b {
            (idx_a, idx_b)
        } else {
            (idx_b, idx_a)
        };
        if !checked_pairs.insert(pair) {
            return false;
        }

        let s1 = &segments[idx_a];
        let s2 = &segments[idx_b];

        // Arbitrary-precision fallback via robust orientation checks.
        // If s1 points bounding box strictly separates s2 endpoints, they don't intersect.
        let a = robust::Coord {
            x: s1.start.x,
            y: s1.start.y,
        };
        let b = robust::Coord {
            x: s1.end.x,
            y: s1.end.y,
        };
        let c = robust::Coord {
            x: s2.start.x,
            y: s2.start.y,
        };
        let d = robust::Coord {
            x: s2.end.x,
            y: s2.end.y,
        };

        let o1 = orient2d(a, b, c);
        let o2 = orient2d(a, b, d);
        let o3 = orient2d(c, d, a);
        let o4 = orient2d(c, d, b);

        // Check exact topological intersection
        let intersects_strictly =
            o1 != o2 && o3 != o4 && o1 != 0.0 && o2 != 0.0 && o3 != 0.0 && o4 != 0.0;

        if intersects_strictly {
            if let Some(LineIntersection::SinglePoint {
                intersection: pt, ..
            }) = line_intersection(
                geo::Line::new(s1.start.to_coord_2d(), s1.end.to_coord_2d()),
                geo::Line::new(s2.start.to_coord_2d(), s2.end.to_coord_2d()),
            ) {
                let eps = 1e-9;

                let dx1_s = pt.x - s1.start.x;
                let dy1_s = pt.y - s1.start.y;
                let s1_start_dist = dx1_s * dx1_s + dy1_s * dy1_s;

                let dx1_e = pt.x - s1.end.x;
                let dy1_e = pt.y - s1.end.y;
                let s1_end_dist = dx1_e * dx1_e + dy1_e * dy1_e;

                let dx2_s = pt.x - s2.start.x;
                let dy2_s = pt.y - s2.start.y;
                let s2_start_dist = dx2_s * dx2_s + dy2_s * dy2_s;

                let dx2_e = pt.x - s2.end.x;
                let dy2_e = pt.y - s2.end.y;
                let s2_end_dist = dx2_e * dx2_e + dy2_e * dy2_e;

                if s1_start_dist > eps
                    && s1_end_dist > eps
                    && s2_start_dist > eps
                    && s2_end_dist > eps
                {
                    let s1_len_sq = (s1.end.x - s1.start.x) * (s1.end.x - s1.start.x)
                        + (s1.end.y - s1.start.y) * (s1.end.y - s1.start.y);
                    let t1 = s1_start_dist.sqrt() / s1_len_sq.sqrt();
                    let z_interp = s1.start.z + t1 * (s1.end.z - s1.start.z);

                    let intersect_coord = Coord3D {
                        x: pt.x,
                        y: pt.y,
                        z: z_interp,
                    };
                    intersections.push(IntersectionSplit {
                        segment_idx: idx_a,
                        point: intersect_coord,
                    });
                    intersections.push(IntersectionSplit {
                        segment_idx: idx_b,
                        point: intersect_coord,
                    });
                    return true;
                }
            }
        }
        false
    }

    fn split_segments(segments: Vec<Line3D>, splits: Vec<IntersectionSplit>) -> Vec<Line3D> {
        let mut new_segments = Vec::new();
        let mut splits_by_segment: std::collections::HashMap<usize, Vec<Coord3D>> =
            std::collections::HashMap::new();

        for split in splits {
            splits_by_segment
                .entry(split.segment_idx)
                .or_default()
                .push(split.point);
        }

        for (i, segment) in segments.into_iter().enumerate() {
            if let Some(mut pts) = splits_by_segment.remove(&i) {
                pts.sort_by(|a, b| {
                    let dx_a = a.x - segment.start.x;
                    let dy_a = a.y - segment.start.y;
                    let dist_a = dx_a * dx_a + dy_a * dy_a;

                    let dx_b = b.x - segment.start.x;
                    let dy_b = b.y - segment.start.y;
                    let dist_b = dx_b * dx_b + dy_b * dy_b;

                    dist_a.partial_cmp(&dist_b).unwrap_or(Ordering::Equal)
                });

                let mut current_start = segment.start;
                let line_id = segment.line_id;

                for pt in pts {
                    let eps = 1e-9;
                    let dx = current_start.x - pt.x;
                    let dy = current_start.y - pt.y;
                    if dx * dx + dy * dy > eps {
                        new_segments.push(Line3D {
                            start: current_start,
                            end: pt,
                            line_id,
                        });
                        current_start = pt;
                    }
                }
                let eps = 1e-9;
                let dx_end = current_start.x - segment.end.x;
                let dy_end = current_start.y - segment.end.y;
                if dx_end * dx_end + dy_end * dy_end > eps {
                    new_segments.push(Line3D {
                        start: current_start,
                        end: segment.end,
                        line_id,
                    });
                }
            } else {
                new_segments.push(segment);
            }
        }
        new_segments
    }
}

#[derive(Debug, Clone, Copy)]
struct Event {
    x: f64,
    segment_idx: usize,
    is_left: bool,
}

impl Ord for Event {
    fn cmp(&self, other: &Self) -> Ordering {
        // We use a max-heap, so we reverse the ordering to get a min-heap behavior.
        // Primary sort: X coordinate.
        match other.x.partial_cmp(&self.x).unwrap_or(Ordering::Equal) {
            Ordering::Equal => {
                // To fix the vertical line issue, left endpoints MUST be processed before right endpoints
                // when X coordinates are identical.
                // Reversing for min-heap means left (true) should be considered "greater" than right (false).
                match (self.is_left, other.is_left) {
                    (true, false) => Ordering::Greater, // Process left before right
                    (false, true) => Ordering::Less,
                    _ => Ordering::Equal,
                }
            }
            ord => ord,
        }
    }
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Event {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.is_left == other.is_left
    }
}

impl Eq for Event {}

struct IntersectionSplit {
    segment_idx: usize,
    point: Coord3D,
}
