use crate::diagnostics::ContainmentStats;
use crate::index::{IndexedEnvelope, RStarBackend};
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
    pub tree: RStarBackend,
    pub simd_shells: Vec<Option<SimdRing>>,
    // Cache exterior areas to avoid O(N) recalculations of `exterior_unsigned_area_2d()` inside the tree intersection loops.
    pub shell_areas: Vec<Option<f64>>,
}

impl ContainmentForest {
    pub fn new(shells: &[Polygon3D]) -> Self {
        let mut simd_shells: Vec<Option<SimdRing>> = Vec::with_capacity(shells.len());
        let mut shell_areas: Vec<Option<f64>> = Vec::with_capacity(shells.len());
        #[cfg(feature = "parallel")]
        {
            let (shells_p, areas_p): (Vec<_>, Vec<_>) = shells
                .par_iter()
                .map(|s| {
                    (
                        Some(SimdRing::new_3d(&s.exterior)),
                        Some(s.exterior_unsigned_area_2d()),
                    )
                })
                .unzip();
            simd_shells.extend(shells_p);
            shell_areas.extend(areas_p);
        }
        #[cfg(not(feature = "parallel"))]
        {
            let (shells_p, areas_p): (Vec<_>, Vec<_>) = shells
                .iter()
                .map(|s| {
                    (
                        Some(SimdRing::new_3d(&s.exterior)),
                        Some(s.exterior_unsigned_area_2d()),
                    )
                })
                .unzip();
            simd_shells.extend(shells_p);
            shell_areas.extend(areas_p);
        }

        let mut indexed_shells = Vec::with_capacity(shells.len());
        for (i, shell) in shells.iter().enumerate() {
            if let Some(bbox) = bounding_rect_3d(&shell.exterior) {
                let aabb: AABB<[f64; 2]> =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
                indexed_shells.push(IndexedEnvelope { aabb, index: i });
            }
        }
        let tree = RStarBackend::new(indexed_shells);

        Self {
            tree,
            simd_shells,
            shell_areas,
        }
    }

    pub fn filter_polygonal(&self, shells: &[Polygon3D], touch_policy: &TouchPolicy) -> Vec<bool> {
        self.filter_polygonal_impl(shells, touch_policy, None)
    }

    pub(crate) fn filter_polygonal_with_stats(
        &self,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
    ) -> (Vec<bool>, ContainmentStats) {
        let mut stats = ContainmentStats::default();
        let keep_mask = self.filter_polygonal_impl(shells, touch_policy, Some(&mut stats));
        (keep_mask, stats)
    }

    fn filter_polygonal_impl(
        &self,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
        mut stats: Option<&mut ContainmentStats>,
    ) -> Vec<bool> {
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

            let candidates = self.tree.locate_containing_envelope(&aabb);
            let probe = probe_points[i];
            let Some(area_i) = self.shell_areas[i] else {
                keep_mask[i] = false;
                continue;
            };

            if let Some(probe_pt) = probe {
                for j in candidates {
                    if let Some(stats) = stats.as_deref_mut() {
                        stats.envelope_candidates += 1;
                    }
                    if i == j {
                        continue;
                    }

                    let Some(area_j) = self.shell_areas[j] else {
                        continue;
                    };
                    if !(area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i)) {
                        if let Some(stats) = stats.as_deref_mut() {
                            stats.area_rejections += 1;
                        }
                        continue;
                    }

                    if let Some(simd_shell) = &self.simd_shells[j] {
                        if let Some(stats) = stats.as_deref_mut() {
                            stats.point_in_ring_calls += 1;
                        }
                        if simd_shell.contains(probe_pt.0) {
                            let touch_ok = match touch_policy {
                                TouchPolicy::AllowPointTouchDisallowEdgeShare => {
                                    if let Some(stats) = stats.as_deref_mut() {
                                        stats.shared_edge_checks += 1;
                                    }
                                    !rings_share_edge(&shells[j].exterior, &shell.exterior, 1e-10)
                                }
                                TouchPolicy::TreatAnyTouchAsDisjoint => {
                                    if let Some(stats) = stats.as_deref_mut() {
                                        stats.shared_edge_checks += 1;
                                    }
                                    let shares_edge = rings_share_edge(
                                        &shells[j].exterior,
                                        &shell.exterior,
                                        1e-10,
                                    );
                                    if let Some(stats) = stats.as_deref_mut() {
                                        stats.shared_vertex_checks += usize::from(!shares_edge);
                                    }
                                    !shares_edge
                                        && !rings_touch_at_vertex(
                                            &shells[j].exterior,
                                            &shell.exterior,
                                            1e-10,
                                        )
                                }
                                TouchPolicy::AllowEdgeShare => true,
                            };

                            container_counts[i] += usize::from(touch_ok);
                        }
                    }
                }
            } else {
                keep_mask[i] = false;
            }
        }

        for (keep, count) in keep_mask.iter_mut().zip(container_counts.iter()) {
            if *keep && count % 2 != 0 {
                *keep = false;
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
        self.assign_hole_impl(hole_3d, shells, touch_policy, None)
    }

    pub(crate) fn assign_hole_with_stats(
        &self,
        hole_3d: &Polygon3D,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
    ) -> (Option<usize>, ContainmentStats) {
        let mut stats = ContainmentStats::default();
        let best_shell_idx = self.assign_hole_impl(hole_3d, shells, touch_policy, Some(&mut stats));
        (best_shell_idx, stats)
    }

    fn assign_hole_impl(
        &self,
        hole_3d: &Polygon3D,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
        mut stats: Option<&mut ContainmentStats>,
    ) -> Option<usize> {
        let bbox = bounding_rect_3d(&hole_3d.exterior)?;
        let hole_aabb: AABB<[f64; 2]> =
            AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

        let candidates = self.tree.locate_containing_envelope(&hole_aabb);

        let mut best_shell_idx = None;
        let mut min_area = f64::MAX;

        let probe_point = guaranteed_interior_probe(&hole_3d.exterior)?;
        let hole_area = hole_3d.exterior_unsigned_area_2d();

        for idx in candidates {
            if let Some(stats) = stats.as_deref_mut() {
                stats.envelope_candidates += 1;
            }
            let Some(area) = self.shell_areas[idx] else {
                continue;
            };
            if area <= hole_area + 1e-6 || area >= min_area {
                if let Some(stats) = stats.as_deref_mut() {
                    stats.area_rejections += 1;
                }
                continue;
            }

            if let Some(simd_shell) = &self.simd_shells[idx] {
                if let Some(stats) = stats.as_deref_mut() {
                    stats.point_in_ring_calls += 1;
                }
                if simd_shell.contains(probe_point.0) {
                    let touch_ok = match touch_policy {
                        TouchPolicy::AllowPointTouchDisallowEdgeShare => {
                            if let Some(stats) = stats.as_deref_mut() {
                                stats.shared_edge_checks += 1;
                            }
                            !rings_share_edge(&shells[idx].exterior, &hole_3d.exterior, 1e-10)
                        }
                        TouchPolicy::TreatAnyTouchAsDisjoint => {
                            if let Some(stats) = stats.as_deref_mut() {
                                stats.shared_edge_checks += 1;
                            }
                            let shares_edge =
                                rings_share_edge(&shells[idx].exterior, &hole_3d.exterior, 1e-10);
                            if let Some(stats) = stats.as_deref_mut() {
                                stats.shared_vertex_checks += usize::from(!shares_edge);
                            }
                            !shares_edge
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
