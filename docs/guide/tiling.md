# Tiling guarantees and fallback behavior

`TiledPolygonizer` is an experimental Rust-only replicate-and-own pipeline. It
partitions an ownership bounding box into rectangular tiles, expands each tile
by a caller-selected buffer, polygonizes the intersecting input geometries, and
keeps each face in exactly the tile selected by its ownership point.

It is not boundary-graph stitching, streaming ingest, or a certified coverage
algorithm. The tiled API remains outside the stable 1.x facade.

## Current equivalence contract

Tiled output is equivalent to untiled output only when the caller can establish
that every face owned by a tile can be reconstructed from the complete input
geometries whose bounding boxes intersect that tile's buffered extent. Input
geometries are replicated whole; they are not clipped at tile borders.

The checked-in equivalence gate covers faces crossing one, two, four, and many
tile boundaries, plus a narrow concavity, a boundary-crossing hole, dirty
intersections, and exterior dangles. Those fixtures use:

- `RepresentativePointInsidePolygon` ownership;
- `CanonicalRingHash` deduplication; and
- a buffer chosen large enough for the complete owned face.

This is evidence for that input class, not a general proof that an arbitrary
buffer is sufficient. `TileReport::coverage_issues` reports an owned face whose
envelope reaches an internal buffered-tile boundary, including the affected
sides, face envelope, representative edge source IDs, and complete aggregate
source IDs when provenance was requested. `StitchingReport` summarizes the
affected tiles and faces. Absence of this definite witness does not certify
coverage because missing external linework may leave no reconstructed face to
inspect.

`TileReport::ownership_domain_issues` reports a reconstructed face whose
selected ownership point falls outside the configured ownership bounding box
while the face envelope overlaps that box. No generated tile can own that
face. This is evidence only: tiled BestEffort output remains domain-scoped and
does not clip or append the face implicitly. `ValidateObservedCoverage`
rejects the evidence, while `with_untiled_fallback` can opt into the existing
whole-input result, which preserves the complete untiled face.

`TileReport::input_boundary_issues` also records the stable input geometry index,
envelope, and internal halo sides for each included geometry whose envelope
reaches beyond that halo. This conservative witness remains available when no
local face is reconstructed, so it catches split-boundary cases that face-only
evidence cannot see. It reports unresolved connectivity, not a confirmed missing
face: whole input geometries are replicated and may already contain everything
needed locally. Full output traces record the three evidence families as
bounded `tile_input_boundary`, `tile_owned_face_boundary`, and
`tile_ownership_domain` events without rescanning inputs or reconstructed
faces.

`TileReport::excluded_component_issues` covers one additional missing-input
case. Separate input geometries that share exact endpoints are grouped. When
input noding is enabled, an indexed exact-intersection pass also groups geometry
connected through segment interiors. When `pre_snap_tolerance` is enabled, the
same bounded preflight applies that caller-selected transformation before
grouping, and reports those components as `PreSnap`. When a fixed-grid precision
model is enabled, it applies the selected snap strategy to the same preflight
endpoints and reports those components as `FixedGrid` unless pre-snap evidence
takes precedence. With `NodingGuarantee::CertifiedFixedPrecision`, the
preflight reuses the bounded hot-pixel noder so snap-created shared vertices
are included in the same `FixedGrid` component evidence. A tile reports the
component when its combined envelope intersects the buffered tile but no member
geometry envelope does. This catches an enclosing component that is excluded
from every halo. It remains conservative: an envelope does not prove that the
component contains a face. `TileComponentConnection` distinguishes endpoint,
segment-intersection, pre-snap, and fixed-grid evidence, and `StitchingReport`
counts affected tiles and issue instances. Full output traces distinguish
bounded `tile_excluded_endpoint_component`,
`tile_excluded_segment_component`, `tile_excluded_pre_snap_component`, and
`tile_excluded_fixed_grid_component` events.

## Ownership and deduplication

Tiles use half-open `[min, max)` ownership intervals, except that the final row
and column include the outer maximum. Ownership policy determines the point used
for that test:

- `RepresentativePointInsidePolygon` uses a point guaranteed to intersect the
  polygon interior when one exists and is the safest current choice.
- `Centroid` is the default but may lie outside a concave polygon.
- `LexicographicMinVertex` is deterministic but chooses a boundary vertex.

`KeepAll` performs no duplicate removal. `CanonicalRingHash` removes exact
duplicate polygon geometry after canonicalizing ring direction and start; its
key contains exact XYZ bits with signed zero normalized, not a tolerance. It
merges duplicate aggregate boundary source sets and compatible input profiles.
If profiles conflict, the merged profile is unset. The first duplicate remains
the deterministic representative for per-edge IDs.

Canonical output options apply the normal final ordering pass after tile merge.
The untraced path may process tiles in parallel, while bounded tracing processes
them in tile order so ownership and deduplication events remain deterministic.

## Failure and fallback boundary

The implementation validates the tile size, buffer, bounding box, and semantic
polygonizer options. A per-tile polygonization error stops the whole call and is
returned to the caller.

`polygonize_with_coverage_guarantee` provides two explicit modes:

- `BestEffort` preserves the existing output behavior and reports detected issues.
- `ValidateOwnedFaces` returns `TiledPolygonizeError::CoverageIncomplete` instead
  of polygons when a reconstructed owned face reaches an internal halo boundary.
- `ValidateObservedCoverage` returns that typed error when owned-face,
  ownership-domain, conservative input-boundary, or excluded component
  evidence is present.

Both validation modes are deliberately narrower than coverage certification.
`with_execution_policy` applies segment, candidate, exact-predicate, and
cancellation bounds to the indexed component preflight, including the optional
pre-snap and fixed-grid endpoint passes, and passes the same policy to each tile
polygonization, tile input/evidence filtering, ownership pass, and recovery
region. Numeric work limits apply independently to the preflight, each tile,
and each recovery region, not as one aggregate budget
across the tiled call; the final canonical merge also enforces the configured
aggregate output-polygon limit and observes cancellation. The component
envelope remains conservative evidence rather than proof of a face.
`with_tile_execution_policy` adds call-level guards for tile count, input
geometry count, replicated geometry assignments, total retry attempts,
fallback-region count, and optional per-call rayon parallelism. Tile generation
checks cancellation before materialization and reserves the validated tile
count exactly; tile failures use a result-aware parallel reduction.
`with_retry_policy` enables deterministic tile-local halo growth for tiles whose
final report still contains owned-face, input-boundary, or excluded-component
evidence. Each attempt adds `buffer_increment` up to `max_attempts` and
`max_buffer`, replaces that tile's earlier polygons and report, and records the
attempt in `TileReport` and `StitchingReport`. Strict validation returns the
typed coverage error with retry counts when the bounded schedule is exhausted.
Ownership-domain evidence is not retried because increasing a halo cannot move
an ownership point into the configured domain.
Retries reuse the same execution policy independently per attempt. Full output
traces record bounded `tile_halo_retry` events directly from the physical retry
history. `ExecutionPolicy::max_tile_retry_attempts` optionally caps retry
attempts per tile; exceeding that operational budget returns the typed
`ResourceLimitExceeded` error before the next tile attempt. The retry policy
still controls the halo schedule. `with_component_fallback` enables
conservative recovery for excluded
or partially observed indexed components. It starts with each component
identified by excluded-component or input-boundary evidence, closes the region
over intersecting input envelopes, interacting retained polygon envelopes, and
other connected component envelopes, then polygonizes that complete region once.
Retained polygons intersecting the recovered region are replaced before the
existing canonical deduplication and ordering pass, preserving containment for
the envelope-closed class without independently appending nested output. The
recovery records retained/recovered/replaced counts and a
`TileCoverageResolution` ledger in `StitchingReport`, plus a bounded
`tile_component_fallback` event containing
the region input indexes, recovered component count, and replacement count.
When owned-face coverage evidence co-occurs with indexed component or
input-boundary evidence, every owned-face witness must intersect an
envelope-closed recovery region; a witness outside those regions declines
component recovery rather than replacing unrelated retained output.
`with_untiled_fallback` remains the containment-safe escape hatch for observed
cases that decline region-local recovery, including a reconstructed face that
cannot be owned by any tile. General boundary-graph stitching remains
unavailable. When enabled for unresolved output, a declined component recovery
is marked in `StitchingReport` and recorded as a bounded
`tile_component_fallback_declined` trace event before the
`tile_untiled_fallback` event. The whole-input fallback only runs when tile
reports contain observed unresolved evidence. It cannot detect an unowned
single-geometry face whose envelope never overlaps the configured ownership
domain, and it never clips that input implicitly.
`StitchingReport` separates `untiled_fallback_attempted`,
`untiled_fallback_authoritative`, and `untiled_fallback_output_polygon_count`;
the older `untiled_fallback_used` field is an alias for authoritative state.
The coverage ledger assigns each observed issue a deterministic
tile/family/index identity and records resolved versus unresolved counts.
`ValidateObservedCoverage` rejects whenever its `unresolved_issue_count` is
nonzero, including partial component recovery.
Component fallback checks the execution policy before region selection and
before each recovery region, so cancellation does not get lost between tile
processing and the bounded region polygonizer. The same policy is checked while
replacing retained polygons, appending recovered output, and deduplicating the
fallback merge, so a cancellation request is not lost after recovery finishes.
Applications that require correctness must run an untiled equivalence check for
their input class or choose untiled polygonization directly when sufficiency is
unknown.

The tiled implementation materializes its tile list and per-tile results; it is
not a bounded-memory or out-of-core subsystem. Streaming work begins only after
coverage detection and deterministic recovery exist.
