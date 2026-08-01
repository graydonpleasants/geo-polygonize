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
- `ValidateObservedCoverage` returns that typed error when either owned-face or
  conservative input-boundary evidence is present.

Both validation modes are deliberately narrower than coverage certification.
They cannot certify missing connected regions whose geometry was excluded from
every halo.
The checked-in excluded-component fixture demonstrates this boundary explicitly:
untiled polygonization reconstructs an enclosing face while every tiled halo
observes no member geometry, no face, and no coverage issue.
There is currently no halo retry, unresolved-region retry, untiled fallback, or
retry budget. No retry or fallback trace event is emitted because those execution
paths do not exist.
Applications that require correctness must run an untiled equivalence check for
their input class or choose untiled polygonization directly when sufficiency is
unknown.

The tiled implementation materializes its tile list and per-tile results; it is
not a bounded-memory or out-of-core subsystem. Streaming work begins only after
coverage detection and deterministic recovery exist.
