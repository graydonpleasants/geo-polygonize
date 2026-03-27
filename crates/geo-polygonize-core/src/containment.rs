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
use crate::gpu::{GpuContainmentContext, GpuCoord, GpuRing, GpuPoint};

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
            IndexBackend::GpuCompute => {
                SpatialIndexBackend::GpuCompute(PackedNativeBackend::new(&indexed_shells))
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

        let is_gpu = matches!(self.tree, SpatialIndexBackend::GpuCompute(_));
        let gpu_ctx = if is_gpu { GpuContainmentContext::new() } else { None };

        if let Some(ref gpu) = gpu_ctx {
            // Bulk GPU mode
            // Find candidate pairs
            let mut candidate_pairs = Vec::new(); // (probe_idx, container_idx, probe_pt)
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
                if let Some(probe_pt) = probe_points[i] {
                    for j in candidates {
                        if i == j {
                            continue;
                        }
                        candidate_pairs.push((i, j, probe_pt));
                    }
                } else {
                    keep_mask[i] = false;
                }
            }

            // Build GPU buffers
            let mut gpu_coords = Vec::new();
            let mut gpu_rings = Vec::new();
            let mut gpu_points = Vec::new();

            for &(_, j, probe_pt) in &candidate_pairs {
                let shell = &shells[j];
                let start_idx = gpu_coords.len() as u32;
                for c in &shell.exterior {
                    gpu_coords.push(GpuCoord { x: c.x as f32, y: c.y as f32 });
                }
                let length = (gpu_coords.len() as u32) - start_idx;

                gpu_rings.push(GpuRing { start_idx, length });
                gpu_points.push(GpuPoint { x: probe_pt.x() as f32, y: probe_pt.y() as f32 });
            }

            let results = gpu.check_containment(&gpu_coords, &gpu_rings, &gpu_points);

            for (idx, is_inside) in results.into_iter().enumerate() {
                if is_inside {
                    let (i, j, _) = candidate_pairs[idx];
                    let area_i = self.shell_areas[i];
                    let area_j = self.shell_areas[j];

                    if area_j > area_i || ((area_j - area_i).abs() < 1e-9 && j < i) {
                        let touch_ok = match touch_policy {
                            TouchPolicy::AllowPointTouchDisallowEdgeShare => {
                                !rings_share_edge(&shells[j].exterior, &shells[i].exterior, 1e-10)
                            }
                            TouchPolicy::TreatAnyTouchAsDisjoint => {
                                !rings_share_edge(&shells[j].exterior, &shells[i].exterior, 1e-10)
                                    && !rings_touch_at_vertex(
                                        &shells[j].exterior,
                                        &shells[i].exterior,
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
            // Standard CPU mode
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
        let bbox = bounding_rect_3d(&hole_3d.exterior)?;
        let hole_aabb: AABB<[f64; 2]> =
            AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

        let candidates: Vec<usize> = self.tree.locate_in_envelope_intersecting(&hole_aabb).collect();

        let mut best_shell_idx = None;
        let mut min_area = f64::MAX;

        let probe_point = guaranteed_interior_probe(&hole_3d.exterior)?;
        let hole_area = hole_3d.exterior_unsigned_area_2d();

        let is_gpu = matches!(self.tree, SpatialIndexBackend::GpuCompute(_));
        let gpu_ctx = if is_gpu { GpuContainmentContext::new() } else { None };

        if let Some(ref gpu) = gpu_ctx {
            let mut gpu_coords = Vec::new();
            let mut gpu_rings = Vec::new();
            let mut gpu_points = Vec::new();

            for &idx in &candidates {
                let shell = &shells[idx];
                let start_idx = gpu_coords.len() as u32;
                for c in &shell.exterior {
                    gpu_coords.push(GpuCoord { x: c.x as f32, y: c.y as f32 });
                }
                let length = (gpu_coords.len() as u32) - start_idx;

                gpu_rings.push(GpuRing { start_idx, length });
                gpu_points.push(GpuPoint { x: probe_point.0.x as f32, y: probe_point.0.y as f32 });
            }

            let results = gpu.check_containment(&gpu_coords, &gpu_rings, &gpu_points);

            for (i, is_inside) in results.into_iter().enumerate() {
                if is_inside {
                    let idx = candidates[i];
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
        } else {
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
        }

        best_shell_idx
    }
}
