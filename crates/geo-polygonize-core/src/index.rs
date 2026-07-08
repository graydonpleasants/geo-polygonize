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
#[derive(Clone, Debug)]
pub struct IndexedEnvelope {
    pub aabb: AABB<[f64; 2]>,
    pub index: usize,
}

impl PartialEq for IndexedEnvelope {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.aabb.lower() == other.aabb.lower()
            && self.aabb.upper() == other.aabb.upper()
    }
}

impl Eq for IndexedEnvelope {}

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
