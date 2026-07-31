# Topology and output semantics

Polygonization builds polygonal faces from linework. It does not repair every
invalid geometry, infer missing boundaries, or return the input lines unchanged.
Topology is computed in XY; Z and source provenance are attributes reconstructed
after the XY topology is established.

## Input contract

With `node_input = false`, every intersection that should participate in the
graph must already be an endpoint on both incident lines. Crossing lines without
a shared vertex are not implicitly connected. Enable noding, and select an
appropriate guarantee, when that precondition is not known to hold.

Coincident and overlapping segments are dissolved into topology edges. Their
source IDs remain available for provenance, but duplicate boundaries do not
create duplicate faces. Zero-length segments and rings that cannot bound a
positive finite area do not produce polygons.

## Result families

`PolygonizerResult` keeps four topology result families:

- `polygons` are positive-area faces assembled from valid shell and hole cycles.
- `dangles` are dead-end line chains removed recursively from the graph.
- `cut_edges` remain connected after dangle removal but do not bound a retained
  ring.
- `invalid_rings` are closed ring candidates rejected during ring validation or
  face assembly.

These classifications describe this polygonization run. They are not a general
validity verdict for the original dataset. In particular, a line may be useful
to another operation even when it is a dangle or cut edge here.

`PolygonizerResult::into_multi_polygon()` intentionally discards dangles, cut
edges, invalid rings, diagnostics, representative edge IDs, provenance, and Z.
Keep the full result or a binding's report output when those values matter.

## Shells, holes, and filtering

Closed cycles are classified by deterministic containment. The configured
`TouchPolicy` decides how touching candidates participate in containment;
changing it may intentionally change shell/hole assignment. Nested shells and
holes are retained when their boundaries satisfy the selected policy.

`extract_only_polygonal` removes shells that are not part of the selected
polygonal subset. `output_filter.minimum_face_area` is applied only after
topology and provenance are established. It keeps faces whose unsigned area is
at least the configured value; it does not alter noding or graph construction.

## Deterministic output

The default determinism options canonicalize ring starts, orientations, holes,
polygons, and non-polygon result families. Representative edge IDs rotate and
reverse with their coordinates, while aggregate boundary provenance is sorted
and deduplicated. `-0.0` is normalized only in the versioned conformance
encoding; ordinary coordinate values remain `f64` values.

Canonical ordering makes equivalent runs comparable. It does not make two
different precision, noding, containment, Z, or filtering profiles semantically
equivalent. Use the versioned topology fingerprint or normalized error contract
for exact cross-binding conformance rather than comparing polygon counts alone.

## Failure boundary

Invalid options, non-finite coordinates, unsupported option combinations,
noding validation failures, topology failures, resource limits, and
cancellation return typed errors. A successful call means the selected pipeline
completed; unchecked noding does not certify that the input or produced segments
are fully noded. The [getting-started guide](./getting-started.md) summarizes the
available validation levels and precision contracts.
