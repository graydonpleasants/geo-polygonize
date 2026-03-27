use rstar::AABB;

pub trait SpatialIndex2D {
    /// Returns the indices of the elements whose bounding boxes intersect the given `aabb`.
    fn locate_in_envelope_intersecting<'a>(
        &'a self,
        aabb: &AABB<[f64; 2]>,
    ) -> Box<dyn Iterator<Item = usize> + 'a>;
}

use rstar::{RTree, RTreeObject};

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

pub struct RStarBackend {
    tree: RTree<IndexedEnvelope>,
}

impl RStarBackend {
    pub fn new(envelopes: Vec<IndexedEnvelope>) -> Self {
        Self {
            tree: RTree::bulk_load(envelopes),
        }
    }
}

impl SpatialIndex2D for RStarBackend {
    fn locate_in_envelope_intersecting<'a>(
        &'a self,
        aabb: &AABB<[f64; 2]>,
    ) -> Box<dyn Iterator<Item = usize> + 'a> {
        Box::new(
            self.tree
                .locate_in_envelope_intersecting(aabb)
                .map(|cand| cand.index),
        )
    }
}

use static_aabb2d_index::StaticAABB2DIndex;
use static_aabb2d_index::StaticAABB2DIndexBuilder;

pub struct PackedNativeBackend {
    index: StaticAABB2DIndex<f64>,
}

impl PackedNativeBackend {
    pub fn new(envelopes: &[IndexedEnvelope]) -> Self {
        let mut builder = StaticAABB2DIndexBuilder::new(envelopes.len());
        for env in envelopes {
            let min = env.aabb.lower();
            let max = env.aabb.upper();
            builder.add(min[0], min[1], max[0], max[1]);
        }
        Self {
            index: builder.build().unwrap(),
        }
    }
}

impl SpatialIndex2D for PackedNativeBackend {
    fn locate_in_envelope_intersecting<'a>(
        &'a self,
        aabb: &AABB<[f64; 2]>,
    ) -> Box<dyn Iterator<Item = usize> + 'a> {
        let min = aabb.lower();
        let max = aabb.upper();
        let query_results = self.index.query(min[0], min[1], max[0], max[1]);
        Box::new(query_results.into_iter())
    }
}

pub enum SpatialIndexBackend {
    RStar(RStarBackend),
    PackedNative(PackedNativeBackend),
    GpuCompute(PackedNativeBackend), // Uses PackedNative as a fallback/accelerator
}

impl SpatialIndex2D for SpatialIndexBackend {
    fn locate_in_envelope_intersecting<'a>(
        &'a self,
        aabb: &AABB<[f64; 2]>,
    ) -> Box<dyn Iterator<Item = usize> + 'a> {
        match self {
            SpatialIndexBackend::RStar(backend) => backend.locate_in_envelope_intersecting(aabb),
            SpatialIndexBackend::PackedNative(backend) => {
                backend.locate_in_envelope_intersecting(aabb)
            }
            SpatialIndexBackend::GpuCompute(backend) => {
                backend.locate_in_envelope_intersecting(aabb)
            }
        }
    }
}
