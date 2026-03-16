use crate::polygonizer::{bounding_rect_3d, guaranteed_interior_probe, rings_share_edge};
use crate::types::Polygon3D;
use crate::utils::simd::SimdRing;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use rstar::{RTree, RTreeObject, AABB};

// Wrapper for Polygon indexable by rstar (2D)
pub struct IndexedEnvelope {
    pub aabb: AABB<[f64; 2]>,
    pub index: usize,
}

impl RTreeObject for IndexedEnvelope {
    type Envelope = AABB<[f64; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.aabb
    }
}

pub struct ContainmentForest {
    pub tree: RTree<IndexedEnvelope>,
    pub simd_shells: Vec<SimdRing>,
}

impl ContainmentForest {
    pub fn new(shells: &[Polygon3D]) -> Self {
        let simd_shells: Vec<SimdRing>;
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
                let aabb: AABB<[f64; 2]> =
                    AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
                indexed_shells.push(IndexedEnvelope { aabb, index: i });
            }
        }
        let tree = RTree::bulk_load(indexed_shells);

        Self { tree, simd_shells }
    }

    pub fn filter_polygonal(&self, shells: &[Polygon3D]) -> Vec<bool> {
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
                for cand in candidates {
                    let j = cand.index;
                    if i == j {
                        continue;
                    }

                    // Check if shell[i] is inside shell[j]
                    let simd_shell = &self.simd_shells[j];

                    if simd_shell.contains(probe_pt.0) {
                        let area_i = shell.exterior_unsigned_area_2d();
                        let area_j = shells[j].exterior_unsigned_area_2d();

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
            }
        }

        for i in 0..shells.len() {
            if keep_mask[i] && container_counts[i] % 2 != 0 {
                keep_mask[i] = false;
            }
        }

        keep_mask
    }

    pub fn assign_hole(&self, hole_3d: &Polygon3D, shells: &[Polygon3D]) -> Option<usize> {
        let bbox = bounding_rect_3d(&hole_3d.exterior)?;
        let hole_aabb: AABB<[f64; 2]> =
            AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);

        let candidates = self.tree.locate_in_envelope_intersecting(&hole_aabb);

        let mut best_shell_idx = None;
        let mut min_area = f64::MAX;

        let probe_point = guaranteed_interior_probe(&hole_3d.exterior)?;

        for cand in candidates {
            let idx = cand.index;
            let simd_shell = &self.simd_shells[idx];

            if simd_shell.contains(probe_point.0) {
                let area = shells[idx].exterior_unsigned_area_2d();
                let hole_area = hole_3d.exterior_unsigned_area_2d();

                if area > hole_area + 1e-6
                    && area < min_area
                    && !rings_share_edge(&shells[idx].exterior, &hole_3d.exterior, 1e-10)
                {
                    min_area = area;
                    best_shell_idx = Some(idx);
                }
            }
        }

        best_shell_idx
    }
}
