# Engineering Roadmap

This is the active, evidence-gated roadmap for `geo-polygonize` after the
`1.1.0` release. The supported 1.x facade is production-supported. Graph,
noding, trace, differential, tiling, partition-mosaic, and other research
surfaces remain intentionally unstable even where they are compiler-public and
marked `#[doc(hidden)]`.

Git history and closed issues preserve the detailed delivery record. This
document focuses on work that can still materially improve correctness,
operability, representative performance, topology scale, and ecosystem trust.

## Current state

The supported 1.x baseline includes:

- a narrow GeoRust-native Rust facade plus Python, WebAssembly, Arrow,
  GeoParquet, FlatGeobuf, and C Data Interface paths;
- deterministic canonical topology fingerprints and normalized errors across
  equivalent bindings;
- floating and fixed precision models, independent noding validation, and
  certified hot-pixel fixed-precision noding;
- explicit Z policies and complete edge-dissolve provenance;
- execution budgets, cooperative cancellation, panic containment, and
  ownership-safe FFI boundaries;
- strict golden, compatibility, fuzz, metamorphic, and external GEOS/JTS
  correctness gates;
- bounded topology traces, automatic differential minimization, and an
  interactive browser debugger;
- correctness-gated benchmark schemas, dedicated-runner publication, and
  production-scale 1k/10k/100k workloads;
- managed packed-result wrappers for standard, slim, and threaded Wasm entry
  points.

The private research stack now also includes:

- a DCEL-like arrangement with persisted `next` links, deterministic local face
  identities, component-local processing, and arrangement validators;
- experimental replicate-and-own tiling with bounded work, explicit coverage
  evidence, deterministic retries, conservative region fallback, and
  whole-input fallback;
- physically boundary-noded partition arrangements and atomic border
  observations;
- a completed bulk-versus-single-partition oracle;
- versioned local partition snapshots and a transactional `PartitionMosaic`;
- typed physical consistency and topology-readiness evidence;
- a private streamed source-segment router with native/Wasm measurement paths;
- a private physical-arrangement witness, one-to-one candidate adoption, and
  many-to-one physical alias validation;
- an additive stitched-output sidecar gated by private readiness and optional
  same-options untiled comparison.

The current frontier is no longer basic polygonization. It is:

1. consuming many-to-one physical aliases without losing face-qualified
   topology;
2. deciding whether streamed partition routing earns production use;
3. making partition replacement scale with the changed partition rather than
   the full mosaic;
4. evaluating flat snapshot and checked integer fixed-grid representations;
5. retaining the strict stable/research boundary through future releases.

## North star

Make `geo-polygonize` the best-supported pure-Rust planar
linework-to-polygons kernel across native Rust, Python, WebAssembly, and Arrow:

- certified correctness when the selected precision policy promises it;
- deterministic, explainable output with complete source provenance;
- one semantic contract across equivalent bindings;
- bounded and cancellable execution on difficult or untrusted inputs;
- production-scale performance claims backed by public, reproducible evidence;
- scalable partitioned topology without weakening global face semantics;
- explicit stable APIs and equally explicit private research contracts.

The target is a defensible contract:

> For a documented input class and precision policy, `geo-polygonize` produces
> a deterministic, independently validated topology result, exposes enough
> evidence to explain failures, and does so competitively across supported
> targets.

## Operating rules

These rules apply to every milestone:

- Keep semantic controls in `PolygonizerOptions`; keep limits, cancellation,
  tracing, and other operational controls in execution policies.
- Never silently change precision, guarantees, or effective topology policy.
- Never return partial topology unless a future API explicitly models
  partial/resumable computation.
- Preserve deterministic canonical output, provenance, representative IDs, Z
  behavior, diagnostics, and normalized errors through every optimization.
- Feed experimental candidate generators through the shared exact/split/dissolve
  path and independent validator.
- Treat replicate-and-own tiling, physical partition consistency, and global
  face stitching as distinct contracts.
- Do not independently finalize disconnected graph components when global
  containment can nest them.
- Do not expose a backend, dispatch rule, stitched mode, or representation
  change until its promotion gate passes.
- Do not publish performance claims before correctness gates pass.
- Treat hosted/shared-runner timings as diagnostic, not decision-quality.
- Keep research surfaces unsupported until their contracts are complete.
- Record rejected experiments and remove losing implementations.
- Prefer stacked, independently reviewable PRs over one large branch.

## Active dependency map

```text
Stable 1.x release governance
└── #1410 current release PR and registry verification

Production-scale baseline suite
├── #1291 component execution/layout decisions
└── #1392 streamed partition-router decision

Completed local partition oracle #1389
├── #1513 structured mismatch witnesses
└── #1392 router promotion evidence

Completed transactional mosaic #1390
├── #1391 alias-aware face-qualified topology
├── #1512 delta-indexed replacement and resumability
└── #1393 flat snapshots and checked integer fixed-grid space

Alias-aware face-qualified topology #1391
└── full stitched-output and same-options untiled-equivalence gate
```

Work may proceed in parallel across these tracks, but promotion decisions must
respect the dependencies above.

The partition work is inspired in part by
[`nyurik/map-tile-toolkit`](https://github.com/nyurik/map-tile-toolkit). It is
not a runtime dependency. Its `Mosaic` reconstructs polylines and its
[`PolygonMosaic`](https://github.com/nyurik/map-tile-toolkit/blob/main/src/mosaic.rs)
reconstructs rings after dropping synthetic clip corners. Those guarantees
are not a substitute for global arrangement, containment, provenance,
representative-ID, Z, diagnostics, and untiled-equivalence proof.

# P0 — Stable release integrity and 1.x governance

## P0.1 Delivered release contract

- [x] Exact source-version synchronization across Rust, npm, and Python.
- [x] Registry publication verification and real-registry installation smoke.
- [x] Machine-readable publication reports and repair procedures.
- [x] Exact MSRV policy and CI coverage.
- [x] Supported-facade allowlist and semver checks.
- [x] Explicit governance for compiler-public `#[doc(hidden)]` research APIs.
- [x] 0.x → 1.0 migration guidance.
- [x] Managed Wasm packed-result ownership and lifecycle helpers.

## P0.2 Ongoing release gate

For every 1.x release:

- [ ] Compare the supported Rust facade against the previous release.
- [ ] Run default, no-default, all-feature, release-mode, and MSRV matrices.
- [ ] Build and import real Python abi3 wheels.
- [ ] Build scalar, SIMD, slim, and threaded Wasm packages.
- [ ] Run JavaScript lifecycle and clean-tarball smoke tests.
- [ ] Record source-package, wheel, npm, native binary, and Wasm size deltas.
- [ ] Curate release notes so hidden research infrastructure is clearly
  distinguished from supported behavior.
- [ ] Verify crates.io, npm, and PyPI publication after the release tag.

Current release work is tracked by
[#1410](https://github.com/graydonpleasants/geo-polygonize/pull/1410).

# P1 — Production-scale evidence

## P1.1 Delivered evidence foundation

- [x] Versioned public workload manifests with license, provenance, checksum,
  profiles, and correctness class.
- [x] Deterministic 1k, 10k, and 100k production-network workloads.
- [x] An optional out-of-tree million-segment artifact.
- [x] Preserved source line-string structure and workload descriptors.
- [x] GEOS/Shapely and JTS reference paths.
- [x] Fail-closed benchmark record schemas.
- [x] Dedicated-runner publication and resumable orchestration.
- [x] Seven-entry production baseline suite.
- [x] Component-memory, allocation, peak-RSS, and workload evidence.
- [x] Durable decision records for evaluated algorithm changes.

## P1.2 Remaining evidence decisions

Tracked primarily by:

- [#1291](https://github.com/graydonpleasants/geo-polygonize/issues/1291)
  for component execution and adjacency layout;
- [#1392](https://github.com/graydonpleasants/geo-polygonize/issues/1392)
  for streamed partition routing;
- [#1391](https://github.com/graydonpleasants/geo-polygonize/issues/1391)
  for stitched-output cost and readiness.

Every decision-quality record must include:

- canonical correctness/reference status;
- p50, p95, throughput, and sample count;
- phase timings;
- allocation count and bytes;
- peak live heap and peak RSS where applicable;
- candidate visits, exact predicates, split events, and segment expansion;
- workload and artifact identity;
- architecture, OS, compiler, features, dependencies, and commit SHA;
- predeclared effect and secondary-regression thresholds.

# P2 — Arrangement and component architecture

## P2.1 Delivered arrangement foundation

- [x] Twin, adjacency, degree, source, angular-order, face-cycle, and Euler
  validation.
- [x] Persisted directed-edge `next` links.
- [x] Deterministic component-local face identities.
- [x] Component-local unbounded cycle evidence.
- [x] Explicit face-walk versus extracted-ring comparison.
- [x] Deterministic connected-component decomposition.
- [x] Component-local dangle, cut, sort, and ring processing.
- [x] Sequential and per-worker scratch reuse.
- [x] Direct single-component fast path.
- [x] Component-scaling and memory evidence.
- [x] A correctness-gated packed-CSR shadow candidate.

## P2.2 Remaining component/layout decisions

Tracked by
[#1291](https://github.com/graydonpleasants/geo-polygonize/issues/1291).

- [ ] Complete balanced, skewed, dangle/cut-heavy, and nested-ring workload
  coverage.
- [ ] Decide deterministic sequential versus parallel thresholds.
- [ ] Compare nested and packed-CSR layouts end to end across the baseline suite.
- [ ] Record compile-time, binary-size, and Wasm costs.
- [ ] Remove losing layout/dispatch prototypes.
- [ ] Retain one global containment phase after component-local graph work.

**Promotion gate:** exact canonical equivalence plus a predeclared end-to-end or
peak-memory benefit on more than one representative workload.

# P3 — Adaptive noding and candidate backends

## P3.1 Delivered candidate architecture

- [x] Broad-phase candidate enumeration separated from exact intersection and
  split accumulation.
- [x] Streaming SIMD and uniform-grid candidate paths.
- [x] One shared floating exact/split/normalize/dissolve path.
- [x] Original, synthetic, and unavailable source-chain identity.
- [x] Deterministic workload descriptors.
- [x] `geo::Intersections` retained as an exact-hit differential oracle.
- [x] MCIndex-style monotone-chain research prototype.
- [x] Golden, compatibility, provenance, Z, operational-error, serial/parallel,
  bounded-fuzz, and production-scale MCIndex evidence.
- [x] MCIndex production decision: rejected for production dispatch after
  correctness passed but matched production workloads regressed materially.

## P3.2 Remaining candidate research

- [ ] Keep the sweep implementation only while it provides differential value.
- [ ] Remove or archive research backends that no longer justify maintenance.
- [ ] Revisit new candidate algorithms only against the production baseline
  suite and shared visitor.
- [ ] Keep dispatch deterministic and inspectable.

## P3.3 Wide-work-unit SIMD

Follow
[`docs/SIMD_OPTIMIZATION.md`](docs/SIMD_OPTIMIZATION.md).

Begin only when profiling identifies a remaining exact-predicate or containment
bottleneck after candidate reduction.

- [ ] Batch independent candidate pairs rather than XY components.
- [ ] Compare query-major and pair-major AoSoA layouts.
- [ ] Use conservative wide filters with scalar robust fallback.
- [ ] Measure packing, lane use, fallback, emission, merge, memory, compile,
  native-size, and Wasm-size costs.
- [ ] Evaluate point-wide repeated containment separately.
- [ ] Require full CPU validation and topology equivalence.
- [ ] Remove losing experiments.

# P4 — Replicate-and-own tiling and input routing

Replicate-and-own tiling remains experimental and distinct from graph stitching.

## P4.1 Delivered coverage and recovery contract

- [x] Validate tile/grid options and propagate per-tile errors.
- [x] Bound tile count, assignment count, retries, fallback regions, parallelism,
  output polygons, and output coordinates.
- [x] Normalize signed-zero deduplication and merge duplicate provenance.
- [x] Emit owned-face, input-boundary, transformed-component, and
  ownership-domain evidence.
- [x] Track per-issue coverage resolution instead of one fallback Boolean.
- [x] Retry unresolved tiles with bounded larger halos.
- [x] Recover conservative envelope-closed regions.
- [x] Escalate unresolved global-containment cases to caller-enabled whole-input
  fallback.
- [x] Preserve deterministic reports, traces, decline reasons, errors, and
  canonical output.
- [x] Differential-test tile size, origin, ownership, container shape, order,
  concavity, holes, overlaps, dirty crossings, dangles, and cuts.

## P4.2 Independent local partition oracle — delivered

Completed issue:
[#1389](https://github.com/graydonpleasants/geo-polygonize/issues/1389).

Inspired by `map-tile-toolkit`'s all-tiles versus one-tile-at-a-time oracle.

- [x] Versioned deterministic local partition snapshots.
- [x] Independent reprocessing from original linework.
- [x] Source selection, noding, graph, border, face, provenance,
  representative-ID, Z, non-polygon, and normalized-error comparison.
- [x] Exhaustive bounded scans of neighboring empty partitions.
- [x] Input and tile metamorphic tests.
- [x] Dedicated bounded fuzz target.
- [x] Same-options tiled-versus-untiled comparison retained as the global gate.

Structured field-level mismatch evidence remains tracked by
[#1513](https://github.com/graydonpleasants/geo-polygonize/issues/1513).

## P4.3 Streamed partition router — decision pending

Tracked by
[#1392](https://github.com/graydonpleasants/geo-polygonize/issues/1392).

Inspired by `map-tile-toolkit`'s one-pass grid router and sink model.

Delivered:

- [x] Source-aware partition sink.
- [x] Strict same-partition inner-box proof.
- [x] Streamed boundary-near candidate scans.
- [x] Exact segment/partition predicates.
- [x] Candidate-visit limits and cancellation.
- [x] Independent assignment and local-snapshot oracle.
- [x] False-positive and work accounting.
- [x] Native and Wasm benchmark entrypoints.
- [x] Allocation and isolated peak-live-heap evidence.

Remaining:

- [ ] Publish matched dedicated-runner results across production-scale,
  long-sparse, mixed, and dense-short-segment workloads.
- [ ] Measure full tiled-call impact, not router-only time.
- [ ] Check in an accept/narrow/reject decision.
- [ ] Keep the current geometry-envelope selector authoritative until that
  decision passes.

## P4.4 Remaining replicate-and-own boundary

- [ ] Broad missing-region detection remains observational rather than certified.
- [ ] Non-envelope-closed region-local reconciliation remains unsupported.
- [ ] Replicate-and-own results must not be described as graph-stitched.
- [ ] Unsupported cases must retain explicit evidence, fallback, or typed error.
- [ ] Do not add more recovery heuristics when physical stitching is the correct
  dependency.

# P5 — Transactional partition topology and true stitching

## P5.1 Delivered physical prerequisites

- [x] Canonical signed-zero-safe partition node and edge keys.
- [x] Qualified partition/component/local-face references.
- [x] Stable atomic observation IDs and conflict rejection.
- [x] Declared adjacency with exact border coordinates and complementary sides.
- [x] Physical boundary noding for endpoints, crossings, corners, and collinear
  border edges.
- [x] One-to-many breakpoint normalization backed by physical local halfedges.
- [x] Source, representative-ID, endpoint-Z, and face-lineage preservation.
- [x] Arrangement and face-walk validation after boundary noding.
- [x] Existing replicate-and-own output left unchanged.

## P5.2 Local partition snapshots and transactional mosaic — delivered

Completed issue:
[#1390](https://github.com/graydonpleasants/geo-polygonize/issues/1390).

Inspired by `map-tile-toolkit`'s transactional `Mosaic`.

- [x] Versioned deterministic partition snapshots.
- [x] Snapshot schema and context validation.
- [x] Atomic replacement and purge.
- [x] Rejected writes preserve committed fingerprints.
- [x] Identical replacement is idempotent.
- [x] Insertion-order equivalence.
- [x] Cancellation and limits before commit.
- [x] Larger-halo retries staged through the transaction boundary.
- [x] Typed physical-span claim storage and conflict witnesses.

The current implementation is correctness-first and rebuilds more state than a
large persistent mosaic should. Delta-indexed replacement and resumability are
tracked by
[#1512](https://github.com/graydonpleasants/geo-polygonize/issues/1512).

## P5.3 Physical consistency and topology readiness — delivered

- [x] Separate physical `valid` / `incomplete` / `conflict` states.
- [x] Separate topology ready / ambiguous / incomplete / conflicting states.
- [x] Multiple face-side claims retained per physical span.
- [x] Half-open span and corner ownership/corroboration obligations.
- [x] Physical conflicts rejected transactionally.
- [x] Only physically valid, face-qualified, unambiguous claims admitted into
  downstream twin/topology planning.
- [x] Ambiguous claims remain fail-closed.
- [x] Private physical-arrangement witness with deterministic faces, Euler
  evidence, and one exterior identity.
- [x] Atomic one-to-one physical successor adoption.
- [x] Many-to-one physical alias payload validation.

## P5.4 Alias-aware face-qualified stitching — active

Tracked by
[#1391](https://github.com/graydonpleasants/geo-polygonize/issues/1391).

Canonical physical directed slots borrow every contributing local edge,
retaining distinct face-qualified claims without copying source or Z payloads.
Claim resolution admits only complete interior claims or one exact complementary
face pair backed by two topology-ready observations and complete alias coverage.
Typed witnesses distinguish absent, incompatible, and ambiguous evidence; full
observation groups are checked before eligibility filtering. The
detached twin and successor candidate uses physical slot IDs and retains every
local alias. Construction requires ready claims, complete span evidence,
reciprocal twins, node-continuous successor cycles, and the physical arrangement
proofs. Connected polygonal candidates reconcile complete local component
coverage through exact claim pairs, project every retained alias onto a physical
face cycle, and prove one exterior cycle covering every qualified local exterior
marker. Bounded local faces cannot split across global faces; clipped exterior
aliases can enter bounded cycles only with an exact paired bounded corroborator.
Uncorroborated node-only joins and disconnected/containment-dependent arrangements
remain blocked. Private physical-table extraction retains canonical simple ring
XYZ, ordered representative IDs, complete source sets, every owned local alias,
and the complete retained dangle/cut/invalid-ring payload ledger. Stale tables,
ambiguous or missing payload evidence, self-touching polygon face cycles, and nested/containment
cases remain blocked. Global non-polygon ownership proofs bind the complete
original-input source and noded-edge inventories to every current snapshot,
global classification, one whole-payload half-open core owner, and every local
edge alias. Equal invalid-ring records retain their exact global multiplicity
and local record indices; ownership proof does not bypass the existing
duplicate-payload extraction gate. The whole-core checker declines cross-core payloads, incompatible noding,
missing inventories, extra copies, and conflicting owners.
Private assembly witnesses now cover open dangle/cut edges and unambiguous
closed invalid-ring chains crossing shared boundaries between adjacent cores.
The complete input/classification pass is shared with whole-core ownership;
replayed boundary tables retain every raw payload copy and assign each atomic
span one owner backed by its exact local
aliases. Coarser local rows must replay into the same atomic spans, retaining
their aliases on every descendant. An exact endpoint chain visits all spans;
each owner transition requires reciprocal shared-side declarations. Stale or
missing tables, halo-only claimants, corner-only transitions, and branched ring
chains remain blocked. Ring spans retain their original payload-edge positions;
the ordered chain must close through every span and validate its final owner
transition. Opposite invalid-ring copies retain their global multiplicity and
every local record index. The raw duplicate gate is unchanged and these witnesses
do not select or normalize non-polygon output.
The witness also retains accompanying whole-core dangle/cut/invalid-ring
payloads through the same owner checker used by the whole-core proof, including
their complete source/edge aliases, local record indices and global multiplicity.
One invalid member rejects the entire mixed witness; no subset is emitted.
The private global non-polygon consumer replays the shared proof validators and
rechecks current snapshot, tile and domain bindings before decoding any family.
It retains every raw local record and complete proof aliases alongside the
globally classified multiplicity; it does not choose a representative local
copy. Selected XYZ is decoded without another Z-policy pass. Every materialized
family must match the untiled result from the same prepared input/options call,
using the same comparison as full-output conformance. Changed proofs, missing
records, closure/ownership conflicts, family mismatches, limits or cancellation
reject the complete consumer. Its family equivalence remains separate from full
arrangement equivalence and selectable output. Current raw duplicate and
physical-face extraction gates remain unchanged. The complete private candidate
revalidates either the whole-core raw extraction or separate physical polygon
evidence paired with a complete cross-core consumer. Both replay the same
physical claim, face/Euler and simple-shell containment checks and bind the
local face graphs and options to the current snapshots. The cross-core path
also binds every raw non-polygon row and its exact extraction counts to those
snapshots before joining globally classified families. Raw duplicates still
fail the original extraction gate; every local alias remains in the consumer
and source ledger. No representative raw copy is selected or discarded.
The adapter retains polygon provenance, representative IDs and selected XYZ;
determinism applies after the join, and aggregate limits apply before filtering
or copying joined families. Full-output conformance gates the complete private
candidate against the same prepared reference: any polygon or non-polygon
mismatch rejects it as a whole, retaining checked/ready/mismatch diagnostics.
Trace distinguishes a materialized candidate from one accepted after comparison;
missing reference evidence cannot accept a candidate. Physical-ready
multi-boundary dangles with whole-core companions now conform privately.
Cross-core cuts, invalid rings and family mixtures also conform privately when
the remaining polygon components have unique owners strictly inside core bounds.
Cut deletion can leave disconnected cycles under one native component ID; split
only complete reciprocal twin/successor groups, preserving every original ID.
Every retained polygon copy must independently pass the existing connected
physical claim, face/Euler and simple-shell extraction proofs, and every copy
must agree on ring XYZ, representative IDs and source sets. Retain all copies
and alias mappings alongside the explicit owner. Bind excluded non-polygon
components to the consumer's complete atomic/coarse segment lineage and cover
the entire prepared source inventory. Every polygon segment must retain a
physical copy in every partition recorded by its prepared aliases; deleting a
halo copy cannot be hidden by updating snapshot graphs. Mixed or unknown
components reject the whole proof. Global interaction checks reject nested or
touching polygon
components and overlapping polygon/invalid-ring envelopes. Border observations
are bound to current snapshots and prepared native-edge/non-polygon segment
lineage, including source IDs, representative IDs and endpoint Z. Rebuild span
groups on replay, require exact pairs, and reject stale caches; observations
are never filtered to make a component eligible. Original ambiguity, raw
duplicate and extraction gates remain unchanged.
Each observation must lie along its claimed finite core side, and each pair
must have reciprocal declarations between actually adjacent cores. Export and
proof validation share the same side-membership predicate, so source-correct
halo aliases cannot invent a border relationship.
The family conformance matrix covers vertical layouts, negative coordinates,
reflection, a `(1e6, -1e6)` translation and fragmented input linework. It checks
core owners, retained halo copies and every output family; nesting, contact,
boundary ambiguity and invalid-ring interactions remain rejected under the same
coordinate changes. This coverage does not enable another input class or output
selection.
On identity coordinates, the rejection matrix also checks all three touch
policies with and without a filter that removes every polygon. These output
options cannot admit an ambiguous physical proof or private candidate; ordinary
and traced calls still have no stitched output.
The complete family mixture also conforms across provenance/ID settings, both
tiled dedup policies, polygonal mode, the exact face-area threshold and its next
representable value, and conflict-rejecting Z policy on zero-Z inputs. These
checks preserve invalid-ring multiplicity and raw-duplicate rejection. Proofs
from a different options context remain rejected even when observable output
is identical.
Exact `0.125`-grid cut fixtures also conform privately with validated floating
noding, fixed Grid/GeosCompat noding and certified Grid noding under input
reversal and two core sizes. Precision changes still reject stale proofs when
their observable output would be identical.
Grouped and nested linework containers preserve exact selected top-level input
indices and per-input source segment ordinals, including empty members and a
selected point that adds no linework. The complete family fixture retains every
halo polygon copy under input reversal and two core sizes.
Partial-halo fixtures preserve exact alias partition sets and owner-copy
indices where halo bounds first touch an input envelope, from singleton owners
to asymmetric and full copy sets. Each snapshot retains its exact selected
input and source inventory; every prepared copy remains bound.
In the cut fixture, partial endpoint visibility preserves local dangles and
rejects the private consumer and candidate. Fast and noded paths conform only
after all three copies observe both endpoint polygons; raw duplicates still
block public selection.
Polygon and MultiPolygon containers, including nested collections, retain all
source rings. Simple disjoint shells conform privately; fixtures with holes
retain the reference's hole and inner face while private containment assembly
remains blocked.
Border-touching polygon components, branched rings and containment-dependent
classes remain blocked. The bridge does not enable output selection or certify
broader input classes.
Shell/hole assembly where needed and the remaining promotion gates still apply.
The local output adapter preserves selected XYZ and ordered representative IDs,
applies the current provenance/profile and face-area options, and compares every
output family against untiled output from the same call and options. Comparison
canonicalizes coordinates and IDs together and rejects missing or mismatched
IDs. The private adapter requires a complete ownership proof bound to the current
snapshot and payload ledger. Ownership and conformance evidence are traced
separately from selectable output; they do not complete the promotion gates.

- [x] Model canonical physical slots with one or more local directed-edge
  aliases.
- [x] Retain complete local twin, successor, face, component, source,
  representative-ID, and raw/selected Z lineage per alias.
- [x] Resolve complementary face-qualified claims independently from physical
  edge deduplication.
- [x] Preserve ambiguous four-claim and oversubscribed groups as explicit
  fail-closed evidence.
- [x] Build alias-aware global twin and successor candidates.
- [x] Reconcile global components and one global unbounded face for connected
  polygonal candidates.
- [x] Materialize private physical-table ring and alias payloads with complete
  source, representative-ID, raw/selected Z, and retained non-polygon evidence.
- [x] Adapt eligible simple-ring physical payloads privately, preserving selected
  XYZ, representative IDs, provenance/profile and face-area options, and compare
  every output family against the same-call, same-options reference (#1541, merged).
- [x] Prove complete whole-core dangle/cut/invalid-ring ownership against current
  input and snapshot inventories, retaining all aliases and exact multiplicity;
  preflight cancellation-enabled source work before extraction (#1542, merged).
- [x] Construct private cross-core witnesses for open payload edges and unbranched
  closed invalid-ring chains, including mixed whole-core companions, complete
  atomic/coarse lineage and every owner transition; reject the complete witness
  on a bad member (#1543, merged).
- [x] Consume revalidated complete ownership or cross-core proofs privately,
  retaining the raw ledger and exact multiplicity, decoding selected XYZ, and
  requiring equivalence of all three non-polygon families (#1544, merged).
- [x] Join current physical polygon evidence with a complete revalidated consumer
  into a private candidate, preserving snapshot/options/source bindings and all
  existing gate outcomes. Accept only after full same-call conformance; missing
  reference evidence or any family mismatch rejects the complete candidate while
  trace distinguishes materialization from acceptance (#1545, merged).
- [x] Extend family physical-component proofs with finite core-side and actual
  neighbor checks (#1547, merged), then retain source/noded affinity across
  coordinate representations (#1548, merged), options (#1549, merged), and
  touch/filter ambiguity rejection (#1550, merged). These remain private proofs.
- [x] Bind a private physical alias view to shared original-input preparation,
  retaining every active native alias and raw family/record index independently
  of global family classification. The 2×2 view retains 32 aliases, 24 slots,
  and 16 raw dangles; ambiguity, family, adoption and selection gates stay closed.
- [x] Prove private native-twin coverage of interior seams for core-contained
  simple native cycles on conforming rectangular cores. Replay native rotation,
  cycle orientation, finite actual neighbors and complete native/observation
  membership. The 2×2 evidence retains four seams and all 32 observations while
  eight domain segments remain explicit obligations for the next authority gate.
- [x] Prove private domain/exterior authority for those eight segments from
  complete native twin pairs and observations. Native successors plus four
  checked seam handoffs form one complete domain-perimeter exterior cycle,
  covering all four qualified exterior markers without consulting witness face
  IDs or successors. Every alias/raw record remains retained; claim readiness,
  family classification and selection stay closed.
- [x] Project all 32 native aliases onto five private global face cycles derived
  from native bounded walks and domain authority, retaining original native
  links and complete seam receipts. Eight local exterior aliases project to
  neighboring bounded faces and eight to the global exterior; each qualified
  local exterior face spans three global faces. Witness links/labels only
  corroborate the projection; claims, acceptance and output gates stay closed.
- [x] Prove complete active component coverage, global angular rotation and
  planar Euler authority for the connected flat cell arrangement: four local
  components, one physical component, 9 vertices, 12 edges and 5 faces. Validate
  the four disjoint bounded cell interiors within the exterior cycle envelope;
  nested, holed and disconnected classes remain unsupported. Claim, raw-family,
  acceptance and public-selection gates remain unchanged.
- [x] Bind complete original-input active incidence for all twelve prepared
  spans, retaining all sixteen raw dangle records and both incident global
  faces. Reference-family contents supply no incidence authority. An original
  raw-only span fails this cover even with an empty reference family; raw
  classification and claim-consumption gates remain closed.
- [x] Complement active incidence with private authority for isolated original
  dangles: both endpoints have degree one in the complete original noded graph,
  one strict core owns the span, and one exact two-point raw record retains its
  original source row and ordered walk. Mixed active/raw-only coverage keeps
  every raw record without reference-family authority. Chains, ambiguous
  ownership, cuts and invalid rings remain unsupported by this certificate;
  existing family, claim, acceptance and output gates remain unchanged.
- [x] Bind the complete original graph observation index and edge buckets to
  every exact snapshot/native boundary observation before role selection.
  Retain all 32 observations, full source/Z/qualified-face/successor payloads,
  native aliases and raw records; reject orphan buckets, duplicate identities
  and payload mismatches without normalizing them away. Claim consumption
  remains a separate, closed obligation.
- [x] Prove additive typed roles for all eight whole domain pairs and four
  whole seam quartets from exact inventory, actual core/domain ownership,
  both native twin pairs and per-alias projected physical sides. Retain every
  observation, original native link/source/Z and raw record. Revalidate the
  borrowed inventories and projection; legacy claims, family classification,
  acceptance and output consumers remain unchanged.
- [x] Prove complete original-source coverage for detached non-branching leaf
  paths strictly inside one core, including the two-edge frame-chain case.
  Check the full original planar inventory, every degree-one removal and exact
  source/ownership/Z/ordered raw-record incidence without reference-family
  authority. Keep all active/raw records; branches, attachments, cross-core or
  halo copies and multi-edge/cut/invalid raw records remain unsupported here.
- [x] Prove every zero-observation local interior active span from a unique
  actual core, both complete native bounded twins/successors, exact source/Z
  and two projected bounded faces. Retain all observed aliases and original
  raw records separately; raw-origin partitions do not determine active owners.
  This private complement never infers a role from an empty observation count
  or promotes claims, raw-family classification, consumers or output selection.
- [x] Consume complete observed and local-interior roles into a separate private
  active-topology table with every alias/observation, original native lineage,
  checked physical links and a cycle-validated derived/witness face bijection.
  Retain the entire observational raw ledger without assigning output families;
  complete original raw disposition still gates a complete candidate/output join.
  Legacy claims, classifiers, candidates and final selection remain unchanged.
- [x] Account independently for every original raw record in the strict-core
  detached-path class, retaining its original kind/order/origin while linking
  residual fragments to checked active topology. A separate original-source
  inventory derives true dangles from complete leaf removals; checked residual
  rotation/distinct cycles/valid rings exclude cuts and invalid rings here.
  The mixed chain retains18 observational records and2 original dangles;
  unsupported remainders and all candidate/output promotion gates stay closed.
- [ ] Prove global raw-family roles and a face-qualified claim rule accounting
  for every two-observation domain group and four-observation seam group,
  without filtering observation-count ambiguity or discarding raw records.
- [ ] Complete representation review and remaining cross-core input-class
  proofs before promotion; family ownership does not replace polygon, Euler
  or containment evidence.
- [ ] Extract polygons, holes, dangles, cuts, and invalid rings with complete
  provenance, representative IDs, and Z.
- [ ] Pass the 2×2 workload under input/tile permutation and reversal.
- [ ] Pass arrangement, Euler, containment, limits, cancellation, trace, and
  same-options untiled-equivalence gates.
- [ ] Publish end-to-end performance and peak-memory evidence.

Version 1.2.0 is published from `b00a20be`. The earlier native stack #1529
(#1527 → #1528 → #1530 → #1534 → #1537) has merged into main and records
post-release private alias-aware extraction progress. Its aggregate non-polygon
ledger guard is retained. Native stack #1546 (#1541 → #1542 → #1543 → #1544
→ #1545) has also merged into main:
private output adapter → whole-core ownership → combined cross-core witnesses
→ proof consumer → complete private candidate with its acceptance gate.
Native stack #1551 (#1547 → #1548 → #1549 → #1550) has also merged into main:
family physical-component proof → coordinate/source affinity → options affinity
→ touch/filter ambiguity rejection.
Native stack #1556 (#1552 → #1553 → #1554 → #1555) has also merged into main:
fixed precision → container inventories → partial halos → the input-bound
physical alias view before family classification. Native stack #1560
(#1557 → #1558 → #1559) has also merged into main: complete native seam
coverage → domain/exterior authority → per-alias global face projection.
Native stack #1566 (#1563 → #1564 → #1565) has also merged into main:
component authority → original active incidence → isolated dangle complements.
Native stack #1570 (#1567 → #1568 → #1569) has also merged into main:
original observation inventory → whole domain/seam roles → original leaf paths.
Component, Euler, containment and output-promotion gates remain independent.
The 2×2 promotion gate is incomplete.
These private milestones do not change the published 1.2.0 artifacts.

**Promotion gate:** stitched output may become selectable only for a documented
input/options class when the same call produces ready stitched output, checks
same-options untiled equivalence, reports zero mismatches, and passes resource,
trace, fuzz, provenance, representative-ID, Z, and memory gates.

# P6 — Partition representation and resumable scale

## P6.1 Delta-indexed mosaic transactions

Tracked by
[#1512](https://github.com/graydonpleasants/geo-polygonize/issues/1512).

- [ ] Add reverse partition-to-span/obligation indexes.
- [ ] Stage only affected buckets during replacement and purge.
- [ ] Preserve the exact all-or-nothing fingerprint contract.
- [ ] Define snapshot schema naming, migration, and compatibility.
- [ ] Add bounded resumable manifests with checksums and library/options
  identity.
- [ ] Compare replacement cost across 2, 16, 128, and 1,024 partitions.
- [ ] Remove losing transaction prototypes.

## P6.2 Flat snapshots and checked integer fixed-grid space

Tracked by
[#1393](https://github.com/graydonpleasants/geo-polygonize/issues/1393).

Experiment A — flat immutable snapshots:

- [ ] Contiguous arenas and checked offsets for coordinates, edges, successors,
  faces, source/Z payloads, observations, aliases, and non-polygon output.
- [ ] Borrowed views tied to committed snapshot lifetimes.
- [ ] Exact fingerprint, transaction, cancellation, limit, and topology
  equivalence.
- [ ] Snapshot-size, allocation, access, serialization, native, and Wasm
  measurements.
- [ ] Accept/narrow/reject decision.

Experiment B — checked integer partition space for explicit fixed precision:

- [ ] Explicit origin/scale and checked lattice conversion.
- [ ] Euclidean division for negative partition coordinates.
- [ ] Exact local/global round trips over a documented domain.
- [ ] Overflow rejection before topology work.
- [ ] `i64` fast predicates with `i128` fallback where required.
- [ ] Floating mode remains bit-for-bit unchanged.
- [ ] Accept/narrow/reject decision.

## P6.3 Streaming and out-of-core execution

Begin only after #1512 establishes resumable snapshot compatibility and #1391
establishes exact physical stitching for a documented class.

- [ ] Stream Arrow `RecordBatch`, GeoParquet row groups, and FlatGeobuf features
  with bounded memory and backpressure.
- [ ] Preserve source IDs, profiles, and source-chain identity across chunks.
- [ ] Separate I/O partitioning from topology partitioning.
- [ ] Persist resumable manifests with checksums, options, partition state, and
  library version.
- [ ] Evaluate disk-backed or memory-mapped indexes only after profiling.
- [ ] Measure I/O, peak RSS, temporary storage, recovery, and output
  equivalence.

# P7 — Later topology capabilities

These remain evidence-gated and should not compete with the active partition
critical path.

| Issue | Scope | Required predecessors |
|---|---|---|
| [#720](https://github.com/graydonpleasants/geo-polygonize/issues/720) | Graph-native Boolean overlay with winding-labeled faces. | Stable global arrangement identity, provenance algebra, robust noding. |
| [#714](https://github.com/graydonpleasants/geo-polygonize/issues/714) | Topology-preserving shared-edge simplification. | Stable shared-edge identity and arrangement validator. |
| [#688](https://github.com/graydonpleasants/geo-polygonize/issues/688) | Robust buffering through offset curves, certified noding, and face selection. | Arrangement face selection and dedicated corpus. |
| [#697](https://github.com/graydonpleasants/geo-polygonize/issues/697) | MVT and TopoJSON adapters with topology-preserving quantization. | Stable simplification and real consumers. |
| [#663](https://github.com/graydonpleasants/geo-polygonize/issues/663) | Separate incremental arrangement API with local invalidation and delta reports. | Global arrangement identities and mutation invariants. |
| [#769](https://github.com/graydonpleasants/geo-polygonize/issues/769) | Separate spherical/ellipsoidal polygonization kernel. | Written geodesic contract and spherical predicate corpus. |
| [#664](https://github.com/graydonpleasants/geo-polygonize/issues/664) | Consumer-driven DuckDB/PostGIS adapters. | Stable streaming contracts and deployment benchmarks. |
| [#771](https://github.com/graydonpleasants/geo-polygonize/issues/771) | GPU broad-phase or batch-predicate research with CPU validation. | Production workloads and a measured CPU bottleneck. |

# Recommended aggressive execution order

Use stacked PRs so dependency-ready work continues while parents are reviewed.

## Stack A — alias-aware stitched topology

```text
#1391 canonical physical slots with local aliases
└── face-qualified alias resolution
    └── alias-aware successor/twin candidate
        └── 2×2 global arrangement and extraction
            └── untiled equivalence and cost evidence
```

## Stack B — partition-router decision

```text
#1392 matched dedicated publications
└── end-to-end tiled-call comparison
    └── accept / narrow / reject decision
```

## Stack C — scalable mosaic state

```text
#1512 reverse indexes and delta transactions
└── snapshot schema/migration contract
    └── resumable manifests
        └── #1393 flat snapshot experiment
            └── checked integer fixed-grid experiment
```

## Stack D — observability

```text
#1513 typed field-level partition differences
└── fuzz reproducer serialization
    └── interactive-debugger navigation
```

The release stack may proceed independently, but no release note should imply
that private router or stitched-output research is supported or selected by
default.

# Stacked PR rules

- Root each independent stack at current `main`.
- Root every child at its immediate parent.
- Open a draft PR as soon as a slice is coherent.
- Continue on dependency-ready children without waiting for parent review.
- Keep each relative diff independently reviewable.
- Rebase descendants when a parent changes; do not merge parent branches into
  children.
- After a parent merges, rebase the next child onto `main` and retarget it.
- Do not enable automerge on PRs whose base is not `main`.
- Include delivered scope, contract impact, validation, remaining work, and the
  exact next branch in every PR body.
- Keep no more than five open PRs in one stack.

# Promotion and claim gates

## Stable facade

A 1.x release is complete only when:

- source versions agree;
- semver comparison passes;
- public registry artifacts exist;
- real registry installs pass;
- supported API conformance passes;
- migration and support metadata are current.

## Streamed partition router

The router may replace or supplement the current selector only when:

- #1389 assignment and local snapshot equivalence pass;
- no candidate-count-proportional temporary storage exists;
- limits and cancellation account the physical scan;
- production-scale end-to-end evidence exceeds predeclared thresholds;
- dense/adverse workloads stay within the regression budget;
- the internal rule is deterministic and inspectable;
- a durable accept/narrow/reject decision exists.

## Tiled graph stitching

Stitched output is selectable only when:

- local arrangements are physically boundary-noded;
- transactional physical claims reconcile deterministically;
- aliases preserve every local face-qualified claim;
- global twins, successors, components, faces, and one unbounded face validate;
- full canonical output equals same-options untiled execution for the documented
  class;
- unsupported cases have explicit evidence, fallback, or typed error behavior;
- resource, cancellation, trace, fuzz, provenance, representative-ID, Z, and
  memory gates pass.

## Representation changes

A flat, CSR, integer, or resumable representation may enter production only
when:

- the current and candidate paths pass the same oracle and topology gates;
- conversion and offset arithmetic is checked;
- floating semantics remain unchanged unless the caller selected fixed
  precision;
- end-to-end memory or time improvements exceed predeclared thresholds;
- compile-time, binary-size, and Wasm costs are recorded;
- losing representations are removed.

# Research references

Core topology:

- [JTS `SnapRoundingNoder`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/noding/snapround/SnapRoundingNoder.html)
- [JTS `ValidatingNoder`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/noding/ValidatingNoder.html)
- [JTS `FastNodingValidator`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/noding/FastNodingValidator.html)
- [JTS `MCIndexNoder`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/noding/MCIndexNoder.html)
- [JTS `OverlayNGRobust`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/operation/overlayng/OverlayNGRobust.html)
- [`geo::algorithm::sweep::Intersections`](https://docs.rs/geo/latest/geo/algorithm/sweep/struct.Intersections.html)
- [GEOS `UnaryUnionOp`](https://libgeos.org/doxygen/classgeos_1_1operation_1_1geounion_1_1UnaryUnionOp.html)
- [CGAL 2D Arrangements and DCEL](https://doc.cgal.org/latest/Arrangement_on_surface_2/index.html)
- [CGAL arrangements with history](https://doc.cgal.org/latest/Arrangement_on_surface_2/classCGAL_1_1Arrangement__with__history__2.html)

Partition and tiling inspiration:

- [`nyurik/map-tile-toolkit`](https://github.com/nyurik/map-tile-toolkit)
- [`map-tile-toolkit` slicing-equivalence fuzzer](https://github.com/nyurik/map-tile-toolkit/blob/main/fuzz/fuzz_targets/slice_equivalence.rs)
- [`map-tile-toolkit` transactional `Mosaic`](https://github.com/nyurik/map-tile-toolkit/blob/main/src/mosaic.rs)
- [`map-tile-toolkit` streamed grid router](https://github.com/nyurik/map-tile-toolkit/blob/main/src/grid.rs)
- [`map-tile-toolkit` flat tile storage](https://github.com/nyurik/map-tile-toolkit/blob/main/src/slicer.rs)
- [`map-tile-toolkit` exact integer predicates](https://github.com/nyurik/map-tile-toolkit/blob/main/src/geom.rs)

Optimization:

- [Box2D: SIMD for Collision](https://box2d.org/posts/2026/07/simd-for-collision/)
- [`docs/SIMD_OPTIMIZATION.md`](docs/SIMD_OPTIMIZATION.md)

# Invariants for all future work

- Keep stable behavior expressible through the canonical semantic options schema.
- Keep execution budgets and cancellation separate from semantic options.
- Preserve deterministic canonical output and structured, actionable errors.
- Preserve complete source provenance and representative identity through
  noding, dissolve, graph decomposition, tiling, stitching, and future topology
  operations.
- Treat replicate-and-own tiling, physical mosaic consistency, and true graph
  stitching as different algorithms with different contracts.
- Do not claim robustness beyond the selected noding policy's checked
  postconditions.
- Do not silently fall back to another precision or guarantee.
- Do not accept a performance win before its correctness gate passes.
- Do not add a stable API path without equivalent cross-binding conformance
  where applicable.
- Ensure every hard failure can emit a bounded witness or reproducible trace.
- Add the smallest strict regression that would have caught each bug.
