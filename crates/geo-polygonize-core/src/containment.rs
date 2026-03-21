use crate::index::{
    IndexedEnvelope, PackedNativeBackend, RStarBackend, SpatialIndex2D, SpatialIndexBackend,
};
use crate::options::IndexBackend;
use crate::options::TouchPolicy;
use crate::polygonizer::{
    bounding_rect_3d, guaranteed_interior_probe, rings_share_edge, rings_touch_at_vertex,
};
use crate::types::Polygon3D;
use crate::utils::simd::SimdRing;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use rstar::AABB;

pub struct ContainmentForest {
    pub tree: SpatialIndexBackend,
    pub simd_shells: Vec<SimdRing>,
    // Cache exterior areas to avoid O(N) recalculations of `exterior_unsigned_area_2d()` inside the tree intersection loops.
    pub shell_areas: Vec<f64>,
}

impl ContainmentForest {
    pub fn new(shells: &[Polygon3D], index_backend: &IndexBackend) -> Self {
        let simd_shells: Vec<SimdRing>;
        let shell_areas: Vec<f64>;
        #[cfg(feature = "parallel")]
        {
            (simd_shells, shell_areas) = shells
                .par_iter()
                .map(|s| (SimdRing::new_3d(&s.exterior), s.exterior_unsigned_area_2d()))
                .unzip();
        }
        #[cfg(not(feature = "parallel"))]
        {
            (simd_shells, shell_areas) = shells
                .iter()
                .map(|s| (SimdRing::new_3d(&s.exterior), s.exterior_unsigned_area_2d()))
                .unzip();
        }

        let mut indexed_shells = Vec::with_capacity(shells.len());
        for (i, shell) in shells.iter().enumerate() {
            if let Some(bbox) = bounding_rect_3d(&shell.exterior) {
                let aabb: AABB<[f64; 2]> =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
                indexed_shells.push(IndexedEnvelope { aabb, index: i });
            }
        }
        let tree = match index_backend {
            IndexBackend::RStar => SpatialIndexBackend::RStar(RStarBackend::new(indexed_shells)),
            IndexBackend::PackedNative => {
                SpatialIndexBackend::PackedNative(PackedNativeBackend::new(&indexed_shells))
            }
        };

        Self {
            tree,
            simd_shells,
            shell_areas,
        }
    }

    pub fn filter_polygonal(&self, shells: &[Polygon3D], touch_policy: &TouchPolicy) -> Vec<bool> {
        let mut keep_mask = vec![true; shells.len()];
        let mut container_counts = vec![0; shells.len()];

        let probe_points: Vec<Option<geo_types::Point<f64>>> = shells
            .iter()
            .map(|s| guaranteed_interior_probe(&s.exterior))
            .collect();

        for (i, shell) in shells.iter().enumerate() {
            let bbox: geo::Rect<f64> = match bounding_rect_3d(&shell.exterior) {
                Some(b) => b,
                None => {
                    keep_mask[i] = false;
                    continue;
                }
            };
            let aabb: AABB<[f64; 2]> =
                AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

            let candidates = self.tree.locate_in_envelope_intersecting(&aabb);
            let probe = probe_points[i];

            if let Some(probe_pt) = probe {
                for cand_idx in candidates {
                    let j = cand_idx;
                    if i == j {
                        continue;
                    }

                    // Check if shell[i] is inside shell[j]
                    let simd_shell = &self.simd_shells[j];

                    if simd_shell.contains(probe_pt.0) {
                        // Using cached areas instead of `shell.exterior_unsigned_area_2d()`
                        let area_i = self.shell_areas[i];
                        let area_j = self.shell_areas[j];

                        // If i is strictly contained inside j, increment container count
                        if area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i) {
                            let touch_ok = match touch_policy {
                                TouchPolicy::AllowPointTouchDisallowEdgeShare => {
                                    !rings_share_edge(&shells[j].exterior, &shell.exterior, 1e-10)
                                }
                                TouchPolicy::TreatAnyTouchAsDisjoint => {
                                    !rings_share_edge(&shells[j].exterior, &shell.exterior, 1e-10)
                                        && !rings_touch_at_vertex(
                                            &shells[j].exterior,
                                            &shell.exterior,
                                            1e-10,
                                        )
                                }
                                TouchPolicy::AllowEdgeShare => true,
                            };

                            if touch_ok {
                                container_counts[i] += 1;
                            }
                        }
                    }
                }
            } else {
                keep_mask[i] = false;
            }
        }

        for i in 0..shells.len() {
            if keep_mask[i] && container_counts[i] % 2 != 0 {
                keep_mask[i] = false;
            }
        }

        keep_mask
    }

    pub fn assign_hole(
        &self,
        hole_3d: &Polygon3D,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
    ) -> Option<usize> {
        let bbox = bounding_rect_3d(&hole_3d.exterior)?;
        let hole_aabb: AABB<[f64; 2]> =
            AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

        let candidates = self.tree.locate_in_envelope_intersecting(&hole_aabb);

        let mut best_shell_idx = None;
        let mut min_area = f64::MAX;

        let probe_point = guaranteed_interior_probe(&hole_3d.exterior)?;
        let hole_area = hole_3d.exterior_unsigned_area_2d();

        for cand_idx in candidates {
            let idx = cand_idx;
            let simd_shell = &self.simd_shells[idx];

            if simd_shell.contains(probe_point.0) {
                // Using cached areas instead of `shells[idx].exterior_unsigned_area_2d()`
                let area = self.shell_areas[idx];

                if area > hole_area + 1e-6 && area < min_area {
                    let touch_ok = match touch_policy {
                        TouchPolicy::AllowPointTouchDisallowEdgeShare => {
                            !rings_share_edge(&shells[idx].exterior, &hole_3d.exterior, 1e-10)
                        }
                        TouchPolicy::TreatAnyTouchAsDisjoint => {
                            !rings_share_edge(&shells[idx].exterior, &hole_3d.exterior, 1e-10)
                                && !rings_touch_at_vertex(
                                    &shells[idx].exterior,
                                    &hole_3d.exterior,
                                    1e-10,
                                )
                        }
                        TouchPolicy::AllowEdgeShare => true,
                    };

                    if touch_ok {
                        min_area = area;
                        best_shell_idx = Some(idx);
                    }
                }
            }
        }

        best_shell_idx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Coord3D;

    fn create_square(x: f64, y: f64, size: f64) -> Polygon3D {
        let coords = vec![
            Coord3D { x, y, z: 0.0 },
            Coord3D {
                x: x + size,
                y,
                z: 0.0,
            },
            Coord3D {
                x: x + size,
                y: y + size,
                z: 0.0,
            },
            Coord3D {
                x,
                y: y + size,
                z: 0.0,
            },
            Coord3D { x, y, z: 0.0 },
        ];
        Polygon3D::new(coords, vec![], vec![], vec![])
    }

    #[test]
    fn test_filter_polygonal_basic() {
        let shells = vec![
            create_square(0.0, 0.0, 10.0), // Outer shell, count 0 -> keep
            create_square(1.0, 1.0, 8.0),  // Inner shell, count 1 -> drop
            create_square(2.0, 2.0, 6.0),  // Inner-inner shell, count 2 -> keep
        ];

        let forest = ContainmentForest::new(&shells, &IndexBackend::RStar);
        let mask = forest.filter_polygonal(&shells, &TouchPolicy::AllowEdgeShare);

        assert_eq!(mask, vec![true, false, true]);
    }

    #[test]
    fn test_filter_polygonal_touch_policy() {
        // Two squares sharing an edge
        let shells = vec![
            create_square(0.0, 0.0, 10.0),
            // "Inner" square but its bottom edge is at y=0, which is the same as the outer shell's bottom edge.
            // Outer square bottom edge: (0,0) to (10,0).
            // Inner square bottom edge: (1,0) to (9,0). This means they share a part of the edge.
            create_square(1.0, 0.0, 8.0),
        ];

        let forest = ContainmentForest::new(&shells, &IndexBackend::RStar);

        // If we allow edge share, the inner square is considered contained and its container count becomes 1 (odd) so it's dropped.
        let mask_allow = forest.filter_polygonal(&shells, &TouchPolicy::AllowEdgeShare);
        assert_eq!(mask_allow, vec![true, false]);

        // If we disallow edge share, the inner square's touch is NOT ok, so it does not get counted as contained.
        // Since it's not counted as contained, container_count remains 0 (even), so it's kept.
        let mask_disallow =
            forest.filter_polygonal(&shells, &TouchPolicy::AllowPointTouchDisallowEdgeShare);
        assert_eq!(mask_disallow, vec![true, true]);
    }

    #[test]
    fn test_assign_hole_basic() {
        let shells = vec![
            create_square(0.0, 0.0, 10.0), // area: 100
            create_square(1.0, 1.0, 8.0), // area: 64 (should be filtered but assign_hole works on raw list)
        ];

        let forest = ContainmentForest::new(&shells, &IndexBackend::RStar);

        // Hole inside the inner shell
        let hole = create_square(2.0, 2.0, 4.0);

        // It should be assigned to the innermost shell that contains it (area 64)
        let best_shell_idx = forest.assign_hole(&hole, &shells, &TouchPolicy::AllowEdgeShare);

        assert_eq!(best_shell_idx, Some(1));
    }
}
