//! Experimental, bounded original-vertex contact lineage. Not a supported 1.x API.
use crate::{Coord3D, ExecutionPolicy, Line3D, Result};
use std::collections::{BTreeSet, HashMap, HashSet};

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
    pub(crate) fn new(
        lines: &[Line3D],
        limit: usize,
        execution_policy: &ExecutionPolicy,
    ) -> Result<Self> {
        execution_policy.check_cancelled("contact_capture")?;
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
                execution_policy.check_cancelled_every(
                    "contact_capture",
                    capture.report.original_vertices.len(),
                )?;
                if !capture.room() {
                    execution_policy.check_cancelled("contact_capture")?;
                    return Ok(capture);
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
        execution_policy.check_cancelled("contact_capture")?;
        Ok(capture)
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
    pub(crate) fn accept(
        &mut self,
        source: usize,
        point: Coord3D,
        execution_policy: &ExecutionPolicy,
    ) -> Result<()> {
        execution_policy.check_cancelled("contact_capture")?;
        let Some(indices) = self.originals_by_xy.get(&key(point)) else {
            return Ok(());
        };
        // Bound fan-out without allocating a copy of the original-vertex list.
        for i in 0..indices.len() {
            execution_policy.check_cancelled_every("contact_capture", i)?;
            if !self.room() {
                execution_policy.check_cancelled("contact_capture")?;
                return Ok(());
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
        execution_policy.check_cancelled("contact_capture")
    }
    pub(crate) fn map_topology(
        &mut self,
        noder: &crate::noding::snap::SnapNoder,
        restoration: Option<&HashMap<XY, Coord3D>>,
        execution_policy: &ExecutionPolicy,
    ) -> Result<()> {
        execution_policy.check_cancelled("contact_capture")?;
        for (i, vertex) in self.report.original_vertices.iter_mut().enumerate() {
            execution_policy.check_cancelled_every("contact_capture", i)?;
            vertex.endpoint_output = vertex.pre_snap_endpoint.map(|point| {
                let mut output = noder.snap(point);
                if let Some(restored) = restoration.and_then(|map| map.get(&key(output))) {
                    output.x = restored.x;
                    output.y = restored.y;
                }
                output
            });
        }
        for (i, contact) in self.report.accepted_contacts.iter_mut().enumerate() {
            execution_policy.check_cancelled_every("contact_capture", i)?;
            contact.topology = noder.snap(contact.accepted);
            contact.output = contact.topology;
            if let Some(restored) = restoration.and_then(|map| map.get(&key(contact.topology))) {
                contact.output.x = restored.x;
                contact.output.y = restored.y;
            }
        }
        execution_policy.check_cancelled("contact_capture")
    }
    pub(crate) fn resolve(
        &mut self,
        xs: &[f64],
        ys: &[f64],
        zs: &[f64],
        execution_policy: &ExecutionPolicy,
    ) -> Result<()> {
        execution_policy.check_cancelled("contact_capture")?;
        // Only index tracked vertices: auxiliary memory is bounded by the report limit.
        let mut wanted: HashMap<XY, Option<(usize, Coord3D)>> = HashMap::new();
        for (i, contact) in self.report.accepted_contacts.iter().enumerate() {
            execution_policy.check_cancelled_every("contact_capture", i)?;
            wanted.insert(key(contact.output), None);
        }
        for (i, vertex) in self.report.original_vertices.iter().enumerate() {
            execution_policy.check_cancelled_every("contact_capture", i)?;
            if let Some(output) = vertex.endpoint_output {
                wanted.entry(key(output)).or_default();
            }
        }
        for (node_id, ((&x, &y), &z)) in xs.iter().zip(ys).zip(zs).enumerate() {
            execution_policy.check_cancelled_every("contact_capture", node_id)?;
            let coordinate = Coord3D::new(x, y, z);
            if let Some(slot) = wanted.get_mut(&key(coordinate)) {
                *slot = Some((node_id, coordinate));
            }
        }
        for (i, vertex) in self.report.original_vertices.iter_mut().enumerate() {
            execution_policy.check_cancelled_every("contact_capture", i)?;
            vertex.endpoint_node = match vertex.endpoint_output.and_then(|c| wanted[&key(c)]) {
                Some((node_id, coordinate)) => FinalNode::Present {
                    node_id,
                    coordinate,
                },
                None => FinalNode::Missing,
            };
        }
        for (i, contact) in self.report.accepted_contacts.iter_mut().enumerate() {
            execution_policy.check_cancelled_every("contact_capture", i)?;
            contact.final_node = match wanted[&key(contact.output)] {
                Some((node_id, coordinate)) => FinalNode::Present {
                    node_id,
                    coordinate,
                },
                None => FinalNode::Missing,
            };
        }
        execution_policy.check_cancelled("contact_capture")
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
    pub(crate) fn graph_edge(
        &mut self,
        edge_id: usize,
        u: usize,
        v: usize,
        line: &Line3D,
        execution_policy: &ExecutionPolicy,
    ) -> Result<()> {
        execution_policy.check_cancelled("contact_capture")?;
        if !self.lineage.complete {
            return Ok(());
        }
        let Some(sources) = self.lineage.current.get(&segment_key(line)) else {
            self.lineage.invalidate();
            self.lineage.failure = LineageStatus::IncompletePropagation;
            return Ok(());
        };
        let edges = &mut self.report.source_edges;
        if edges.last().is_none_or(|edge| edge.edge_id != edge_id) {
            // Every edge requires at least one membership, so this also bounds edge records.
            if self.edge_memberships >= self.limit {
                self.lineage.invalidate();
                return Ok(());
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
        for (i, &source) in sources.iter().enumerate() {
            execution_policy.check_cancelled_every("contact_capture", i)?;
            match edge.source_segments.binary_search(&source) {
                Ok(_) => (),
                Err(at) => {
                    if self.edge_memberships >= self.limit {
                        self.lineage.invalidate();
                        return Ok(());
                    }
                    edge.source_segments.insert(at, source);
                    self.edge_memberships += 1;
                }
            }
        }
        execution_policy.check_cancelled("contact_capture")
    }
    pub(crate) fn classify(
        &mut self,
        result: &crate::polygonizer::PolygonizerResult,
        execution_policy: &ExecutionPolicy,
    ) -> Result<()> {
        execution_policy.check_cancelled("contact_classification")?;
        if !self.lineage.complete {
            self.report.lineage_status = self.lineage.failure;
            self.report.source_edges.clear();
            return Ok(());
        }
        // Match output edges only to edges whose source membership was already
        // captured at actual graph creation/merge. No coordinate source inference.
        let mut by_xy = HashMap::new();
        for (i, edge) in self.report.source_edges.iter().enumerate() {
            execution_policy.check_cancelled_every("contact_classification", i)?;
            let a = key(edge.start);
            let b = key(edge.end);
            by_xy.insert((a.min(b), a.max(b)), i);
        }
        let mut boundary_refs = 0usize;
        let mut boundary_limit_exceeded = false;
        let mut output_work = 0usize;
        let mut mark = |points: &[Coord3D],
                        category: u8,
                        owner: Option<(usize, Option<usize>)>|
         -> Result<()> {
            execution_policy.check_cancelled_every("contact_classification", output_work)?;
            output_work = output_work.saturating_add(1);
            for (edge_index, pair) in points.windows(2).enumerate() {
                execution_policy.check_cancelled_every("contact_classification", output_work)?;
                output_work = output_work.saturating_add(1);
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
            Ok(())
        };
        for (polygon_index, polygon) in result.polygons.iter().enumerate() {
            mark(&polygon.exterior, 0, Some((polygon_index, None)))?;
            for (interior_ring, hole) in polygon.interiors.iter().enumerate() {
                mark(hole, 0, Some((polygon_index, Some(interior_ring))))?;
            }
        }
        for points in &result.dangles {
            mark(points, 1, None)?;
        }
        for points in &result.cut_edges {
            mark(points, 2, None)?;
        }
        for points in &result.invalid_rings {
            mark(points, 3, None)?;
        }
        execution_policy.check_cancelled("contact_classification")?;
        if boundary_limit_exceeded {
            self.lineage.invalidate();
            self.report.lineage_status = LineageStatus::LimitExceeded;
            self.report.source_edges.clear();
            return Ok(());
        }
        // Aggregate once per already-capped graph membership, rather than scanning
        // every audited edge for every accepted contact. At most 2 * limit keys
        // and limit descendants are retained; neither index scales with raw input.
        let mut incident_counts: HashMap<(usize, usize), [usize; 5]> = HashMap::new();
        let mut descendants = HashSet::new();
        let mut membership_work = 0usize;
        for edge in &self.report.source_edges {
            let edge_counts = [
                usize::from(edge.polygon_boundary),
                usize::from(edge.dangle),
                usize::from(edge.cut_edge),
                usize::from(edge.invalid_ring),
                usize::from(
                    !(edge.polygon_boundary || edge.dangle || edge.cut_edge || edge.invalid_ring),
                ),
            ];
            for &source in &edge.source_segments {
                execution_policy
                    .check_cancelled_every("contact_classification", membership_work)?;
                membership_work = membership_work.saturating_add(1);
                descendants.insert(source);
                for (endpoint, &node_id) in edge.node_ids.iter().enumerate() {
                    // An undirected edge is counted once, even if both IDs agree.
                    if endpoint == 1 && node_id == edge.node_ids[0] {
                        continue;
                    }
                    let counts = incident_counts.entry((node_id, source)).or_default();
                    for (count, contribution) in counts.iter_mut().zip(edge_counts) {
                        *count += contribution;
                    }
                }
            }
        }
        for (i, contact) in self.report.accepted_contacts.iter_mut().enumerate() {
            execution_policy.check_cancelled_every("contact_classification", i)?;
            let counts = match contact.final_node {
                FinalNode::Present { node_id, .. } => incident_counts
                    .get(&(node_id, contact.accepted_source_segment))
                    .copied()
                    .unwrap_or_default(),
                FinalNode::Missing => [0; 5],
            };
            let any_descendant = descendants.contains(&contact.accepted_source_segment);
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
        execution_policy.check_cancelled("contact_classification")
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

#[cfg(test)]
mod contact_execution_tests {
    use super::*;
    use crate::{CancellationToken, Polygon3D, PolygonizeError, PolygonizerResult};

    fn policy_at(work_item: usize) -> ExecutionPolicy {
        let token = CancellationToken::new();
        ExecutionPolicy {
            cancellation_token: Some(token.clone()),
            cancel_at_work_item: Some((token, work_item)),
            ..Default::default()
        }
    }

    fn empty_result() -> PolygonizerResult {
        PolygonizerResult {
            polygons: Vec::new(),
            dangles: Vec::new(),
            cut_edges: Vec::new(),
            invalid_rings: Vec::new(),
            diagnostics: None,
        }
    }

    fn capture(limit: usize) -> ContactCapture {
        ContactCapture::new(&[], limit, &ExecutionPolicy::default()).unwrap()
    }

    fn edge(id: usize, nodes: [usize; 2], sources: Vec<usize>) -> SourceEdgeAudit {
        SourceEdgeAudit {
            edge_id: id,
            node_ids: nodes,
            source_segments: sources,
            start: Coord3D::new(id as f64, 0.0, 0.0),
            end: Coord3D::new(id as f64, 1.0, 0.0),
            polygon_boundary: false,
            output_boundaries: Vec::new(),
            dangle: false,
            cut_edge: false,
            invalid_ring: false,
        }
    }

    fn contact(source: usize, node: Option<usize>) -> AcceptedContact {
        let point = Coord3D::new(0.0, 0.0, 0.0);
        AcceptedContact {
            original_vertex: 0,
            accepted_source_segment: source,
            accepted: point,
            topology: point,
            output: point,
            final_node: node.map_or(FinalNode::Missing, |node_id| FinalNode::Present {
                node_id,
                coordinate: point,
            }),
            final_source_incidence: FinalSourceIncidence::NotTracked,
            absent_reason: None,
        }
    }

    fn assert_cancelled<T>(result: Result<T>, expected_stage: &str) {
        assert!(
            matches!(result, Err(PolygonizeError::Cancelled { stage }) if stage == expected_stage)
        );
    }

    #[test]
    fn contact_capture_checks_before_work_and_during_original_rows() {
        let line = Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 0);
        let cancelled = policy_at(usize::MAX);
        cancelled.cancellation_token.as_ref().unwrap().cancel();
        assert_cancelled(ContactCapture::new(&[], 0, &cancelled), "contact_capture");
        assert_cancelled(
            ContactCapture::new(&[line; 200], 1_000, &policy_at(256)),
            "contact_capture",
        );
        // Cancellation between periodic checkpoints is caught before returning.
        assert_cancelled(
            ContactCapture::new(&[line], 2, &policy_at(1)),
            "contact_capture",
        );
        assert_cancelled(
            ContactCapture::new(&[line], 1, &policy_at(1)),
            "contact_capture",
        );
    }

    #[test]
    fn contact_capture_interrupts_duplicate_xy_acceptance_fanout() {
        let point = Coord3D::new(0.0, 0.0, 0.0);
        let lines = [Line3D::new(point, point, 0); 150];
        let mut capture = ContactCapture::new(&lines, 1_000, &ExecutionPolicy::default()).unwrap();
        assert_cancelled(capture.accept(0, point, &policy_at(256)), "contact_capture");
        assert_eq!(capture.report.accepted_contacts.len(), 256);
    }

    #[test]
    fn contact_capture_interrupts_graph_memberships_and_node_resolution() {
        let line = Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 0);
        let mut capture = capture(1_000);
        for source in 0..300 {
            capture.child(source, line);
        }
        assert_cancelled(
            capture.graph_edge(0, 0, 1, &line, &policy_at(256)),
            "contact_capture",
        );
        assert_eq!(capture.edge_memberships, 256);
        assert_cancelled(
            capture.resolve(&[0.0; 300], &[0.0; 300], &[0.0; 300], &policy_at(256)),
            "contact_capture",
        );
    }

    #[test]
    fn contact_capture_interrupts_topology_mapping() {
        let mut capture = capture(1_000);
        capture.report.accepted_contacts = vec![contact(0, None); 300];
        assert_cancelled(
            capture.map_topology(
                &crate::noding::snap::SnapNoder::new(1.0),
                None,
                &policy_at(256),
            ),
            "contact_capture",
        );
    }

    #[test]
    fn classification_preserves_distinct_edge_counts_absence_and_output_owners() {
        let mut capture = capture(100);
        capture.report.source_edges = vec![
            edge(0, [10, 11], vec![3, 4]),
            edge(1, [10, 12], vec![3]),
            edge(2, [10, 13], vec![3]),
            edge(3, [10, 14], vec![3]),
            edge(4, [20, 21], vec![5]),
            // Even a repeated endpoint must count its undirected edge only once.
            edge(5, [10, 10], vec![3]),
        ];
        capture.report.accepted_contacts = vec![
            contact(3, Some(10)),
            contact(4, Some(10)),
            contact(5, Some(10)),
            contact(6, Some(10)),
            contact(3, None),
        ];
        let pair = |i: usize| {
            vec![
                capture.report.source_edges[i].start,
                capture.report.source_edges[i].end,
            ]
        };
        let mut result = empty_result();
        result.polygons = vec![
            Polygon3D::new(pair(0), Vec::new(), Vec::new(), Vec::new()),
            Polygon3D::new(Vec::new(), vec![pair(0)], Vec::new(), Vec::new()),
        ];
        result.dangles = vec![pair(0), pair(4)];
        result.cut_edges = vec![pair(1)];
        result.invalid_rings = vec![pair(2)];
        capture
            .classify(&result, &ExecutionPolicy::default())
            .unwrap();
        let rows = capture.report.complete_source_incidence().unwrap();
        assert_eq!(
            rows[0].final_source_incidence,
            FinalSourceIncidence::Present {
                polygon_boundary: 1,
                dangle: 1,
                cut_edge: 1,
                invalid_ring: 1,
                other: 2,
            }
        );
        assert_eq!(
            rows[1].final_source_incidence,
            FinalSourceIncidence::Present {
                polygon_boundary: 1,
                dangle: 1,
                cut_edge: 0,
                invalid_ring: 0,
                other: 0,
            }
        );
        for (i, reason) in [
            (2, AbsentReason::SourceNotIncident),
            (3, AbsentReason::NoGraphDescendants),
            (4, AbsentReason::SourceNotIncident),
        ] {
            assert_eq!(rows[i].final_source_incidence, FinalSourceIncidence::Absent);
            assert_eq!(rows[i].absent_reason, Some(reason));
        }
        assert_eq!(
            capture.report.source_edges[0].output_boundaries,
            vec![
                OutputBoundaryRef {
                    polygon_index: 0,
                    interior_ring: None,
                    edge_index: 0,
                },
                OutputBoundaryRef {
                    polygon_index: 1,
                    interior_ring: Some(0),
                    edge_index: 0,
                },
            ]
        );
        assert!(capture.report.complete_output_classification().is_none());
    }

    #[test]
    fn classification_reference_limit_still_discards_lineage() {
        let mut capture = capture(1);
        capture.report.source_edges = vec![edge(0, [0, 1], vec![0])];
        capture.report.accepted_contacts = vec![contact(0, Some(0))];
        let pair = vec![
            capture.report.source_edges[0].start,
            capture.report.source_edges[0].end,
        ];
        let mut result = empty_result();
        result.polygons = vec![Polygon3D::new(
            pair.clone(),
            vec![pair],
            Vec::new(),
            Vec::new(),
        )];
        capture
            .classify(&result, &ExecutionPolicy::default())
            .unwrap();
        assert_eq!(capture.report.lineage_status, LineageStatus::LimitExceeded);
        assert!(capture.report.source_edges.is_empty());
        assert_eq!(
            capture.report.accepted_contacts[0].final_source_incidence,
            FinalSourceIncidence::NotTracked
        );
        assert!(capture.report.complete_source_incidence().is_none());
    }

    #[test]
    fn classification_checks_cancellation_before_incomplete_lineage_return() {
        let mut capture = capture(0);
        capture.lineage.invalidate();
        let policy = policy_at(usize::MAX);
        policy.cancellation_token.as_ref().unwrap().cancel();
        assert_cancelled(
            capture.classify(&empty_result(), &policy),
            "contact_classification",
        );
    }

    #[test]
    fn classification_interrupts_edge_index_construction() {
        let mut capture = capture(1_000);
        capture.report.source_edges = (0..300)
            .map(|id| edge(id, [id, id + 1], vec![id]))
            .collect();
        assert_cancelled(
            capture.classify(&empty_result(), &policy_at(256)),
            "contact_classification",
        );
    }

    #[test]
    fn classification_interrupts_output_traversal() {
        let mut capture = capture(1_000);
        let mut result = empty_result();
        result.dangles = vec![vec![Coord3D::new(0.0, 0.0, 0.0); 300]];
        assert_cancelled(
            capture.classify(&result, &policy_at(256)),
            "contact_classification",
        );
    }

    #[test]
    fn classification_interrupts_membership_aggregation() {
        let mut capture = capture(1_000);
        capture.report.source_edges = vec![edge(0, [0, 1], (0..300).collect())];
        assert_cancelled(
            capture.classify(&empty_result(), &policy_at(256)),
            "contact_classification",
        );
    }

    #[test]
    fn classification_interrupts_contact_lookup_and_checks_before_success() {
        let mut capture = capture(1_000);
        capture.report.accepted_contacts = vec![contact(0, Some(0)); 300];
        assert_cancelled(
            capture.classify(&empty_result(), &policy_at(256)),
            "contact_classification",
        );
        let mut capture = self::capture(10);
        capture.report.accepted_contacts = vec![contact(0, None); 2];
        assert_cancelled(
            capture.classify(&empty_result(), &policy_at(1)),
            "contact_classification",
        );
    }
}
