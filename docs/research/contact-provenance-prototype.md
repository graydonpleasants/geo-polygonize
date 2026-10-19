# Bounded native pre-snap contact provenance (experimental)

Base: released 1.3.0, `9d6ac79d8fbbb2edf79bebcbb3b28f9b3861f0de`.
This is an experimental Rust research prototype, not a supported facade or a consumer fix.

## Opt-in contract

`Polygonizer::polygonize_with_contact_provenance(max_records)` returns the unchanged
`PolygonizerResult` alongside a separate `contact::ContactReport`. It requires
positive pre-snap tolerance, noding enabled, and the iterative noder. Certified
hot-pixel noding is deliberately rejected. Existing options/results and ordinary
JSON serialization are unchanged. All new surfaces are documentation-hidden.

The record cap counts original-vertex rows plus accepted-contact rows. Auxiliary
capture indexes scale with that cap. Once truncated, `complete_presnap_acceptances()` returns
`None`; a consumer must reject the entire relation for absence/completeness tests.
This bounds capture memory, not the pre-existing geometry processing work. The
report stores values, not an allocation of size `max_records` in advance.

Original identities are `(flattened input segment index, Start|End)`, independently
of absent, duplicate, or zero line IDs. An interior polyline vertex can appear as
two distinct segment endpoint identities. Raw XYZ is copied without Z-policy
changes. Original endpoints at the same XY but different Z remain distinct.

For every actually retained pre-snap point, capture records each original endpoint
with exactly equal XY and the actual original segment accepting that point. This
is captured after the production pre-snap point sorting/dedup, before emitted
segments are reordered. No epsilon, priority selection, final-coordinate distance,
or mere grid-cell sharing is used to invent incidence. Generated intersection
vertices without an original endpoint at exactly that XY are outside this report.
Endpoint relocation is separately recorded on the original row; it is not confused
with acceptance of that endpoint's original position.

Topology coordinates are computed by the actual configured snapper; output XY uses
the same GeosCompat restoration map as geometry. Final node IDs/XYZ are looked up
exactly in the graph immediately after graph bulk load, before pruning. Missing
nodes are explicit. Node IDs are call-local; deterministic identity comparisons
across input permutation must use coordinates and remap input indexes.

## Critical limitation: final source-edge incidence is NotTracked

`final_node = Present` proves only that the mapped pre-pruning graph node exists.
A collapsed source can map to a node supported solely by another source. A dangle
can be removed later. Every row therefore explicitly has
`final_source_incidence = NotTracked`. No consumer should treat this report as proof
of a surviving polygon-boundary/source contact. XY contact also asserts nothing
about Z compatibility. Absence in this raw-XY acceptance relation does not prove absence of final contact: an original endpoint can itself move during pre-snap. This first stage distinguishes accepted pre-snap incidence
from coordinate-only collapse; it is not yet the complete downstream drtwrk fix.

## Smallest next source-lineage extension

1. Introduce an internal source segment key independent of caller line IDs, with
   sets carried beside noded segments. Pre-snap child segments inherit the input
   segment key; retain all accepted endpoint identities before source dedup.
2. Propagate these sets through iterative `SnapNoder::node_impl`, initial snap and
   degenerate filtering, `normalize_and_dedup` (union sets when merging), and
   `apply_split_events` (children inherit the full set). The exact-node reference
   pass must preserve geometry behavior. Do not replace line IDs with surrogate
   IDs, because that changes dedup/tie behavior.
3. Carry the sets into `PlanarGraph::bulk_load` edge creation and duplicate-edge
   merging, parallel to existing line-ID `EdgeSources`. Join each accepted row
   against source membership of edges actually incident at its final node.
4. Expose separate pre-pruning and surviving/output-boundary statuses after dangle,
   cut-edge, and polygon extraction classification. A source whose edges all
   collapse or are removed must have an explicit reason, never inferred survival.
5. Keep certified, tiling, and other unsupported paths rejected until each has
   equivalent lineage propagation tests. The first consumer experiment can target
   only the exact iterative global mode used in the real witness.

## Python full/lean route

Do not add fields to existing full/lean responses by default. Once semantics are
reviewed, a separate experimental binding entry point can invoke this hidden method
and return `{result, contact_provenance}` with an explicit record limit and schema
version. Both full and lean serializers must preserve all ordinary result data and
emit the separate report only for that entry point. Source identities must map from
flattened segments back to caller line/vertex indexes before use in drtwrk. The
prototype intentionally changes no Python API, wrapper, release allowlist, package
version, or published artifact. Promoting a stable optional API requires normal
API/schema review and an appropriate future release, not claiming a patch is
semver-neutral simply because it is opt-in.

## Verification at stage-1 freeze

- `cargo test -p geo-polygonize-core`: 619 passed, 0 failed, 2 existing exhaustive
  tiling tests ignored. Includes 12 dedicated contact tests and capture cleanup.
- `cargo clippy -p geo-polygonize-core --all-targets -- -D warnings`: passed.
- `cargo fmt --all --check` and `git diff --check`: passed.
- A prior serial-only core unit run passed 523 tests before the final added cleanup
  unit test; default-feature full core tests above cover the final source.
- Dependency `proc-macro-error2 2.0.1` emitted Cargo's existing future-compatibility
  advisory; no new Clippy warnings. Other workspace crates/WASM/Python were not built.

Opt-out microbenchmark: same 800 segments (200 disjoint rings), 100 ordinary calls
per sample after 5 warmups, release/no-default-features, same Rust 1.99.1 toolchain.
Twenty alternating samples per binary after our tests/builds finished gave median
238.371 ms for release 1.3 versus 235.645 ms for the modified ordinary path (-1.14%).
An earlier 12-pair run overlapping other work measured +4.99%; this variation means
we do not claim a speedup or a proven zero overhead. No clear stable opt-out
regression was established by this small benchmark. Raw final samples are in
`contact-overhead.json`; `contact_overhead.rs` reproduces the workload. Broader
corpus/allocation benchmarking remains necessary before API promotion.

## Stage 2: exact surviving source lineage

Stage-1 freeze remains `f0d6e9a03da598eb4af3d5017a381940ed9d94c5`; the limitation
above describes that freeze. Stage 2 replaces `NotTracked` on complete reports
with exact pre-pruning source-edge incidence and separate output classification.
This remains a hidden, experimental research API, with no Python/full/lean change.

Pre-snap children are explicitly assigned their original flattened segment index
at emission. A bounded sidecar carries sets through the actual iterative noder:
initial snapping drops collapsed children; split children inherit all parent
sources; unchanged segments retain theirs. Canonical exact XY endpoint + caller-ID
keys mirror the production normalization/dedup partition. Equal keys union source
sets, including reversed duplicates and different-Z segments. Caller IDs are
never overwritten or used as original identities. The exact-noded reference pass
still only supplies reference vertices: source attribution comes from the actual
original-segment pre-snap emission, not that pass's deduplicated line IDs.
Restoration remaps sidecar generations alongside the same actual coordinate
restoration used by geometry, and Z reconciliation does not alter source keys.

At graph dissolve/creation, each actual input segment contributes its carried
source set to the actual graph edge ID, including all merged duplicates. The
separate `source_edges` audit retains edge IDs, endpoint node IDs, original-source
sets, graph XYZ, and classification flags. It survives component-local graph
processing; no source is inferred from a coincident final node or polygon.
An accepted row is `Present` only if its original accepting-source index is in
an actual graph edge incident at the row's mapped node. `Absent` is accompanied
by `NoGraphDescendants` (including collapse) or `SourceNotIncident`. Endpoint
relocation remains separately recorded and is not invented as raw-XY acceptance.

Output classification compares exact output edge XY pairs to already-proven
source graph edges, independent of source IDs and Z. Counts distinguish polygon
boundary, dangle, cut edge, invalid ring, and `other`. More than one category can
occur at the same contact (e.g. one source is partly boundary and partly dangle).
`other` explicitly means the edge has no exact representation in classified
output; filtered, transformed, or otherwise unrepresented edges must never be
promoted to boundary survival. This is edge classification, not polygon ownership
or Z compatibility. `Present` alone is therefore not a polygon-boundary claim.

`max_records` still caps original+acceptance rows. Independently, it caps source
memberships in each lineage generation and graph-edge memberships. Maps contain
no more keys than memberships; there is no input-sized lineage allocation or
full-input lineage clone. Two split generations, the graph audit, and bounded
classification indexes can coexist, so the budget is a constant-factor storage
bound, not a byte limit or a bound on ordinary geometry work. On membership
exhaustion `lineage_status=LimitExceeded`, graph audit is discarded and all contact
incidence remains `NotTracked`. Missing exact parents instead produce
`IncompletePropagation`. `complete_source_incidence()` returns `None` on either
failure or truncated acceptance capture. `complete_presnap_acceptances()` still
only certifies the distinct pre-snap raw-XY acceptance relation; never use it to
certify final-contact completeness.

Global iterative noding remains the only supported mode. Certified noding,
partition export, and simultaneous trace capture are rejected before capture.
Diagnostics use the identical stats-producing noder path; elapsed phase timings
are inherently variable and excluded only from instrumented/ordinary comparison.
The ordinary path uses no lineage allocations and keeps its prior geometry,
caller-ID tie-breaking, Z, and serialization behavior.

`complete_source_incidence()` certifies source membership only. To require a fully
classified boundary/dangle/cut/invalid relation, use
`complete_output_classification()`: it additionally returns `None` if any captured
graph edge is `other`. In particular a face removed by output-area filtering does
not yield a supposedly complete boundary-absence result.

Each audited edge also exposes bounded `output_boundaries` references containing
`polygon_index`, optional `interior_ring` (None means exterior), and `edge_index`
into the final returned result after canonical ordering. The referenced pair is
exactly ring[edge_index] -> ring[edge_index+1]. Shared polygon boundaries retain
all occurrences. To qualify a selected candidate, require both the accepted row's
mapped node ID and accepting source index on that audited edge, then select its
exact output-polygon reference. Presence of a source elsewhere in the pass does
not satisfy this join. References are independently capped by `max_records`;
overflow invalidates and discards the entire final-lineage audit.

### Stage-2 verification

- Default-feature full core tests: 638 passed, 0 failed, 2 existing exhaustive
  tiling tests ignored. This includes 12 retained stage-1 tests, 15 new stage-2
  tests, three direct lineage-budget/union tests, and an independent review oracle
  checking 300 randomized exact axis-aligned memberships plus targeted cases.
- Serial-only core library tests: 527 passed, 0 failed.
- Clippy all core targets with `-D warnings`, formatting and diff whitespace checks
  pass. Cargo still reports the existing proc-macro-error2 future-compatibility
  advisory. Other workspace crates, Python and WASM were not built.
- Ordinary/instrumented geometry and serialized diagnostics (excluding elapsed
  phase timings) match across fixtures. Exact output references round-trip to the
  final output, including shared boundaries and hole references.
- Separate tests exhaust row capture, split membership, and output-reference
  budgets; each applicable completeness accessor fails closed.

### Public draft scope and downstream experiment

The public draft excludes a private drawing-derived fixture and its two dedicated
regressions. The historical stage-1 and stage-2 totals above include those tests;
they describe the separately preserved local validation, not the public suite.
All published new regression inputs are synthetic. No private drawing coordinates,
WKB, CAD handles, target geometry, or whole-drawing consumer output are included.

A separate private one-target consumer diagnostic joined original source identity,
accepted pre-snap contact, incident source graph edge, and final output polygon
reference. At tolerances 0.5 and 0.1 it selected the right-of-way candidate rather
than the sidewalk candidate, with candidate geometry unchanged. This is limited
external validation, not a reproducible public regression, six-closure completion,
full XYZ compatibility, or grading validation. No public Python binding is added.

A stage-2 versus stage-1 opt-out microbenchmark measured +2.13% median elapsed time
(242.182 versus 237.121 ms per 100 calls). It used 20 alternating paired samples,
800 synthetic segments forming 200 disjoint rings, five warmups, release mode and
no default features on the same Rust 1.99.1 toolchain. This small synthetic workload
is not a representative corpus or proof of zero overhead. The older measurements
above and in contact-overhead.json compare stage 1 against released 1.3.0.

Public-only validation after excluding the two private-fixture tests: 636 core
tests passed, 0 failed, 2 existing exhaustive tiling tests ignored; serial-only
core library: 527 passed. Clippy all core targets with `-D warnings`, formatting,
and diff whitespace checks pass. The private fixture was tested separately and
is excluded from this pull request and its commit history.
