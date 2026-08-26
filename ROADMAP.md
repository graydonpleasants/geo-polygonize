# Engineering Roadmap

This is the active, evidence-gated roadmap for `geo-polygonize` after the
`1.0.0` release on August 3, 2026.

The stable 1.x facade is production-supported. Graph, noding, trace, differential,
tiling, and other research surfaces remain intentionally unstable even where
they are compiler-public and marked `#[doc(hidden)]`.

Git history preserves the detailed pre-1.0 delivery record. This document now
focuses on the remaining work that can materially improve correctness,
operability, ecosystem trust, representative performance, and topology scale.

## Current state

`1.0.0` established the supported baseline:

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
- a private DCEL-like arrangement with persisted `next` links, deterministic
  face identities, component-local processing, and arrangement invariants;
- experimental replicate-and-own tiling with bounded work, explicit coverage
  evidence, deterministic retries, conservative region fallback, and
  whole-input fallback;
- correctness-gated benchmark schemas, publication policy, and dedicated-runner
  workflow.

The remaining SOTA work is no longer basic polygonization. It is:

1. proving that releases are actually available and usable from every registry;
2. producing decision-quality evidence on production-scale linework;
3. promoting or rejecting adaptive candidate backends from that evidence;
4. completing physical cross-partition graph stitching;
5. making the post-1.0 support and unstable-API boundaries durable.

## North star

Make `geo-polygonize` the best-supported pure-Rust planar
linework-to-polygons kernel across native Rust, Python, WebAssembly, and Arrow:

- certified correctness when the selected precision policy promises it;
- deterministic, explainable output with complete source provenance;
- one semantic contract across equivalent bindings;
- bounded and cancellable execution on difficult or untrusted inputs;
- production-scale performance claims backed by public, reproducible evidence;
- an explicit stable facade and an equally explicit research boundary;
- scalable partitioned topology without weakening global face semantics.

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
- Never return partial topology unless a future API models partial/resumable
  computation explicitly.
- Preserve deterministic canonical output, provenance, Z behavior, diagnostics,
  and normalized errors through every optimization.
- Feed experimental candidate generators through the shared exact/split/dissolve
  path and independent validator.
- Do not expose a new backend or dispatch rule until its promotion gate passes.
- Do not publish performance claims before correctness gates pass.
- Treat hosted/shared-runner timings as diagnostic, not decision-quality.
- Keep replicate-and-own tiling separate from true graph stitching.
- Do not independently finalize disconnected graph components when global
  containment can nest them.
- Keep research surfaces unsupported until their contracts are complete.
- Record rejected experiments and remove losing implementations.
- Prefer stacked, independently reviewable PRs over one large branch.

## Dependency map

The current critical path is:

```text
Release integrity
#1285
  ↓
Post-1.0 support and API governance
#1286

Production-scale workloads and baselines
#1290
  ├──→ component memory/layout decision #1291
  └──→ MCIndex production decision #1287

Physical partition-boundary noding
#1288
  ↓
Validated stitched arrangement and untiled equivalence
#1289

Independent bulk-versus-single-partition oracle
#1389
  ├──→ streamed source-segment routing #1392
  └──→ transactional partition snapshots and mosaic #1390
          ├──→ physical-edge consistency versus topology readiness #1391
          └──→ flat snapshots and checked integer partition space #1393
```

Work may proceed in parallel across these tracks, but promotion decisions must
respect the dependencies above. The partition-mosaic research is inspired by
`nyurik/map-tile-toolkit`; it is not a runtime dependency and its polyline
edge-set guarantees are not a substitute for global arrangement, containment,
provenance, Z, and untiled-equivalence proof.

# P0 — Release integrity and 1.x governance

`1.0.0` shipped the supported facade. The immediate priority is making release
completion externally verifiable and the 1.x support contract precise.

## P0.1 Cross-registry release verification

Tracked by
[#1285](https://github.com/graydonpleasants/geo-polygonize/issues/1285).

- [x] Verify the actual `1.0.0` state on crates.io, npm, and PyPI.
- [x] Repair any missing or failed publication before publishing another version.
- [x] Install exact released artifacts from each public registry.
- [x] Run canonical success and normalized-error smoke tests against those
  registry artifacts.
- [x] Add a bounded tag-triggered post-publication verifier.
- [x] Emit a machine-readable publication report by package, registry, version,
  platform, and result.
- [x] Document publication repair and rerun procedures.
- [x] Prevent a later release from being treated as complete while the previous
  release report is incomplete.

Draft release PR
[#1283](https://github.com/graydonpleasants/geo-polygonize/pull/1283)
must remain unpublished until this gate is closed.

## P0.2 Post-1.0 support and metadata

Tracked by
[#1286](https://github.com/graydonpleasants/geo-polygonize/issues/1286).

- [x] Remove stale “before 1.0” language from user-facing docs.
- [x] Archive the achieved 1.0 gates and publish a concise 0.x → 1.0 migration
  guide.
- [x] Update crate-level rustdoc to distinguish unchecked iterative noding from
  certified hot-pixel fixed precision.
- [x] Declare and continuously test an exact MSRV.
- [x] Align Rust, npm, and Python licensing, author, classifiers, descriptions,
  and support metadata.
- [x] Remove the PyPy classifier or add a real supported import/call gate.
- [x] Add a supported-facade API snapshot or allowlist gate.
- [x] Define how compiler-public `#[doc(hidden)]` research APIs are governed
  during 1.x.
- [x] Document a migration path toward a genuinely isolated research surface.
- [x] Decide how internal-only features affect release-please semver.

The exact MSRV gate is x86-sensitive: AVX-512 multiversion variants remain
intentionally disabled because Rust 1.87 rejects those target features as
unstable. Revisit them only with a documented minor-release MSRV change.

`#[doc(hidden)]` is a documentation boundary, not Rust privacy. Do not silently
stabilize those modules, but do not break experimental users in a patch release
without a migration policy.

# P1 — Production-scale evidence

The benchmark and correctness infrastructure is mature. The missing input is
representative scale and structure.

## P1.1 Public corpus foundation

Delivered:

- [x] Small redistributable fixtures for already-noded coverage, networks,
  dirty CAD-like linework, hydrographic boundaries, sparse chains, dense
  crossings, overlaps, nested rings, extreme scale, and Z policies.
- [x] Versioned workload manifests with license, provenance, checksum,
  compatibility class, and permitted profiles.
- [x] A source-pinned California OpenStreetMap production input manifest.
- [x] Equivalent already-noded, floating, and certified-fixed benchmark lanes.
- [x] GEOS/Shapely and JTS reference pipelines.
- [x] Decision-quality publication policy and durable decision records.

## P1.2 Materialize production-scale workloads

Tracked by
[#1290](https://github.com/graydonpleasants/geo-polygonize/issues/1290).

- [x] Acquire and verify the pinned OSM source out of tree.
- [x] Pin converter versions, options, derivation rules, and output checksums.
- [x] Produce deterministic workloads around 1k, 10k, and 100k segments.
- [x] Provide an optional/downloaded 1m-scale workload or document a concrete
  blocker: an out-of-tree 1,000,009-segment artifact was generated, but no
  dedicated runner/reference gate is provisioned for a decision-quality 1m run.
- [x] Preserve real source line-string structure.
- [x] Record chain lengths, component distribution, occupancy, candidate/split
  density, and exact-duplicate incidence; collinear-overlap measurement remains
  an explicit deferred field.
- [x] Add representative coverage, network, hydrographic, and
  CAD-shaped workload coverage, using existing public/procedural fixtures where
  a new source was not authorized.
- [x] Use only authorized, sanitized, or procedurally abstracted CFB data.
- [x] Correctness-gate every materialized workload before timing.
- [x] Define a fail-closed production baseline suite covering the current
  already-noded, floating, certified-fixed, component-local, and 1k/10k/100k
  production-network paths.
- [x] Publish dedicated-runner baselines for current production algorithms.

The publication workflow now accepts an operator-staged out-of-tree
`runner-manifest-v1.json` and verifies each selected clip SHA-256 in the GEOS,
Rust, and JTS reference paths. This evidence plumbing is complete, but the
suite contract now fails closed on missing, duplicate, mixed-environment, or
under-sized publications, and requires component-memory evidence. The checkbox
closed with the first complete seven-entry decision-quality suite in
[run 32447893165](https://github.com/graydonpleasants/geo-polygonize/actions/runs/32447893165)
for commit `8338213030347f086fe44baebf48779fcc448d9c`. The retained report
includes corrected component-memory evidence for every publication. Future
component-layout and candidate-backend decisions must use this suite as their
baseline.

No adaptive backend or graph-layout promotion decision may rely only on the
tiny correctness corpus.

## P1.3 Benchmark evidence requirements

Every decision-quality record must include:

- canonical correctness/reference status;
- p50, p95, throughput, and sample count;
- phase timings;
- allocation count and bytes;
- peak RSS;
- candidate pairs, exact predicates, split events, and segment expansion;
- input/output sizes and workload descriptors;
- selected workload artifact identity, including its manifest clip SHA-256;
- architecture, OS, compiler, features, dependencies, and commit SHA;
- predeclared minimum effect size and secondary regression budget.

Rejected experiments must remain linked to their evidence so they are not
repeated.

# P2 — Arrangement and component architecture

## P2.1 Delivered arrangement foundation

- [x] Validate twin symmetry, adjacency, degrees, source sets, angular ordering,
  face cycles, and Euler invariants.
- [x] Persist directed-edge `next` links.
- [x] Assign deterministic component-local face identities.
- [x] Identify component-local unbounded face cycles.
- [x] Compare explicit face walks with final extracted rings across the golden
  corpus.
- [x] Retain qualified component/face identity for partition evidence.
- [x] Decompose active graph components deterministically.
- [x] Process component-local dangles, cuts, sorting, and ring extraction.
- [x] Merge component results in deterministic global order.
- [x] Reuse sequential and per-Rayon-worker scratch.
- [x] Add component-scaling benchmark coverage.

The arrangement remains private. Component-local unbounded faces and IDs are not
global topology identities.

## P2.2 Component memory and adjacency decision

Tracked by
[#1291](https://github.com/graydonpleasants/geo-polygonize/issues/1291).

- [x] Measure peak RSS, allocations, partition vectors, scratch high-water
  marks, output buffering, and worker multiplication.
- [ ] Cover one connected graph, balanced components, skewed components,
  dangle/cut-heavy components, and nested disconnected rings.
- [x] Evaluate a direct single-component fast path.
- [ ] Evaluate deterministic sequential versus parallel component thresholds.
- [x] Prototype flat/CSR adjacency privately.
- [x] Emit deterministic component distribution, adjacency-capacity, reusable
  scratch high-water, scratch-state/worker, and merged-output evidence in
  diagnostics and benchmark records.
- [x] Run arrangement and full topology conformance before timing.
- [ ] Compare layout and execution decisions on production-scale workloads.
- [x] Check in explicit decision records for evaluated paths.
- [ ] Remove losing prototypes.

**Promotion gate:** exact canonical equivalence plus a predeclared end-to-end or
peak-memory win on more than one representative workload.

The evidence plumbing and the first dedicated-runner baseline suite are
complete. The direct single-component path is retained as research because its
primary timing and allocation wins are clear, but one peak-RSS result exceeds
the strict secondary regression budget; see
`benchmarks/decisions/component-single-component-v1.json`. The sequential /
parallel threshold and real-workload flat/CSR layout comparisons remain open.
The private CSR shadow validates packed adjacency offsets, component discovery,
and face-walk successors against the current layout, but it is not integrated
into production; use the seven-entry #1290 suite as the common baseline for
the timing decision. Production component-memory records now also carry the
current row/inner capacities and an equivalent packed-CSR offset/edge word
estimate; this is memory evidence only, not a timing or dispatch decision.
The benchmark record now also carries a correctness-gated, shadow-only packed
CSR traversal timing on the same graph snapshot; the end-to-end layout and
execution comparison remains open until that candidate is measured as part of
the seven-workload publication and compared against the promotion gate.

## P2.3 Remaining arrangement payload work

- [ ] Retain a direct private mapping from every face to its complete boundary
  source set and final Z decisions.
- [ ] Keep local-to-global identity transitions explicit during stitching.
- [ ] Preserve one global containment phase after component-local graph work.
- [ ] Do not expose a public DCEL until overlay-quality invariants and mutation
  semantics are proven.

# P3 — Adaptive noding and candidate backends

## P3.1 Delivered candidate foundation

- [x] Separate broad-phase candidate enumeration from exact intersection and
  split accumulation.
- [x] Stream current SIMD and uniform-grid candidates without materializing all
  pairs.
- [x] Use one shared floating exact-intersection/split accumulator.
- [x] Retain original, synthetic, and unavailable source-chain identity.
- [x] Record deterministic workload descriptors.
- [x] Compare floating SIMD and uniform-grid topology, provenance, Z, work, and
  errors.
- [x] Prototype `geo::Intersections` for sparse exact-hit research.
- [x] Prototype an MCIndex-style monotone-chain candidate generator.
- [x] Compare the MCIndex prototype with brute-force envelope oracles on long
  sparse CAD, road, contour, overlap, self-intersection, and component-local
  workloads.

The sweep and MCIndex implementations remain research prototypes. Neither is a
public backend or default dispatch path.

## P3.2 MCIndex production experiment

Tracked by
[#1287](https://github.com/graydonpleasants/geo-polygonize/issues/1287).

- [x] Build a persistent/flat monotone-chain tree with cached envelopes.
- [x] Stream candidates through the common visitor.
- [x] Observe execution limits and cancellation during physical traversal.
- [x] Cover original ↔ original, original ↔ fallback, and fallback ↔ fallback
  pairs without misses or duplicate exact work.
- [x] Preserve source-chain/segment/parametric identity through preprocessing.
- [x] Use the shared exact/split/dissolve/validation path.
- [ ] Pass golden, compatibility, fuzz, real-world, provenance, Z,
  serial/parallel, and normalized-error conformance.
- [x] Measure native production-scale candidate reduction, allocations, peak RSS,
  and end-to-end effect.
- [x] Check in an accept/reject/promotion decision.

The research prototype now owns cached flat-tree traversal and a
`CandidatePair`-compatible visitor with bounded policy accounting and
cancellation checks. Its source-chain-preserving hybrid adapter routes emitted
pairs through the shared exact, split, normalization/dissolve, and independent
validation path. It remains disconnected from production dispatch until the
remaining conformance and decision-quality comparison gates are complete.
Dedicated-runner baseline evidence is now available from #1290.

Focused differential coverage now compares the hybrid adapter with the current
SnapNoder on self-intersecting, road/contour, mixed-chain, duplicate/reversed,
Z interpolation/source-ID preservation, bounded-resource, normalized
operational-error, input-permutation, normalized-boundary-error,
partial-overlap, nested-ring, and canonical feature-build fingerprint cases.
The focused corpus also includes a fixture-backed square-with-hole fingerprint
case, a dirty-bowtie fingerprint case, and a floating-microfaces compatibility
fingerprint case. The focused fixture set also covers zero-length compatibility
normalization and a profile-bearing provenance fingerprint case.
The focused fixture set also covers the Z Ignore policy fingerprint.
It also compares the Z conflict failure through the normalized-error contract.
The square-with-hole parity case also verifies the native serial/parallel graph
dispatch selected by the corresponding feature build.
The existing grid_10 fixture now records exact-intersection candidate reduction
against all possible segment pairs; dedicated production-scale measurement gates
remain open.
The focused comparisons execute under both default and no-default core builds;
the full golden, compatibility, fuzz, real-world, serial/parallel, and
normalized-error corpus gates remain open. A bounded seeded differential-fuzz
corpus now exercises 12 generated cases against the same baseline.

Hybrid candidate coverage uses the MCIndex traversal for original↔original
pairs and a streaming fallback scan for any pair involving synthetic or
unavailable segments; the fallback is intentionally research-only and is not a
production dispatch path.

The experiment also retains a direct per-segment source-chain identity index
covering source ID, chain position, segment position, chain length, and chain
kind across the flattened input.

Hybrid candidates can now be evaluated through the shared `ExactCandidate`
path; split application, dissolve, full noding validation, and production
dispatch remain separate promotion gates.

The dedicated publication workflow now has an explicit, opt-in
`mcindex-experiment` noding path for floating workloads. It retains source
line-string ranges, runs the research adapter before the shared polygonizer,
and emits the existing correctness-gated record shape under a distinct
implementation name. This remains evidence plumbing only; the production-scale
decision is recorded below.

Matched dedicated production and MCIndex publications now cover the 1k, 10k,
and 100k California highway tiers at commit `397ea8d`. Correctness passed on
all six publications, but MCIndex regressed p50 end-to-end time by 30.5% to
35.2% across the tiers and exceeded the secondary regression budget on multiple
tiers. The durable decision in
`benchmarks/decisions/mcindex-production-v1.json` rejects production promotion
and retains the adapter as research-only. Wasm, code-size, and compile-time
measurements were not pursued after the native promotion gate failed.

## P3.3 Sweep prototype boundary

The sweep prototype currently returns exact intersections rather than
policy-accounted broad-phase candidates.

- [x] Use it as a differential oracle while semantics and cost are characterized.
- [ ] Add execution-policy accounting and cancellation only if evidence justifies
  a production experiment.
- [ ] Reuse the common split/provenance/Z pipeline if it advances.
- [ ] Prefer removal over maintaining a second exact-topology implementation
  without a production win.

Related historical audit:
[#775](https://github.com/graydonpleasants/geo-polygonize/issues/775).

The sweep prototype is now covered as a differential oracle for complete
indexed exact-hit results, including collinear overlaps, against the pairwise
`geo` intersection reference. It remains exact-hit-only research plumbing with
no execution-policy accounting, cancellation, or production dispatch.

## P3.4 Backend promotion gate

A backend or dispatch rule may become production-visible only when:

- [ ] zero unexpected validator failures remain in golden, compatibility,
  production-scale, and fuzz corpora;
- [ ] canonical output, source sets, Z, diagnostics, and errors are identical;
- [ ] the end-to-end effect exceeds a predeclared threshold on more than one
  representative workload and supported architecture;
- [ ] no material regression exists outside the target workload;
- [ ] maintenance, compile-time, binary-size, and Wasm costs are documented;
- [ ] dispatch inputs are deterministic and inspectable;
- [ ] a durable decision record is checked in.

Prefer simple benchmark-derived rules over runtime learning or autotuning.

# P4 — Replicate-and-own tiling

Replicate-and-own tiling remains experimental and distinct from graph stitching.

## P4.1 Delivered coverage and recovery contract

- [x] Validate tile/grid options and propagate per-tile errors.
- [x] Bound tile count, assignment count, retries, fallback regions, parallelism,
  output polygons, and output coordinates.
- [x] Normalize signed-zero deduplication and merge duplicate provenance.
- [x] Emit owned-face, input-boundary, transformed-component, and
  ownership-domain evidence.
- [x] Track per-issue coverage resolution instead of a global fallback Boolean.
- [x] Retry unresolved tiles with bounded larger halos.
- [x] Recover conservative envelope-closed regions.
- [x] Escalate unresolved global-containment cases to caller-enabled whole-input
  fallback.
- [x] Preserve deterministic reports, traces, decline reasons, errors, and
  canonical output.
- [x] Differential-test tile size, origin, ownership, container shape, order,
  concavity, holes, overlaps, dirty crossings, dangles, and cuts.

## P4.2 Remaining boundary

- [ ] Broad missing-region detection remains observational rather than certified.
- [ ] Non-envelope-closed region-local reconciliation remains unsupported.
- [ ] Replicate-and-own results must not be described as graph-stitched.
- [ ] Unsupported cases must retain explicit evidence, fallback, or typed failure.
- [ ] Do not add more recovery heuristics when physical stitching is the correct
  dependency.

## P4.3 Independent bulk-versus-single-partition oracle

Tracked by
[#1389](https://github.com/graydonpleasants/geo-polygonize/issues/1389).

Inspired by `map-tile-toolkit` requiring its all-tiles and one-tile-at-a-time
slicers to produce identical per-tile results. Adopt the independent-oracle
pattern, not its no-new-vertices clipping contract.

- [x] Define a versioned deterministic local partition snapshot/fingerprint.
- [x] Reprocess one partition from original linework without reusing the bulk
  path's selected-input cache or mutable graph.
- [ ] Compare selected source segments, local noding, graph/face state, boundary
  nodes, atomic observations, provenance, representative IDs, Z, non-polygon
  families, and normalized errors.
- [ ] Exhaustively scan bounded neighboring empty partitions so missed and
  invented partition assignments are observable.
- [ ] Add input permutation, reversal, grouping, duplicate-vertex, duplicate-edge,
  tile-origin, tile-size, buffer, and precision metamorphic tests.
- [ ] Add a bounded fuzz target that names the first differing partition and
  stage.
- [ ] Retain final tiled-versus-untiled comparison as the stronger global gate.

## P4.4 Stream source segments into partition sinks

Tracked by
[#1392](https://github.com/graydonpleasants/geo-polygonize/issues/1392).
Blocked on the #1389 oracle for promotion.

Inspired by `map-tile-toolkit`'s one-pass `Grid::route`/`RouteSink` architecture.
Routing remains separate from topology mutation: emit original source segments,
then let the existing physical boundary-noding stage create atomic halfedges.

- [ ] Define a source-aware partition sink retaining chain, segment, parametric,
  provenance, representative-ID, and raw endpoint-Z identity.
- [ ] Add a documented same-partition inner-box fast path for segments safely
  farther than the halo from every border.
- [ ] Stream boundary-near segment/partition candidates without a
  candidate-count-proportional temporary vector.
- [ ] Charge every physical candidate visit to explicit limits and poll
  cancellation inside the scan.
- [ ] Compare assignments and local snapshots against #1389.
- [ ] Measure geometry-envelope false positives, allocations, peak memory,
  routing time, native/Wasm cost, and long sparse CAD/road/contour workloads.
- [ ] Keep the implementation private and accept, narrow, or reject it through
  a durable evidence record.

# P5 — True partition graph stitching

## P5.1 Delivered stitching prerequisites

- [x] Canonical signed-zero-safe partition-border node and edge keys.
- [x] Qualified partition/component/local-face references.
- [x] Stable observation IDs with conflicting-payload rejection.
- [x] Declared adjacency with complementary sides and exact border coordinates.
- [x] One-to-many breakpoint normalization.
- [x] Source-set and endpoint-Z candidate aggregation.
- [x] Physical tile-pass export after local face assignment.
- [x] Keep exported observations separate from current tiled output.

These prerequisites do not yet stitch topology.

## P5.2 Physical boundary noding

Tracked by
[#1288](https://github.com/graydonpleasants/geo-polygonize/issues/1288).

- [x] Split local arrangement edges at exact partition-boundary intersections,
  including breakpoints contributed by separate crossing edges.
- [x] Define exact rectangle-side intersection events, including endpoint,
  corner, finite-side collinear, signed-zero, and reversed-edge fixtures.
- [x] Handle endpoints, crossings, corners, and collinear-on-border edges.
- [x] Rebuild adjacency and angular order before component-local `next` links,
  face IDs, and unbounded markers are assigned.
- [x] Preserve provenance, representative IDs, Z interpolation/conflicts, and
  qualified component identity through boundary export and observation
  normalization.
- [x] Preserve cancellation and resource limits through boundary noding, with
  fail-closed pre-mutation checks for split events, graph nodes, noded
  segments, and cooperative cancellation.
- [x] Export only halfedges physically present on the boundary-noded arrangement.
- [x] Introduce collision-free atomic observation identity.
- [x] Emit bounded trace evidence for boundary-noding counts, atomic border
  observations, and rejected observations.
- [x] Exercise fixed-grid and certified-fixed export under input permutation.
- [x] Run arrangement and face-walk validators after boundary noding.
- [x] Leave replicate-and-own output unchanged.

## P5.3 Stitched arrangement and equivalence

Tracked by
[#1289](https://github.com/graydonpleasants/geo-polygonize/issues/1289).
Blocked by #1288.

- [x] Emit deterministic declared-adjacency twin/payload reconciliation
  evidence while leaving ambiguous, unrelated, and replicate-and-own output
  cases untouched.
- [x] Build and retain a deterministic face-qualified twin-link plan for exact
  declared-adjacency pairs with valid partition-matching local face identity;
  report and trace missing or malformed face lineage without mutating local
  adjacency or tiled output.
- [x] Build canonical border-node reconciliation evidence that unions source
  sets, representative IDs, face lineage, and endpoint-Z candidates under
  the selected Z policy, with fail-closed conflict, limit, and cancellation
  behavior; do not mutate local or tiled topology.
- [x] Build and retain deterministic connected-component evidence for
  qualified border faces, including explicit singleton faces and linked twin
  edges with fail-closed limit and cancellation behavior; do not mutate local
  or tiled topology.
- [x] Retain qualified local face-walk successor and unbounded-face evidence
  in a deterministic global face-boundary plan, reporting missing successors
  without mutating local `next`, face IDs, or tiled output.
- [x] Validate retained face-boundary plan identity, successor lineage, twin
  coverage, deterministic grouping, limits, and cancellation without mutating
  local or global topology.
- [x] Retain deterministic local face-boundary successor identities and report
  closed-cycle mutation readiness without assigning global `next` links, face
  IDs, or tiled output.
- [x] Materialize deterministic ordered local transition plans from the
  mutation gate, proving insertion-order equivalence while retaining
  incomplete cycles as evidence only.
- [x] Position declared face-qualified twins inside those ordered cycles with
  explicit mapped/unmapped and mutation-ready counts, without assigning global
  `next` links or face IDs.
- [x] Validate retained local face-walk cycles, cross-partition twin positions,
  payload/source/Z lineage, and face-adjacency connectivity as evidence;
  preserve incomplete cycles and local-unbounded multiplicity without claiming
  global face IDs or a planar Euler proof.
- [x] Add a conservative exactly-one-local-marker unbounded-face proof gate;
  report closed-cycle and twin-readiness requirements, and reject multiple local
  unbounded markers rather than guessing their global identity.
- [x] Retain deterministic component-level border payload merges for source IDs,
  representative IDs, endpoint Z candidates, selected Z policy, and conflict
  counts without writing global nodes or face payloads.
- [x] Retain a deterministic border-only Euler witness over unique boundary
  vertices, edges, closed cycles, and component arithmetic; keep it diagnostic
  and explicitly do not treat it as a planar Euler proof for the incomplete
  global arrangement.
- [x] Retain deterministic cross-tile global-next splice candidates from mapped
  twins and local cycle predecessors/successors; keep incomplete or conflicting
  assignments fail-closed without writing local `next` links or global face IDs.
- [x] Retain boundary-only deterministic global face identity cycle candidates
  from prospective successors; require a closed permutation walk and keep
  incomplete or non-permutation components from receiving global face IDs.
- [x] Retain a fail-closed global-next mutation plan from validated identity
  cycles; require exact cycle assignments while keeping local and global links
  unwritten until a real global topology exists.
- [x] Retain deterministic candidate global face IDs for closed boundary
  mutation cycles; keep incomplete cycles explicitly unassigned and do not
  write candidate IDs into local observations or tiled output.
- [x] Capture active local face-edge lineage and deterministically map local
  symmetric/successor identities plus declared face-qualified twins into
  global edge slots, validating geometry, provenance, Z, and cancellation
  boundaries without mutating topology.
- [x] Reconcile deterministic global face-node slots from active edge
  endpoints, retaining merged source, face, observation, representative, and
  endpoint-Z payloads with atomic conflict, limit, and cancellation behavior;
  attach node slots to evidence edges without mutating topology.
- [x] Map validated global face mutation cycles into global edge-slot
  successor candidates with cross-border twin reversal and endpoint-node
  continuity checks; retain incomplete or discontinuous plans without writing
  `next` links.
- [x] Materialize a detached full global directed-edge successor candidate from
  local successors plus validated boundary overrides, and require complete
  one-in/one-out endpoint-continuous closed cycles before any future mutation.
- [x] Gate detached topology application on unambiguous twins remaining backed
  by declared adjacent borders and reciprocal global edge slots; retain
  malformed or incomplete evidence as not ready without mutating topology.
- [x] Reconcile canonical border nodes, source sets, representative IDs, face
  lineage, and selected Z-policy decisions against active global face-node
  slots, while allowing canonical-only non-face-qualified observations.
- [x] Reconcile deterministic global connected components and verify that
  every face-qualified detached edge is covered exactly once without duplicate
  faces or twin ownership.
- [x] Validate that deterministic candidate global face IDs map one-to-one
  onto detached closed candidate cycles with contiguous IDs; retain incomplete,
  duplicate, or unmapped evidence without mutating topology.
- [x] Gate exactly-one local-unbounded-face evidence on one mapped detached
  candidate face ID and cycle, retaining missing or duplicate markers without
  mutating topology.
- [x] Combine detached topology, component, face-ID, unbounded, twin, cycle,
  source, Euler, and face-walk evidence into one bounded pre-mutation gate;
  retain incomplete proof without mutating topology.
- [x] Atomically commit a validated detached global successor vector only after
  the complete gate; keep local half-edge links, face IDs, and tiled output
  untouched until their own promotion proofs exist.
- [x] Atomically commit deterministic candidate face IDs onto detached cycle
  starts only after unique cycle mapping, contiguous IDs, successor mutation,
  and exactly-one-unbounded evidence are ready; keep local IDs and output
  payloads untouched.
- [x] Atomically promote the uniquely proven unbounded candidate onto detached
  global identity state only after successor and face-ID commits; retain the
  cycle start and candidate ID without changing local IDs or output payloads.
- [x] Materialize a detached per-edge global face-ID map from committed closed
  cycles, requiring complete edge coverage and the committed unbounded identity
  while leaving local topology and tiled output untouched.
- [x] Cross-check detached per-edge face identity against successor cycles,
  reciprocal twins, source lineage, and retained face-walk/Euler evidence;
  preserve incomplete proofs without mutating topology or output.
- [x] Cross-check the committed detached global successor permutation against
  local face successors, retained boundary overrides, per-edge face identity,
  and reciprocal face-qualified cross-partition twins without mutating
  topology or output.
- [x] Map each detached closed successor cycle exactly once to its candidate
  global face plan, validating observation slots, qualified local face
  lineage, and local-unbounded marker lineage without mutating topology or
  output.
- [x] Cross-check detached cycle/face lineage against complete component
  coverage and exactly-one-unbounded application evidence with explicit count
  mismatches and bounded cancellation/limit handling; keep the promotion gate
  evidence-only.
- [x] Cross-check every detached gate-ready face cycle against active source
  lineage, endpoint Z, face metadata, and reconciled global-node payloads;
  retain missing or corrupt payloads as bounded evidence without mutating
  topology or output.
- [x] Validate detached cycle geometry from the committed successor and node
  payloads, retaining winding, nested-containment, shell/hole orientation, and
  exactly-one-unbounded negative-orientation evidence; retain bounded proper
  crossing, endpoint/boundary-touch, and expected-versus-unexpected collinear
  overlap evidence without classifying or extracting stitched output; retain
  stable canonical ring payload, non-adjacent self-intersection, and reciprocal
  symmetric-edge coverage evidence as a separate pre-extraction gate.
- [x] Combine committed detached face identity, successor, cycle lineage,
  payload, geometry, and canonical ring evidence into one bounded final
  pre-extraction readiness record without promoting local topology or output.
- [x] Materialize canonical detached ring payload evidence with face IDs,
  source lineage, endpoint Z, and exactly-one-unbounded coverage only after the
  final gate; retain it privately without merging stitched output.
- [x] Classify retained detached ring payloads into private shell and hole
  candidates using winding and containment depth; keep the unbounded cycle
  separate and reject crossings, touches, unexpected overlaps, and same-winding
  nesting without promoting stitched output.
- [x] Assemble deterministic private shell-to-hole candidate groups from the
  classified rings; require every bounded hole to have one containing shell and
  retain unassigned or multiply-contained holes without promoting output.
- [x] Materialize private extraction candidates backed by canonical ring
  payloads; require one-to-one shell/hole payload coverage, valid XYZ/source
  lineage, bounded limits, and atomic readiness without promoting output.
- [x] Materialize private stitched shell/hole payloads from ready candidates,
  retaining raw XYZ bits and merged source IDs with atomic limits and
  fail-closed evidence; do not expose public polygon output.
- [x] Retain tile-local dangle, cut-edge, and invalid-ring geometry as
  deterministic private XYZ evidence with duplicate and finite-coordinate
  checks; defer source-lineage promotion until the upstream payload contract
  carries it explicitly.
- [x] Build private global `next` links and deterministic global face IDs from
  validated detached edge slots; keep local topology and tiled output
  untouched.
- [x] Identify exactly one global unbounded face and carry its private
  cycle/edge identity into the detached global topology records.
- [x] Consolidate private global topology, shell/hole payload, and non-polygon
  evidence into one fail-closed extraction-readiness record; keep output
  promotion deferred.
- [x] Validate and consolidate detached twin, cycle, source, Euler, and
  face-walk invariant evidence in a private fail-closed gate.
- [x] Commit a private atomic extraction snapshot of validated stitched
  shells, holes, dangles, cuts, and invalid rings, retaining source IDs and
  raw XYZ payloads without changing tiled output.
- [x] Expose validated stitched shells, holes, dangles, cuts, invalid rings,
  provenance, and Z through an additive experimental output sidecar; keep the
  existing tiled polygons unchanged until full untiled equivalence is proven.
- [x] Add an opt-in full canonical comparison of validated stitched output
  against the same-options untiled result; record fail-closed equivalence
  evidence without replacing tiled output or promoting on the comparison alone.
- [x] Differential-fuzz bounded tile origins, sizes, orders, precision
  profiles, provenance, and Z-policy profiles against same-options untiled
  output; retain coverage evidence as the fail-closed exception.
- [ ] Publish performance and peak-memory evidence.
- [x] Document the exact per-call eligibility boundary for a future promotion:
  an input is eligible only when the same `TiledPolygonizer::polygonize` call
  returns `stitched_output: Some(_)`,
  `partition_border_global_stitched_output_ready` is true,
  `partition_border_global_untiled_equivalence_checked` is true,
  `partition_border_global_untiled_equivalence_ready` is true, and the
  mismatch count is zero. The comparison must cover canonical polygon
  geometry, Z, source IDs, provenance, dangles, cut edges, and invalid rings
  under the same options. Missing stitched output, incomplete private
  evidence, unresolved coverage, or a fuzz match is not promotion evidence;
  this is a per-input/per-options gate, not a broad geometric-class claim.

This eligibility boundary is documented only. No public selector promotes the
sidecar, and performance or peak-memory evidence is still required before any
promotion decision.

**Promotion gate:** exact untiled equivalence or an explicit deterministic
fallback for every documented supported case.

## P5.4 Transactional partition mosaic and staged consistency

Tracked by
[#1390](https://github.com/graydonpleasants/geo-polygonize/issues/1390) and
[#1391](https://github.com/graydonpleasants/geo-polygonize/issues/1391).

Inspired by `map-tile-toolkit`'s transactional `Mosaic`: stage and validate one
tile before mutating shared state, reject inconsistencies atomically, support
replacement/purge, and prove insertion-order independence. Generalize that idea
to full partition arrangements rather than polyline edge sets.

- [ ] Define an immutable versioned `PartitionSnapshotV1` containing local graph,
  face, border, source, representative-ID, Z, non-polygon, options, and execution
  evidence.
- [ ] Add atomic `replace_partition` and `purge_partition` operations; rejection
  must preserve the previous mosaic fingerprint exactly.
- [ ] Prove identical replacement is idempotent and arbitrary insertion order
  produces one deterministic mosaic.
- [ ] Use the transaction boundary for larger-halo retries and independently
  regenerated partitions.
- [ ] Separate physical mosaic consistency from topology mosaic readiness.
- [ ] Retain all face-side claims on one physical span rather than forcing one
  physical edge to have one semantic twin.
- [ ] Classify physical spans as `valid`, `incomplete`, or `conflict`, with typed
  witnesses naming partitions, coordinates, claim multiplicity, and payload
  class.
- [ ] Define deterministic core-ownership/corroboration obligations without
  using ownership to select final polygon output.
- [ ] Allow only physically valid, face-qualified claims into global twin,
  successor, component, face, unbounded-face, and extraction gates.
- [ ] Keep physically valid but topologically ambiguous cases fail-closed and
  distinct from physical conflicts.
- [ ] Retain full global arrangement, Euler, containment, provenance, Z,
  cancellation, limits, trace, and untiled-equivalence validation.

# P6 — Later evidence-gated research

These programs begin only after their stated predecessors.

## P6.1 Flat partition snapshots and checked integer fixed-grid topology

Tracked by
[#1393](https://github.com/graydonpleasants/geo-polygonize/issues/1393).

Inspired by `map-tile-toolkit`'s flat vertex/offset arenas and exact integer tile
space. Treat both as private removable experiments, not universal input or
precision changes.

- [ ] Prototype contiguous arenas and checked offset tables for coordinates,
  edges, successors, faces, source/Z payloads, border observations, and
  non-polygon output.
- [ ] Provide borrowed views with lifetimes tied to immutable committed
  snapshots.
- [ ] Preserve deterministic fingerprints, transactional replacement, purge,
  limits, cancellation, and serialization/reconstruction bounds.
- [ ] Define explicit origin/scale and checked `i64` conversion for caller-selected
  fixed-grid/certified-fixed precision only.
- [ ] Use Euclidean division for negative partition coordinates and exact
  local/global round trips.
- [ ] Reject overflow before topology work and retain `i128` predicate fallback
  where the documented integer domain requires it.
- [ ] Keep floating coordinate behavior and bits unchanged; never quantize
  floating mode silently.
- [ ] Keep Z and source payloads separate from XY topology identity.
- [ ] Benchmark equality, hashing, ordering, graph construction, boundary
  predicates, snapshot size, allocation count, peak RSS, native/Wasm cost, and
  serialization.
- [ ] Pass #1389 local-partition and full global topology conformance before
  timing.
- [ ] Check in separate accept/reject decisions for the flat layout and integer
  specialization; remove losing prototypes.

## P6.2 Explicit robustness fallback profile

- [ ] Keep the profile opt-in.
- [ ] Record every attempted policy and failure witness.
- [ ] Never mutate caller precision silently.
- [ ] Return the effective policy in the report.
- [ ] Bound retries through execution policy.
- [ ] Add cross-binding compatibility fixtures.

## P6.3 Wide-work-unit SIMD

Follow
[`docs/SIMD_OPTIMIZATION.md`](docs/SIMD_OPTIMIZATION.md).

Begin only after production-scale baselines and candidate dispatch boundaries
are stable.

- [ ] Batch independent candidate pairs rather than vectorizing XY components.
- [ ] Compare query-major and pair-major AoSoA layouts.
- [ ] Use a conservative wide filter with scalar robust fallback.
- [ ] Measure packing, lane utilization, fallback, emission, merge, and memory.
- [ ] Evaluate point-wide repeated containment separately.
- [ ] Require CPU validation and full topology equivalence.
- [ ] Keep losing experiments out of production.

Related targeted threshold issue:
[#838](https://github.com/graydonpleasants/geo-polygonize/issues/838).

## P6.4 Streaming and out-of-core execution

Begin after physical stitching has a credible equivalence contract and
production-scale memory evidence identifies a concrete bottleneck.

- [ ] Stream Arrow `RecordBatch`, GeoParquet row groups, and FlatGeobuf features
  with bounded memory and backpressure.
- [ ] Preserve source IDs and profiles across chunks.
- [ ] Separate I/O partitioning from topology partitioning.
- [ ] Add resumable manifests with checksums, options, partition state, and
  library version.
- [ ] Evaluate disk-backed or memory-mapped indexes only after profiling.
- [ ] Measure I/O, peak RSS, temporary storage, recovery, and output equivalence.

# Post-1.0 capability tree

| Existing issue | Reframed scope | Required predecessors |
|---|---|---|
| [#720 — graph-native Boolean overlay](https://github.com/graydonpleasants/geo-polygonize/issues/720) | Winding-labeled overlay on an explicit validated arrangement. | Global face model, provenance algebra, robust noding. |
| [#714 — topology-preserving simplification](https://github.com/graydonpleasants/geo-polygonize/issues/714) | Simplify shared edge chains once, then rebuild and validate incident faces. | Stable shared-edge identity and arrangement validator. |
| [#688 — robust buffering](https://github.com/graydonpleasants/geo-polygonize/issues/688) | Offset curves followed by certified noding and face selection. | Arrangement face selection and dedicated buffer corpus. |
| [#697 — MVT and TopoJSON](https://github.com/graydonpleasants/geo-polygonize/issues/697) | Downstream adapters with topology-preserving quantization/shared-edge encoding. | Stable simplification and real consumers. |
| [#663 — incremental topology](https://github.com/graydonpleasants/geo-polygonize/issues/663) | Separate experimental arrangement API with local invalidation and delta reports. | Global arrangement identities and mutation invariants. |
| [#769 — geodesic polygonization](https://github.com/graydonpleasants/geo-polygonize/issues/769) | Separate spherical/ellipsoidal kernel, not a planar precision mode. | Written geodesic contract and spherical-predicate corpus. |
| [#664 — database adapters](https://github.com/graydonpleasants/geo-polygonize/issues/664) | Consumer-driven DuckDB/PostGIS adapters around stable streaming contracts. | P6.4 and concrete deployment benchmarks. |
| [#771 — GPU point-in-polygon](https://github.com/graydonpleasants/geo-polygonize/issues/771) | Broad-phase or batch-predicate experiment with mandatory CPU validation. | #1290, stable candidate boundary, measured CPU bottleneck. |

# Recommended aggressive execution order

Use stacked PRs so dependency-ready work continues while parents are reviewed.

## Stack R — release and governance

```text
#1285 registry verification and install smoke
└── #1286 post-1.0 docs, metadata, MSRV, and API boundary
```

## Stack E — production evidence

```text
#1290 source materialization and current baselines
├── #1291 component memory/layout decisions
└── #1287 MCIndex production experiment and decision
```

MCIndex implementation plumbing can start before every large workload is ready,
but it cannot be promoted without #1290 evidence.

## Stack T — true stitching

```text
#1288 physical boundary noding and atomic observations
└── #1289 stitched arrangement, validators, and untiled equivalence
```

## Stack M — partition mosaic hardening

```text
#1389 bulk-versus-single-partition oracle
├── #1392 streamed source-segment router
└── #1390 transactional partition snapshots and mosaic
    ├── #1391 physical consistency versus topology readiness
    └── #1393 flat snapshot and checked integer research
```

#1393 additionally depends on #1290/#1291 evidence before any representation or
performance decision. #1392 may prototype early but cannot replace current input
selection until #1389 is green across representative workloads.

## After those stacks

1. Close the local partition oracle before changing input routing.
2. Make partition replacement transactional before adding resumable/streaming
   topology state.
3. Separate physical consistency from face topology before relaxing any
   ambiguous-border readiness gate.
4. Decide component execution/layout from #1291.
5. Decide MCIndex/sweep production status from #1287 and evidence.
6. Decide whether stitched output is selectable for a documented input class.
7. Revisit flat/integer partition representation from #1393.
8. Revisit wide SIMD only after candidate and workload evidence stabilizes.
9. Begin streaming/out-of-core work only after stitching and memory evidence.

# Stacked PR rules

- Root each independent stack at current `main`.
- Root every child at its immediate parent branch.
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
- public registry artifacts exist;
- real registry installs pass;
- supported API conformance passes;
- migration and support metadata are current.

## Adaptive backend

A backend is production-visible only when:

- candidate coverage and topology conformance are exact;
- validator failures are zero or classified;
- provenance, Z, errors, limits, and cancellation agree;
- production-scale evidence exceeds predeclared thresholds;
- the dispatch rule is deterministic and inspectable;
- a durable decision record exists.

## Tiled graph stitching

Stitched output is selectable only when:

- bulk and independent one-partition snapshots agree for the documented class;
- local arrangements are physically boundary-noded;
- committed partition snapshots are transactionally consistent;
- physical edge claims are valid and face-topology readiness is unambiguous;
- border twins and payloads reconcile deterministically;
- one valid global arrangement and unbounded face exist;
- full canonical output equals untiled execution for the documented class;
- unsupported cases have explicit evidence/fallback/error behavior;
- resource, cancellation, trace, fuzz, and memory gates pass.

# Research references

## Partition tiling inspiration

[`nyurik/map-tile-toolkit`](https://github.com/nyurik/map-tile-toolkit) inspired
the partition-oracle, transactional-mosaic, streamed-sink, flat-arena, and exact
integer-space research in #1389–#1393:

- [`Grid::route` and `RouteSink`](https://github.com/nyurik/map-tile-toolkit/blob/main/src/grid.rs)
  for one-pass bounded segment-to-tile routing;
- [`SlicerAll`/`SlicerOne`](https://github.com/nyurik/map-tile-toolkit/blob/main/src/slicer.rs)
  for independent all-tiles versus one-tile equivalence and flat offset arenas;
- [`Mosaic`](https://github.com/nyurik/map-tile-toolkit/blob/main/src/mosaic.rs)
  for atomic replacement, purge, insertion-order independence, and shared-edge
  consistency checks;
- [slicing equivalence fuzzing](https://github.com/nyurik/map-tile-toolkit/blob/main/fuzz/fuzz_targets/slice_equivalence.rs)
  and [bad mosaic fixtures](https://github.com/nyurik/map-tile-toolkit/blob/main/tests/mosaic_bad.rs)
  for executable cross-path and conflict oracles;
- [exact integer predicates](https://github.com/nyurik/map-tile-toolkit/blob/main/src/geom.rs)
  for checked integer partition-space research.

These are research references only. Do not adopt `map-tile-toolkit` as a runtime
dependency without a separate compatibility/evidence decision. Its deliberate
“no new boundary vertices” polyline contract and directed-edge-set reassembly do
not prove polygon face topology, global containment, representative edge IDs,
Z reconciliation, or canonical untiled equivalence for `geo-polygonize`.

## Topology and noding references

- [JTS `SnapRoundingNoder`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/noding/snapround/SnapRoundingNoder.html)
- [JTS `ValidatingNoder`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/noding/ValidatingNoder.html)
- [JTS `FastNodingValidator`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/noding/FastNodingValidator.html)
- [JTS `MCIndexNoder`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/noding/MCIndexNoder.html)
- [JTS `OverlayNGRobust`](https://locationtech.github.io/jts/javadoc/org/locationtech/jts/operation/overlayng/OverlayNGRobust.html)
- [`geo::algorithm::sweep::Intersections`](https://docs.rs/geo/latest/geo/algorithm/sweep/struct.Intersections.html)
- [GEOS `UnaryUnionOp`](https://libgeos.org/doxygen/classgeos_1_1operation_1_1geounion_1_1UnaryUnionOp.html)
- [CGAL 2D Arrangements and DCEL](https://doc.cgal.org/latest/Arrangement_on_surface_2/index.html)
- [CGAL arrangements with history](https://doc.cgal.org/latest/Arrangement_on_surface_2/classCGAL_1_1Arrangement__with__history__2.html)

# Invariants for all future work

- Keep stable behavior expressible through the canonical semantic options schema.
- Keep execution budgets and cancellation separate from semantic options.
- Preserve deterministic canonical output and structured, actionable errors.
- Preserve complete source provenance through noding, dissolve, graph
  decomposition, tiling, stitching, and future topology operations.
- Treat replicate-and-own tiling and true graph stitching as different
  algorithms with different contracts.
- Do not claim robustness beyond the selected noding policy’s checked
  postconditions.
- Do not silently fall back to another precision or guarantee.
- Do not accept a performance win before its correctness gate passes.
- Do not add a stable API path without equivalent cross-binding conformance where
  applicable.
- Ensure every hard failure can emit a bounded witness or reproducible trace.
- Add the smallest strict regression that would have caught each bug.
