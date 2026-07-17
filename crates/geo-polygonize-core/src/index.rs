use rstar::{Envelope, SelectionFunction, AABB};

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

    /// Returns indices whose envelopes fully contain `aabb` without boxing the iterator.
    pub fn locate_containing_envelope(
        &self,
        aabb: &AABB<[f64; 2]>,
    ) -> impl Iterator<Item = usize> + '_ {
        self.tree
            .locate_with_selection_function(SelectContainingEnvelope { aabb: *aabb })
            .map(|candidate| candidate.index)
    }
}

struct SelectContainingEnvelope {
    aabb: AABB<[f64; 2]>,
}

impl SelectionFunction<IndexedEnvelope> for SelectContainingEnvelope {
    fn should_unpack_parent(&self, parent: &AABB<[f64; 2]>) -> bool {
        parent.contains_envelope(&self.aabb)
    }

    fn should_unpack_leaf(&self, leaf: &IndexedEnvelope) -> bool {
        leaf.aabb.contains_envelope(&self.aabb)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containing_query_prunes_intersecting_non_containers() {
        let tree = RStarBackend::new(vec![
            IndexedEnvelope {
                aabb: AABB::from_corners([0.0, 0.0], [10.0, 10.0]),
                index: 0,
            },
            IndexedEnvelope {
                aabb: AABB::from_corners([2.0, 2.0], [3.0, 3.0]),
                index: 1,
            },
            IndexedEnvelope {
                aabb: AABB::from_corners([2.5, 2.5], [4.0, 4.0]),
                index: 2,
            },
        ]);
        let query = AABB::from_corners([2.0, 2.0], [3.0, 3.0]);
        assert_eq!(tree.locate_in_envelope_intersecting(&query).count(), 3);

        let mut actual: Vec<_> = tree.locate_containing_envelope(&query).collect();
        actual.sort_unstable();

        assert_eq!(actual, vec![0, 1]);
    }
}
