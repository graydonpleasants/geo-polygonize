use crate::diagnostics::ContainmentStats;
use crate::index::{IndexedEnvelope, RStarBackend};
use crate::options::TouchPolicy;
use crate::polygonizer::{
    bounding_rect_3d, guaranteed_interior_probe_prepared, rings_share_edge_with_count,
    rings_touch_at_vertex_with_count,
};
use crate::types::{Polygon3D, RingGraphIdentity};
use crate::utils::simd::SimdRing;
use geo::algorithm::indexed::IntervalTreeMultiPolygon;
use geo::{Contains, MultiPolygon};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use rstar::AABB;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

const LOCATOR_INDEX_QUERY_THRESHOLD: usize = 64;
const LOCATOR_INDEX_MIN_COORDS: usize = 65;

fn touch_allowed(
    shell: &Polygon3D,
    shell_identity: Option<&RingGraphIdentity>,
    ring: &Polygon3D,
    ring_identity: Option<&RingGraphIdentity>,
    touch_policy: &TouchPolicy,
    mut stats: Option<&mut ContainmentStats>,
) -> bool {
    match touch_policy {
        TouchPolicy::AllowPointTouchDisallowEdgeShare | TouchPolicy::TreatAnyTouchAsDisjoint => {
            let (shares_edge, pair_checks, used_graph_ids) = if let (
                Some(shell_identity),
                Some(ring_identity),
            ) = (shell_identity, ring_identity)
            {
                let (shares_edge, pair_checks) =
                    sorted_intersects(&shell_identity.edge_keys, &ring_identity.edge_keys);
                (shares_edge, pair_checks, true)
            } else {
                let (shares_edge, pair_checks) =
                    rings_share_edge_with_count(&shell.exterior, &ring.exterior, 1e-10);
                (shares_edge, pair_checks, false)
            };
            if let Some(stats) = stats.as_deref_mut() {
                stats.shared_edge_checks += 1;
                if used_graph_ids {
                    stats.graph_edge_key_checks += pair_checks;
                } else {
                    stats.shared_edge_pair_checks += pair_checks;
                }
            }
            if shares_edge || matches!(touch_policy, TouchPolicy::AllowPointTouchDisallowEdgeShare)
            {
                return !shares_edge;
            }

            let (touches_vertex, pair_checks, used_graph_ids) = if let (
                Some(shell_identity),
                Some(ring_identity),
            ) =
                (shell_identity, ring_identity)
            {
                let (touches_vertex, pair_checks) =
                    sorted_intersects(&shell_identity.node_ids, &ring_identity.node_ids);
                (touches_vertex, pair_checks, true)
            } else {
                let (touches_vertex, pair_checks) =
                    rings_touch_at_vertex_with_count(&shell.exterior, &ring.exterior, 1e-10);
                (touches_vertex, pair_checks, false)
            };
            if let Some(stats) = stats {
                stats.shared_vertex_checks += 1;
                if used_graph_ids {
                    stats.graph_vertex_id_checks += pair_checks;
                } else {
                    stats.shared_vertex_pair_checks += pair_checks;
                }
            }
            !touches_vertex
        }
        TouchPolicy::AllowEdgeShare => true,
    }
}

fn sorted_intersects<T: Ord>(left: &[T], right: &[T]) -> (bool, usize) {
    let (mut i, mut j, mut checks) = (0, 0, 0);
    while i < left.len() && j < right.len() {
        checks += 1;
        match left[i].cmp(&right[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => return (true, checks),
        }
    }
    (false, checks)
}

struct PreparedRing {
    signed_area: f64,
    aabb: Option<AABB<[f64; 2]>>,
    diagonal: f64,
    probe: OnceLock<Option<geo_types::Point<f64>>>,
    locator_queries: AtomicUsize,
    interval_locator: OnceLock<IntervalTreeMultiPolygon<f64>>,
    graph_identity: Option<RingGraphIdentity>,
}

impl PreparedRing {
    fn new(polygon: &Polygon3D, graph_identity: Option<RingGraphIdentity>) -> Self {
        let signed_area = Polygon3D::ring_signed_area_2d(&polygon.exterior);
        let bbox = bounding_rect_3d(&polygon.exterior);
        let diagonal = bbox
            .as_ref()
            .map(|bbox| {
                let dx = bbox.max().x - bbox.min().x;
                let dy = bbox.max().y - bbox.min().y;
                (dx * dx + dy * dy).sqrt()
            })
            .unwrap_or(1.0);
        let aabb = bbox.map(|bbox| {
            AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y])
        });

        Self {
            signed_area,
            aabb,
            diagonal,
            probe: OnceLock::new(),
            locator_queries: AtomicUsize::new(0),
            interval_locator: OnceLock::new(),
            graph_identity,
        }
    }

    fn probe(&self, polygon: &Polygon3D, locator: &SimdRing) -> Option<geo_types::Point<f64>> {
        *self.probe.get_or_init(|| {
            guaranteed_interior_probe_prepared(
                &polygon.exterior,
                self.signed_area,
                self.diagonal,
                locator,
            )
        })
    }

    fn contains(
        &self,
        polygon: &Polygon3D,
        locator: &SimdRing,
        point: geo_types::Coord<f64>,
        track_queries: bool,
    ) -> bool {
        let queries = if track_queries || polygon.exterior.len() >= LOCATOR_INDEX_MIN_COORDS {
            self.locator_queries.fetch_add(1, Ordering::Relaxed) + 1
        } else {
            0
        };
        if queries >= LOCATOR_INDEX_QUERY_THRESHOLD
            && polygon.exterior.len() >= LOCATOR_INDEX_MIN_COORDS
        {
            let interval = self.interval_locator.get_or_init(|| {
                IntervalTreeMultiPolygon::new(&MultiPolygon(vec![polygon.to_polygon_2d()]))
            });
            if interval.contains(&point) {
                return true;
            }
        }
        locator.contains(point)
    }
}

pub struct ContainmentForest {
    pub(crate) tree: RStarBackend,
    pub(crate) simd_shells: Vec<Option<SimdRing>>,
    prepared_shells: Vec<PreparedRing>,
}

impl ContainmentForest {
    pub fn new(shells: &[Polygon3D]) -> Self {
        Self::new_impl(shells, None)
    }

    pub(crate) fn new_with_graph_ids(
        shells: &[Polygon3D],
        graph_ids: &[Option<RingGraphIdentity>],
    ) -> Self {
        Self::new_impl(shells, Some(graph_ids))
    }

    fn new_impl(shells: &[Polygon3D], graph_ids: Option<&[Option<RingGraphIdentity>]>) -> Self {
        debug_assert!(graph_ids.is_none_or(|ids| ids.len() == shells.len()));
        #[cfg(feature = "parallel")]
        let prepared_and_locators: Vec<_> = shells
            .par_iter()
            .enumerate()
            .map(|(i, shell)| {
                let locator = SimdRing::new_3d(&shell.exterior);
                (
                    PreparedRing::new(shell, graph_ids.and_then(|ids| ids[i].clone())),
                    locator,
                )
            })
            .collect();
        #[cfg(not(feature = "parallel"))]
        let prepared_and_locators: Vec<_> = shells
            .iter()
            .enumerate()
            .map(|(i, shell)| {
                let locator = SimdRing::new_3d(&shell.exterior);
                (
                    PreparedRing::new(shell, graph_ids.and_then(|ids| ids[i].clone())),
                    locator,
                )
            })
            .collect();

        let mut prepared_shells = Vec::with_capacity(shells.len());
        let mut simd_shells = Vec::with_capacity(shells.len());
        for (prepared, locator) in prepared_and_locators {
            prepared_shells.push(prepared);
            simd_shells.push(Some(locator));
        }

        let mut indexed_shells = Vec::with_capacity(shells.len());
        for (i, prepared) in prepared_shells.iter().enumerate() {
            if let Some(aabb) = prepared.aabb {
                indexed_shells.push(IndexedEnvelope { aabb, index: i });
            }
        }
        let tree = RStarBackend::new(indexed_shells);

        Self {
            tree,
            simd_shells,
            prepared_shells,
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

    pub(crate) fn record_locator_reuse(&self, stats: &mut ContainmentStats) {
        let mut max_queries = 0;
        let mut shells_at_threshold = 0;
        for prepared in &self.prepared_shells {
            let count = prepared.locator_queries.load(Ordering::Relaxed);
            max_queries = max_queries.max(count);
            shells_at_threshold += usize::from(count >= LOCATOR_INDEX_QUERY_THRESHOLD);
        }
        stats.max_point_in_ring_calls_per_shell =
            stats.max_point_in_ring_calls_per_shell.max(max_queries);
        stats.shells_with_64_plus_point_in_ring_calls += shells_at_threshold;
    }

    fn filter_polygonal_impl(
        &self,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
        mut stats: Option<&mut ContainmentStats>,
    ) -> Vec<bool> {
        let track_queries = stats.is_some();
        let mut keep_mask = vec![true; shells.len()];
        let mut container_counts = vec![0; shells.len()];

        for (i, shell) in shells.iter().enumerate() {
            let prepared = &self.prepared_shells[i];
            let aabb = match prepared.aabb.as_ref() {
                Some(aabb) => aabb,
                None => {
                    keep_mask[i] = false;
                    continue;
                }
            };

            let candidates = self.tree.locate_containing_envelope(aabb);
            let probe = prepared.probe(shell, self.simd_shells[i].as_ref().unwrap());
            let area_i = prepared.signed_area.abs();

            if let Some(probe_pt) = probe {
                for j in candidates {
                    if let Some(stats) = stats.as_deref_mut() {
                        stats.envelope_candidates += 1;
                    }
                    if i == j {
                        continue;
                    }

                    let area_j = self.prepared_shells[j].signed_area.abs();
                    if !(area_j > area_i || (area_j == area_i && j < i)) {
                        if let Some(stats) = stats.as_deref_mut() {
                            stats.area_rejections += 1;
                        }
                        continue;
                    }

                    if let Some(simd_shell) = &self.simd_shells[j] {
                        if let Some(stats) = stats.as_deref_mut() {
                            stats.point_in_ring_calls += 1;
                        }
                        if self.prepared_shells[j].contains(
                            &shells[j],
                            simd_shell,
                            probe_pt.0,
                            track_queries,
                        ) {
                            let touch_ok = touch_allowed(
                                &shells[j],
                                self.prepared_shells[j].graph_identity.as_ref(),
                                shell,
                                prepared.graph_identity.as_ref(),
                                touch_policy,
                                stats.as_deref_mut(),
                            );

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
        self.assign_hole_impl(hole_3d, None, shells, touch_policy, None)
    }

    pub(crate) fn assign_hole_with_graph_ids(
        &self,
        hole_3d: &Polygon3D,
        hole_graph_ids: Option<&RingGraphIdentity>,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
    ) -> Option<usize> {
        self.assign_hole_impl(hole_3d, hole_graph_ids, shells, touch_policy, None)
    }

    pub(crate) fn assign_hole_with_graph_ids_and_stats(
        &self,
        hole_3d: &Polygon3D,
        hole_graph_ids: Option<&RingGraphIdentity>,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
    ) -> (Option<usize>, ContainmentStats) {
        let mut stats = ContainmentStats::default();
        let best_shell_idx = self.assign_hole_impl(
            hole_3d,
            hole_graph_ids,
            shells,
            touch_policy,
            Some(&mut stats),
        );
        (best_shell_idx, stats)
    }

    fn assign_hole_impl(
        &self,
        hole_3d: &Polygon3D,
        hole_graph_ids: Option<&RingGraphIdentity>,
        shells: &[Polygon3D],
        touch_policy: &TouchPolicy,
        mut stats: Option<&mut ContainmentStats>,
    ) -> Option<usize> {
        let track_queries = stats.is_some();
        let hole_locator = SimdRing::new_3d(&hole_3d.exterior);
        let prepared_hole = PreparedRing::new(hole_3d, hole_graph_ids.cloned());
        let hole_aabb = prepared_hole.aabb.as_ref()?;

        let candidates = self.tree.locate_containing_envelope(hole_aabb);

        let mut best_shell_idx = None;
        let mut min_area = f64::MAX;

        let probe_point = prepared_hole.probe(hole_3d, &hole_locator)?;
        let hole_area = prepared_hole.signed_area.abs();

        for idx in candidates {
            if let Some(stats) = stats.as_deref_mut() {
                stats.envelope_candidates += 1;
            }
            let area = self.prepared_shells[idx].signed_area.abs();
            if area <= hole_area || area >= min_area {
                if let Some(stats) = stats.as_deref_mut() {
                    stats.area_rejections += 1;
                }
                continue;
            }

            if let Some(simd_shell) = &self.simd_shells[idx] {
                if let Some(stats) = stats.as_deref_mut() {
                    stats.point_in_ring_calls += 1;
                }
                if self.prepared_shells[idx].contains(
                    &shells[idx],
                    simd_shell,
                    probe_point.0,
                    track_queries,
                ) {
                    let touch_ok = touch_allowed(
                        &shells[idx],
                        self.prepared_shells[idx].graph_identity.as_ref(),
                        hole_3d,
                        prepared_hole.graph_identity.as_ref(),
                        touch_policy,
                        stats.as_deref_mut(),
                    );

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
    use geo::algorithm::indexed::IntervalTreeMultiPolygon;
    use geo::{Contains, Coord, MultiPolygon};

    #[test]
    fn prepared_ring_retains_containment_metadata() {
        let polygon = Polygon3D::new(
            vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(10.0, 10.0, 0.0),
                Coord3D::new(0.0, 10.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
            ],
            vec![],
            vec![],
            vec![],
        );
        let locator = SimdRing::new_3d(&polygon.exterior);
        let prepared = PreparedRing::new(&polygon, None);

        assert_eq!(prepared.signed_area, 100.0);
        assert_eq!(prepared.aabb.unwrap().lower(), [0.0, 0.0]);
        let probe = prepared.probe(&polygon, &locator).unwrap().0;
        assert!(prepared.contains(&polygon, &locator, probe, false));
        assert_eq!(prepared.locator_queries.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn graph_identity_distinguishes_point_and_edge_touch() {
        let polygon = Polygon3D::new(vec![], vec![], vec![], vec![]);
        let shell_ids = RingGraphIdentity::new(vec![(0, 1)], vec![0, 1]);
        let ring_ids = RingGraphIdentity::new(vec![(1, 2)], vec![1, 2]);
        let mut stats = ContainmentStats::default();

        assert!(touch_allowed(
            &polygon,
            Some(&shell_ids),
            &polygon,
            Some(&ring_ids),
            &TouchPolicy::AllowPointTouchDisallowEdgeShare,
            Some(&mut stats),
        ));
        assert!(!touch_allowed(
            &polygon,
            Some(&shell_ids),
            &polygon,
            Some(&ring_ids),
            &TouchPolicy::TreatAnyTouchAsDisjoint,
            Some(&mut stats),
        ));
        assert_eq!(stats.shared_edge_pair_checks, 0);
        assert_eq!(stats.shared_vertex_pair_checks, 0);
        assert!(stats.graph_edge_key_checks > 0);
        assert!(stats.graph_vertex_id_checks > 0);
    }

    #[test]
    fn interval_tree_boundary_semantics_do_not_match_simd() {
        let polygon = Polygon3D::new(
            vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(10.0, 10.0, 0.0),
                Coord3D::new(0.0, 10.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
            ],
            vec![],
            vec![],
            vec![],
        );
        let point = Coord { x: 5.0, y: 0.0 };
        let simd = SimdRing::new_3d(&polygon.exterior);
        let interval = IntervalTreeMultiPolygon::new(&MultiPolygon(vec![polygon.to_polygon_2d()]));

        assert!(simd.contains(point));
        assert!(!interval.contains(&point));
    }

    #[test]
    fn adaptive_locator_preserves_simd_boundary_semantics() {
        let mut exterior = Vec::with_capacity(65);
        for i in 0..16 {
            let offset = 20.0 * i as f64 / 16.0;
            exterior.push(Coord3D::new(-10.0 + offset, -10.0, 0.0));
        }
        for i in 0..16 {
            let offset = 20.0 * i as f64 / 16.0;
            exterior.push(Coord3D::new(10.0, -10.0 + offset, 0.0));
        }
        for i in 0..16 {
            let offset = 20.0 * i as f64 / 16.0;
            exterior.push(Coord3D::new(10.0 - offset, 10.0, 0.0));
        }
        for i in 0..16 {
            let offset = 20.0 * i as f64 / 16.0;
            exterior.push(Coord3D::new(-10.0, 10.0 - offset, 0.0));
        }
        exterior.push(exterior[0]);
        let polygon = Polygon3D::new(exterior, vec![], vec![], vec![]);
        let simd = SimdRing::new_3d(&polygon.exterior);
        let prepared = PreparedRing::new(&polygon, None);
        let boundary = Coord { x: 0.0, y: -10.0 };

        assert!(simd.contains(boundary));
        for _ in 0..LOCATOR_INDEX_QUERY_THRESHOLD {
            assert_eq!(
                prepared.contains(&polygon, &simd, boundary, false),
                simd.contains(boundary)
            );
        }
        assert!(prepared.interval_locator.get().is_some());
    }
}
