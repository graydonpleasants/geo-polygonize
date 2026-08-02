# Tiling guarantees and fallback behavior

`TiledPolygonizer` is an experimental Rust-only replicate-and-own pipeline. It
partitions an ownership bounding box into rectangular tiles, expands each tile
by a caller-selected buffer, polygonizes the intersecting input geometries, and
keeps each face in exactly the tile selected by its ownership point.

It is not boundary-graph stitching, streaming ingest, or a certified coverage
algorithm. The tiled API remains outside the stable facade before `1.0`.

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

`TileReport::input_boundary_issues` also records the stable input geometry index,
envelope, and internal halo sides for each included geometry whose envelope
reaches beyond that halo. This conservative witness remains available when no
local face is reconstructed, so it catches split-boundary cases that face-only
evidence cannot see. It reports unresolved connectivity, not a confirmed missing
face: whole input geometries are replicated and may already contain everything
needed locally. Full output traces record both evidence families as bounded
`tile_input_boundary` and `tile_owned_face_boundary` events without rescanning
inputs or reconstructed faces.

`TileReport::excluded_component_issues` covers one additional missing-input
case. Separate input geometries that share exact endpoints are grouped. When
input noding is enabled, an indexed exact-intersection pass also groups geometry
connected through segment interiors. When `pre_snap_tolerance` is enabled, the
same bounded preflight applies that caller-selected transformation before
grouping, and reports those components as `PreSnap`. When a fixed-grid precision
model is enabled, it applies the selected snap strategy to the same preflight
endpoints and reports those components as `FixedGrid` unless pre-snap evidence
takes precedence. A tile reports the
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
key contains exact XYZ bits, not a tolerance. It does not merge distinct
provenance payloads, so callers requiring provenance equivalence must verify the
retained polygon against untiled output.

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
  conservative input-boundary, or excluded component evidence is present.

Both validation modes are deliberately narrower than coverage certification.
`with_execution_policy` applies segment, candidate, exact-predicate, and
cancellation bounds to the indexed component preflight, including the optional
pre-snap and fixed-grid endpoint passes, and passes the same policy to each tile
polygonization. Numeric work limits apply independently to the preflight and to
each tile, not as one aggregate budget across the tiled call. The component
envelope remains conservative evidence rather than proof of a face.
`with_retry_policy` enables deterministic tile-local halo growth for tiles whose
final report still contains owned-face, input-boundary, or excluded-component
evidence. Each attempt adds `buffer_increment` up to `max_attempts` and
`max_buffer`, replaces that tile's earlier polygons and report, and records the
attempt in `TileReport` and `StitchingReport`. Strict validation returns the
typed coverage error with retry counts when the bounded schedule is exhausted.
Retries reuse the same execution policy independently per attempt. Full output
traces record bounded `tile_halo_retry` events directly from the physical retry
history. `with_component_fallback` enables a narrower recovery path for excluded
components whose envelope is disjoint from every non-member input envelope,
retained tile polygon, and other recovered component. Member linework may have
appeared in a tile halo; if that produced a retained face intersecting the
component envelope, recovery declines. Otherwise it polygonizes the complete
component with the same options, merges the result deterministically, records
`component_fallback_used` and a bounded `tile_component_fallback` event. The
event includes the recovered polygon count and the number of retained tile
polygons present at the merge boundary. Recovery declines when any envelope
overlaps or nests. `with_untiled_fallback`
remains the containment-safe escape hatch for those declined cases. General
cross-region graph merging remains unavailable.
Applications that require correctness must run an untiled equivalence check for
their input class or choose untiled polygonization directly when sufficiency is
unknown.

The tiled implementation materializes its tile list and per-tile results; it is
not a bounded-memory or out-of-core subsystem. Streaming work begins only after
coverage detection and deterministic recovery exist.
