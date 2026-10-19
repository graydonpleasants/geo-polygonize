//! Experimental, bounded original-vertex contact lineage. Not a supported 1.x API.
use crate::{Coord3D, Line3D};
use std::collections::{BTreeSet, HashMap};

type XY = (u64, u64);
fn key(c: Coord3D) -> XY {
    fn bits(v: f64) -> u64 {
        if v == 0.0 {
            0
        } else {
            v.to_bits()
        }
    }
    (bits(c.x), bits(c.y))
}

/// Identity is local to the call, independent of optional/non-unique line IDs.
/// Interior polyline vertices appear once for each adjacent flattened segment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum Endpoint {
    Start,
    End,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct OriginalVertex {
    pub input_segment: usize,
    pub line_id: u32,
    pub endpoint: Endpoint,
    /// Unmodified input XYZ, before any Z policy or coordinate snapping.
    pub raw: Coord3D,
    /// The endpoint selected by the actual pre-snap operation (not inferred proximity).
    pub pre_snap_endpoint: Option<Coord3D>,
    /// Exact grid/restoration mapping of the moved endpoint, separate from contact.
    pub endpoint_output: Option<Coord3D>,
    pub endpoint_node: FinalNode,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub enum FinalNode {
    /// IDs identify nodes in the pre-pruning graph of this call, not across calls.
    Present { node_id: usize, coordinate: Coord3D },
    /// An accepted vertex can disappear when all its supporting segments collapse.
    Missing,
}

/// Exact original-source incidence on pre-pruning graph edges, classified against output.
/// Counts refer to distinct incident undirected edges, not caller line IDs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum FinalSourceIncidence {
    NotTracked,
    /// No descendant edge of this source is incident at the mapped node.
    Absent,
    Present {
        polygon_boundary: usize,
        dangle: usize,
        cut_edge: usize,
        invalid_ring: usize,
        /// An incident edge not represented by an exact edge in classified output.
        /// Never interpret this as polygon-boundary survival.
        other: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum LineageStatus {
    Complete,
    LimitExceeded,
    /// Internal propagation could not identify an exact parent; fail closed.
    IncompletePropagation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub enum AbsentReason {
    /// The source has no noncollapsed descendant in the constructed graph.
    NoGraphDescendants,
    /// The source has graph descendants, but none touches this mapped node.
    SourceNotIncident,
}

/// Exact final result location. Indexes are local to this returned result.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct OutputBoundaryRef {
    pub polygon_index: usize,
    /// None denotes the exterior; Some(i) denotes interiors[i].
    pub interior_ring: Option<usize>,
    /// The edge is ring[edge_index] -> ring[edge_index + 1].
    pub edge_index: usize,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct SourceEdgeAudit {
    pub edge_id: usize,
    pub node_ids: [usize; 2],
    pub source_segments: Vec<usize>,
    pub start: Coord3D,
    pub end: Coord3D,
    pub polygon_boundary: bool,
    pub output_boundaries: Vec<OutputBoundaryRef>,
    pub dangle: bool,
    pub cut_edge: bool,
    pub invalid_ring: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct AcceptedContact {
    /// Index into `original_vertices`; exact original XY equals accepted XY.
    /// Multiple originals at that XY remain distinct; this asserts no Z compatibility.
    pub original_vertex: usize,
    /// Flattened input segment that actually accepted this reference vertex.
    pub accepted_source_segment: usize,
    /// Source-specific accepted XYZ, before grid snapping and Z reconciliation.
    pub accepted: Coord3D,
    /// Snapped topology XY (Z is the source-specific pre-reconciliation value).
    pub topology: Coord3D,
    /// XY restoration used for this accepted vertex; not a distance test.
    pub output: Coord3D,
    pub final_node: FinalNode,
    pub final_source_incidence: FinalSourceIncidence,
    pub absent_reason: Option<AbsentReason>,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct ContactReport {
    pub original_vertices: Vec<OriginalVertex>,
    pub accepted_contacts: Vec<AcceptedContact>,
    /// If true, no absence or completeness claim may be made from this report.
    pub truncated: bool,
    /// Separate source-membership budget; never infer completeness from pre-snap alone.
    pub lineage_status: LineageStatus,
    /// Exact graph construction relationships, retained across component-local pruning.
    pub source_edges: Vec<SourceEdgeAudit>,
}
impl ContactReport {
    /// Certifies only exact source membership at mapped nodes. `other` remains
    /// unknown output classification and MUST NOT be interpreted as boundary absence.
    pub fn complete_source_incidence(&self) -> Option<&[AcceptedContact]> {
        (!self.truncated && self.lineage_status == LineageStatus::Complete)
            .then_some(&self.accepted_contacts)
    }

    /// Fail closed for output classification as well as source incidence.
    /// A filtered or otherwise unrepresented graph edge makes classification unknown.
    pub fn complete_output_classification(&self) -> Option<&[AcceptedContact]> {
        self.complete_source_incidence().filter(|_| {
            self.source_edges.iter().all(|edge| {
                edge.polygon_boundary || edge.dangle || edge.cut_edge || edge.invalid_ring
            })
        })
    }

    /// Fail closed for the exact raw-XY pre-snap acceptance relation only.
    /// Completeness here NEVER proves final source incidence or absence of final contact.
    /// Endpoint relocation is a separate relation in `original_vertices`.
    pub fn complete_presnap_acceptances(&self) -> Option<&[AcceptedContact]> {
        (!self.truncated).then_some(&self.accepted_contacts)
    }
}

pub(crate) struct ContactCapture {
    pub(crate) report: ContactReport,
    limit: usize,
    originals_by_xy: HashMap<XY, Vec<usize>>,
    pub(crate) lineage: SourceLineage,
    edge_memberships: usize,
}
impl ContactCapture {
    pub(crate) fn new(lines: &[Line3D], limit: usize) -> Self {
        let mut capture = Self {
            report: ContactReport {
                original_vertices: Vec::new(),
                accepted_contacts: Vec::new(),
                truncated: false,
                lineage_status: LineageStatus::Complete,
                source_edges: Vec::new(),
            },
            limit,
            originals_by_xy: HashMap::new(),
            lineage: SourceLineage::new(limit),
            edge_memberships: 0,
        };
        for (input_segment, line) in lines.iter().enumerate() {
            for (endpoint, raw) in [(Endpoint::Start, line.start), (Endpoint::End, line.end)] {
                if !capture.room() {
                    return capture;
                }
                let index = capture.report.original_vertices.len();
                capture
                    .originals_by_xy
                    .entry(key(raw))
                    .or_default()
                    .push(index);
                capture.report.original_vertices.push(OriginalVertex {
                    input_segment,
                    line_id: line.line_id,
                    endpoint,
                    raw,
                    pre_snap_endpoint: None,
                    endpoint_output: None,
                    endpoint_node: FinalNode::Missing,
                });
            }
        }
        capture
    }
    fn room(&mut self) -> bool {
        if self.report.original_vertices.len() + self.report.accepted_contacts.len() >= self.limit {
            self.report.truncated = true;
            false
        } else {
            true
        }
    }
    pub(crate) fn endpoints(&mut self, segment: usize, start: Coord3D, end: Coord3D) {
        // Original rows are ordered as two endpoints per input segment.
        for (offset, point) in [start, end].into_iter().enumerate() {
            if let Some(vertex) = self.report.original_vertices.get_mut(segment * 2 + offset) {
                vertex.pre_snap_endpoint = Some(point);
            }
        }
    }
    pub(crate) fn accept(&mut self, source: usize, point: Coord3D) {
        let Some(indices) = self.originals_by_xy.get(&key(point)) else {
            return;
        };
        // Bound fan-out without allocating a copy of the original-vertex list.
        for i in 0..indices.len() {
            if !self.room() {
                return;
            }
            let original_vertex = self.originals_by_xy[&key(point)][i];
            self.report.accepted_contacts.push(AcceptedContact {
                original_vertex,
                accepted_source_segment: source,
                accepted: point,
                topology: point,
                output: point,
                final_node: FinalNode::Missing,
                final_source_incidence: FinalSourceIncidence::NotTracked,
                absent_reason: None,
            });
        }
    }
    pub(crate) fn map_topology(
        &mut self,
        noder: &crate::noding::snap::SnapNoder,
        restoration: Option<&HashMap<XY, Coord3D>>,
    ) {
        for vertex in &mut self.report.original_vertices {
            vertex.endpoint_output = vertex.pre_snap_endpoint.map(|point| {
                let mut output = noder.snap(point);
                if let Some(restored) = restoration.and_then(|map| map.get(&key(output))) {
                    output.x = restored.x;
                    output.y = restored.y;
                }
                output
            });
        }
        for contact in &mut self.report.accepted_contacts {
            contact.topology = noder.snap(contact.accepted);
            contact.output = contact.topology;
            if let Some(restored) = restoration.and_then(|map| map.get(&key(contact.topology))) {
                contact.output.x = restored.x;
                contact.output.y = restored.y;
            }
        }
    }
    pub(crate) fn resolve(&mut self, xs: &[f64], ys: &[f64], zs: &[f64]) {
        // Only index tracked vertices: auxiliary memory is bounded by the report limit.
        let mut wanted: HashMap<XY, Option<(usize, Coord3D)>> = self
            .report
            .accepted_contacts
            .iter()
            .map(|c| (key(c.output), None))
            .collect();
        for vertex in &self.report.original_vertices {
            if let Some(output) = vertex.endpoint_output {
                wanted.entry(key(output)).or_default();
            }
        }
        for (node_id, ((&x, &y), &z)) in xs.iter().zip(ys).zip(zs).enumerate() {
            let coordinate = Coord3D::new(x, y, z);
            if let Some(slot) = wanted.get_mut(&key(coordinate)) {
                *slot = Some((node_id, coordinate));
            }
        }
        for vertex in &mut self.report.original_vertices {
            vertex.endpoint_node = match vertex.endpoint_output.and_then(|c| wanted[&key(c)]) {
                Some((node_id, coordinate)) => FinalNode::Present {
                    node_id,
                    coordinate,
                },
                None => FinalNode::Missing,
            };
        }
        for contact in &mut self.report.accepted_contacts {
            contact.final_node = match wanted[&key(contact.output)] {
                Some((node_id, coordinate)) => FinalNode::Present {
                    node_id,
                    coordinate,
                },
                None => FinalNode::Missing,
            };
        }
    }
}

// A key denotes an actual segment in a processing generation. It is deliberately
// independent of Z (which the production noder deduplicates) and retains caller ID
// solely to mirror that noder's merge partition, never as original identity.
type SegmentKey = (XY, XY, u32);
fn segment_key(line: &Line3D) -> SegmentKey {
    let a = key(line.start);
    let b = key(line.end);
    (a.min(b), a.max(b), line.line_id)
}

pub(crate) struct SourceLineage {
    current: HashMap<SegmentKey, BTreeSet<usize>>,
    next: HashMap<SegmentKey, BTreeSet<usize>>,
    limit: usize,
    next_memberships: usize,
    current_memberships: usize,
    complete: bool,
    failure: LineageStatus,
}
impl SourceLineage {
    fn new(limit: usize) -> Self {
        Self {
            current: HashMap::new(),
            next: HashMap::new(),
            limit,
            next_memberships: 0,
            current_memberships: 0,
            complete: true,
            failure: LineageStatus::Complete,
        }
    }
    fn invalidate(&mut self) {
        self.complete = false;
        self.failure = LineageStatus::LimitExceeded;
        self.current.clear();
        self.next.clear();
    }
    fn seed(&mut self, source: usize, child: &Line3D) {
        if !self.complete {
            return;
        }
        let k = segment_key(child);
        if self
            .current
            .get(&k)
            .is_some_and(|set| set.contains(&source))
        {
            return;
        }
        if self.current_memberships >= self.limit {
            self.invalidate();
            return;
        }
        self.current.entry(k).or_default().insert(source);
        self.current_memberships += 1;
    }
    pub(crate) fn begin_split(&mut self) {
        self.next.clear();
        self.next_memberships = 0;
    }
    pub(crate) fn inherit(&mut self, parent: &Line3D, child: &Line3D) {
        if !self.complete {
            return;
        }
        let Some(sources) = self.current.get(&segment_key(parent)) else {
            self.invalidate();
            self.failure = LineageStatus::IncompletePropagation;
            return;
        };
        let child_key = segment_key(child);
        // No unbounded temporary copy: split generations each hold <= limit memberships.
        for &source in sources {
            if self
                .next
                .get(&child_key)
                .is_some_and(|set| set.contains(&source))
            {
                continue;
            }
            if self.next_memberships >= self.limit {
                self.invalidate();
                return;
            }
            self.next.entry(child_key).or_default().insert(source);
            self.next_memberships += 1;
        }
    }
    pub(crate) fn finish_split(&mut self) {
        std::mem::swap(&mut self.current, &mut self.next);
        self.current_memberships = self.next_memberships;
        self.next.clear();
    }
}

impl ContactCapture {
    pub(crate) fn child(&mut self, source: usize, child: Line3D) {
        self.lineage.seed(source, &child);
    }
    pub(crate) fn graph_edge(&mut self, edge_id: usize, u: usize, v: usize, line: &Line3D) {
        if !self.lineage.complete {
            return;
        }
        let Some(sources) = self.lineage.current.get(&segment_key(line)) else {
            self.lineage.invalidate();
            self.lineage.failure = LineageStatus::IncompletePropagation;
            return;
        };
        let edges = &mut self.report.source_edges;
        if edges.last().is_none_or(|edge| edge.edge_id != edge_id) {
            // Every edge requires at least one membership, so this also bounds edge records.
            if self.edge_memberships >= self.limit {
                self.lineage.invalidate();
                return;
            }
            edges.push(SourceEdgeAudit {
                edge_id,
                node_ids: [u, v],
                source_segments: Vec::new(),
                start: line.start,
                end: line.end,
                polygon_boundary: false,
                output_boundaries: Vec::new(),
                dangle: false,
                cut_edge: false,
                invalid_ring: false,
            });
        }
        let edge = edges.last_mut().expect("captured edge");
        for &source in sources {
            match edge.source_segments.binary_search(&source) {
                Ok(_) => (),
                Err(at) => {
                    if self.edge_memberships >= self.limit {
                        self.lineage.invalidate();
                        return;
                    }
                    edge.source_segments.insert(at, source);
                    self.edge_memberships += 1;
                }
            }
        }
    }
    pub(crate) fn classify(&mut self, result: &crate::polygonizer::PolygonizerResult) {
        if !self.lineage.complete {
            self.report.lineage_status = self.lineage.failure;
            self.report.source_edges.clear();
            return;
        }
        // Match output edges only to edges whose source membership was already
        // captured at actual graph creation/merge. No coordinate source inference.
        let mut by_xy = HashMap::new();
        for (i, edge) in self.report.source_edges.iter().enumerate() {
            let a = key(edge.start);
            let b = key(edge.end);
            by_xy.insert((a.min(b), a.max(b)), i);
        }
        let mut boundary_refs = 0usize;
        let mut boundary_limit_exceeded = false;
        let mut mark = |points: &[Coord3D], category: u8, owner: Option<(usize, Option<usize>)>| {
            for (edge_index, pair) in points.windows(2).enumerate() {
                let a = key(pair[0]);
                let b = key(pair[1]);
                if let Some(&i) = by_xy.get(&(a.min(b), a.max(b))) {
                    let edge = &mut self.report.source_edges[i];
                    match category {
                        0 => {
                            if boundary_refs >= self.limit {
                                boundary_limit_exceeded = true;
                                continue;
                            }
                            let (polygon_index, interior_ring) = owner.expect("boundary owner");
                            edge.output_boundaries.push(OutputBoundaryRef {
                                polygon_index,
                                interior_ring,
                                edge_index,
                            });
                            boundary_refs += 1;
                            edge.polygon_boundary = true;
                        }
                        1 => edge.dangle = true,
                        2 => edge.cut_edge = true,
                        _ => edge.invalid_ring = true,
                    }
                }
            }
        };
        for (polygon_index, polygon) in result.polygons.iter().enumerate() {
            mark(&polygon.exterior, 0, Some((polygon_index, None)));
            for (interior_ring, hole) in polygon.interiors.iter().enumerate() {
                mark(hole, 0, Some((polygon_index, Some(interior_ring))));
            }
        }
        for points in &result.dangles {
            mark(points, 1, None);
        }
        for points in &result.cut_edges {
            mark(points, 2, None);
        }
        for points in &result.invalid_rings {
            mark(points, 3, None);
        }
        if boundary_limit_exceeded {
            self.lineage.invalidate();
            self.report.lineage_status = LineageStatus::LimitExceeded;
            self.report.source_edges.clear();
            return;
        }
        for contact in &mut self.report.accepted_contacts {
            let mut counts = [0; 5];
            let mut any_descendant = false;
            for edge in &self.report.source_edges {
                if edge
                    .source_segments
                    .binary_search(&contact.accepted_source_segment)
                    .is_err()
                {
                    continue;
                }
                any_descendant = true;
                if !matches!(contact.final_node, FinalNode::Present { node_id, .. } if edge.node_ids.contains(&node_id))
                {
                    continue;
                }
                counts[0] += usize::from(edge.polygon_boundary);
                counts[1] += usize::from(edge.dangle);
                counts[2] += usize::from(edge.cut_edge);
                counts[3] += usize::from(edge.invalid_ring);
                counts[4] += usize::from(
                    !(edge.polygon_boundary || edge.dangle || edge.cut_edge || edge.invalid_ring),
                );
            }
            contact.final_source_incidence = if counts.iter().all(|&n| n == 0) {
                contact.absent_reason = Some(if any_descendant {
                    AbsentReason::SourceNotIncident
                } else {
                    AbsentReason::NoGraphDescendants
                });
                FinalSourceIncidence::Absent
            } else {
                FinalSourceIncidence::Present {
                    polygon_boundary: counts[0],
                    dangle: counts[1],
                    cut_edge: counts[2],
                    invalid_ring: counts[3],
                    other: counts[4],
                }
            };
        }
    }
}

#[cfg(test)]
mod source_lineage_tests {
    use super::*;

    fn line(start: f64, end: f64, id: u32) -> Line3D {
        Line3D::new(
            Coord3D::new(start, 0.0, 0.0),
            Coord3D::new(end, 0.0, 0.0),
            id,
        )
    }

    #[test]
    fn split_fanout_exhaustion_discards_all_lineage() {
        let parent = line(0.0, 3.0, 7);
        let mut lineage = SourceLineage::new(2);
        lineage.seed(0, &parent);
        lineage.begin_split();
        lineage.inherit(&parent, &line(0.0, 1.0, 7));
        lineage.inherit(&parent, &line(1.0, 2.0, 7));
        assert!(lineage.complete);
        assert_eq!(lineage.next_memberships, 2);
        lineage.inherit(&parent, &line(2.0, 3.0, 7));
        assert!(!lineage.complete);
        assert_eq!(lineage.failure, LineageStatus::LimitExceeded);
        assert!(lineage.current.is_empty());
        assert!(lineage.next.is_empty());
        lineage.finish_split();
        lineage.seed(99, &parent);
        assert!(!lineage.complete);
        assert!(lineage.current.is_empty());
    }

    #[test]
    fn duplicate_geometry_unions_exact_sources_not_caller_ids() {
        let first = line(0.0, 2.0, 7);
        let mut reversed_other_z = line(2.0, 0.0, 7);
        reversed_other_z.start.z = 10.0;
        reversed_other_z.end.z = 20.0;
        let unrelated_same_id = line(4.0, 5.0, 7);
        let mut lineage = SourceLineage::new(6);
        lineage.seed(0, &first);
        lineage.seed(1, &reversed_other_z);
        lineage.seed(1, &reversed_other_z);
        lineage.seed(2, &unrelated_same_id);
        assert_eq!(lineage.current_memberships, 3);
        assert_eq!(lineage.current.len(), 2);
        assert_eq!(
            lineage.current[&segment_key(&first)],
            BTreeSet::from([0, 1])
        );

        let left = line(0.0, 1.0, 7);
        let right = line(1.0, 2.0, 7);
        lineage.begin_split();
        lineage.inherit(&first, &left);
        lineage.inherit(&reversed_other_z, &left);
        lineage.inherit(&first, &right);
        lineage.inherit(&unrelated_same_id, &unrelated_same_id);
        lineage.finish_split();
        assert!(lineage.complete);
        assert_eq!(lineage.current_memberships, 5);
        assert_eq!(lineage.current[&segment_key(&left)], BTreeSet::from([0, 1]));
        assert_eq!(
            lineage.current[&segment_key(&right)],
            BTreeSet::from([0, 1])
        );
        assert_eq!(
            lineage.current[&segment_key(&unrelated_same_id)],
            BTreeSet::from([2])
        );
    }

    #[test]
    fn missing_parent_fails_closed_even_with_matching_caller_id() {
        let known = line(0.0, 2.0, 7);
        let unknown = line(2.0, 4.0, 7);
        let mut lineage = SourceLineage::new(8);
        lineage.seed(0, &known);
        lineage.begin_split();
        lineage.inherit(&known, &line(0.0, 1.0, 7));
        lineage.inherit(&unknown, &line(2.0, 3.0, 7));
        assert!(!lineage.complete);
        assert_eq!(lineage.failure, LineageStatus::IncompletePropagation);
        assert!(lineage.current.is_empty());
        assert!(lineage.next.is_empty());
        lineage.finish_split();
        assert!(lineage.current.is_empty());
    }
}
