# Z and provenance behavior

Topology is always two-dimensional. XY determines intersections, graph nodes,
rings, containment, and faces. Z values and input source IDs are carried through
that topology so callers can reconstruct elevations and explain which boundaries
formed each polygon.

## Z reconstruction

`ZPolicy` controls the Z assigned to endpoints introduced or moved by noding:

- `InterpolateAlongEdge` linearly interpolates Z at the constructed XY position.
- `PreferNearestEndpoint` uses the nearer source endpoint. An exact midpoint uses
  a deterministic endpoint ordering.
- `Ignore` emits `0.0` for every graph coordinate.
- `ErrorOnConflict` interpolates, then fails when one XY node receives Z values
  whose range exceeds `z.conflict_tolerance`.

After noding, all segment endpoints at the same XY node are reconciled to one Z.
Candidates are ordered by source line ID, then Z and stable segment data. Unless
`Ignore` or `ErrorOnConflict` changes the outcome, the lowest line ID and then
lowest Z wins. This rule makes the result independent of input order; it does not
average elevations from unrelated source lines.

A conflict exists only when `max_z - min_z` is greater than the configured
non-negative tolerance. `ErrorOnConflict` returns a structured error containing
the XY coordinate and sorted unique contributing line IDs. Other policies retain
the deterministic Z and expose conflict counts through diagnostics. Bounded full
traces can also retain exact XY/Z bit patterns, ordered candidates, policy,
tolerance, and the retained value.

Use `Line3D` in Rust or typed buffers with `stride = 3` in Python and Wasm when Z
must be preserved. GeoRust, GeoJSON, and the current GeoArrow adapters are XY-only
and therefore cannot provide the same Z conformance contract.

## Edge identity and complete provenance

Every live topology edge retains a sorted, unique set of contributing input
`line_id` values. Coincident or overlapping boundaries dissolve to one edge
without losing that complete source set. The edge's representative ID is the
lowest source ID and is stored per ring edge in `exterior_ids` and
`interiors_ids`.

Representative IDs and aggregate provenance serve different purposes:

- representative IDs preserve one deterministic source identity for each edge;
- `PolygonProvenance.boundary_line_ids` is the sorted, deduplicated union of all
  contributing nonzero boundary IDs for the polygon; and
- `input_profile_id` carries the caller-supplied dataset/profile label without
  affecting topology.

Set `provenance.enabled = true` to attach `PolygonProvenance`. Set
`include_boundary_line_ids = true` to populate its aggregate boundary IDs;
otherwise the provenance object contains an empty boundary list and any supplied
profile ID. Input `line_id = 0` is the anonymous/default identity and is omitted
from aggregate boundary provenance.

Canonical ring rotation and reversal move representative IDs with their edge
coordinates, preserving multiplicity and attribution. Aggregate provenance is
sorted and deduplicated. The versioned topology fingerprint retains both forms,
plus exact Z output, so conformance comparisons can detect an attribution or Z
change even when polygon XY coordinates are identical.

## Lossy adapters and outputs

Converting a result to `MultiPolygon<f64>` drops Z, representative IDs, complete
provenance, diagnostics, and non-polygon result families. Binding object modes or
GeoJSON-style polygon outputs may be similarly lossy. Use the full Rust result,
Python report mode, or Wasm typed-buffer report when exact Z/provenance evidence
is required, and compare the versioned topology fingerprint rather than geometry
alone.
