# Engineering Roadmap

This is the active, evidence-gated roadmap for `geo-polygonize`.

It was reconciled against `main` through PR #919 on July 22, 2026. The
foundation planned in the original roadmap has largely shipped; this document
now orders the remaining work required for a production-grade, state-of-the-art
planar polygonization library. Milestone names are planning buckets, not release
date or version promises.

## North star

Make `geo-polygonize` the best-supported pure-Rust planar
linework-to-polygons kernel across native Rust, Python, WebAssembly, and Arrow:

- certified fixed-precision correctness when that guarantee is selected;
- deterministic, explainable results with complete source provenance;
- one semantic contract across every supported binding and data path;
- bounded and cancellable execution on untrusted or unexpectedly difficult
  inputs;
- performance leadership demonstrated on public, correctness-gated workloads;
- a stable GeoRust-native facade with clear support and compatibility policies.

The target is not merely “faster than GEOS on a synthetic benchmark.” The
target is a defensible contract:

> For a documented input class and precision policy, `geo-polygonize` produces
> a deterministic, independently validated topology result, exposes enough
> evidence to explain failures, and does so competitively across supported
> targets.

## Delivered baseline

The releases from `0.40.0` through `0.51.2`, plus PR #919, established the
current foundation:

- [x] Stateless one-shot polygonization and reusable allocation-only workspaces.
- [x] One validated, serde-defaulted `PolygonizerOptions` schema across Rust,
  Python, and Wasm.
- [x] Finite-coordinate validation and scale-independent topology validity,
  with explicit optional minimum-area filtering.
- [x] Explicit floating and fixed precision models.
- [x] Independent full-noding validation with deterministic failure witnesses.
- [x] Certified hot-pixel fixed-precision noding.
- [x] Iterative grid noding documented as unchecked rather than certified.
- [x] Deterministic canonical output and strict, read-only golden fixtures.
- [x] A persisted compatibility corpus containing parity, expected divergence,
  and invalid/ambiguous cases.
- [x] Typed binding errors, diagnostics, complete edge-dissolve provenance, and
  explicit Z policies.
- [x] A narrow stable Rust facade plus borrowed `geo_traits` input and
  `geo_types::MultiPolygon` conversion.
- [x] Experimental tiled polygonization with validated options, safe ownership,
  collision-safe deduplication, deterministic output, equivalence fixtures, and
  per-tile/stitching reports.
- [x] Dedicated core, Arrow/GeoArrow/GeoParquet, Python, FlatGeobuf, and Wasm
  crates or adapters.
- [x] Native and Wasm benchmarks, allocation/instruction profiling, fuzzing,
  differential tests, supply-chain checks, and release automation.
- [x] Measured native scalar/portable-SIMD dispatch on Linux x86-64 and AArch64;
  the evidence supports retaining the current workload- and architecture-aware
  choices rather than adding another native SIMD layer.

## Definition of production readiness

The library is production-ready when all of the following are true:

1. **Correctness is selectable and checkable.** Certified modes have independent
   postcondition checks. Unchecked modes are named and documented as such.
2. **Bindings agree.** Equivalent APIs across Rust, Python, Wasm, Arrow, and C
   produce the same canonical topology fingerprint or the same normalized error.
3. **Work is bounded.** Callers can set resource budgets and cancel long-running
   work without receiving silent partial output.
4. **Failures are reproducible.** Difficult inputs produce structured witnesses,
   versioned traces, and minimizable fixtures.
5. **Performance claims are comparable.** Benchmarks use equivalent pipelines,
   correctness gates, public data, pinned dependencies, and reproducible
   environments.
6. **The stable surface is supportable.** MSRV, targets, feature combinations,
   ABI behavior, deprecation policy, and release synchronization are explicit.

## Operating rules

These rules apply to every milestone:

- Keep changes small, independently releasable, and guarded by the smallest
  regression that would have caught the problem.
- Keep semantic controls in the canonical options schema. Keep operational
  controls such as time, memory, trace, and output budgets in a separate
  execution policy.
- Never silently change precision, switch guarantees, or return partial output.
- Do not expose a new backend publicly until it passes the independent validator,
  the conformance corpus, and a predeclared benchmark promotion gate.
- Preserve deterministic canonical output and complete source provenance through
  every optimization.
- Do not publish a performance comparison until topology correctness has passed
  first.
- Do not add a new crate without a real dependency boundary and consumer.
- Keep experimental tiling, graph internals, and research backends hidden until
  their contracts are complete.
- Record rejected experiments and their evidence so they are not repeatedly
  rediscovered.

# Active execution plan

Work the following milestones in order. Parallel work is appropriate only where
the dependency notes allow it.

## P0 — Production contract and bounded execution

### P0.1 Canonical conformance harness

Progress: Rust-side `TopologyFingerprintV1` and normalized errors are complete
for the one-shot, workspace, and borrowed GeoRust paths. A shared exact fixture
now runs through those three paths, Python canonical options, and both Wasm
canonical-options paths (GeoJSON report/fingerprint and typed buffer). The
same fixture's retained polygon contract now also runs through GeoArrow and
the Arrow C Data Interface, FlatGeobuf, and GeoParquet. Normalized core
failures are exposed by Python and Wasm adapters.

Build one fixture-driven conformance suite covering every supported entrypoint:

- [x] Rust one-shot `polygonize`.
- [x] Rust `polygonize_with_workspace`.
- [x] Borrowed `geo_traits` / GeoRust facade.
- [x] Python canonical-options API.
- [x] Wasm canonical-options GeoJSON API.
- [x] Wasm typed-buffer API.
- [x] GeoArrow / Arrow IPC API.
- [x] Arrow C Data Interface API.
- [x] FlatGeobuf and GeoParquet adapters where their input contracts overlap.

Define a versioned canonical fingerprint containing:

- canonical polygons, ring coordinates, and hole structure;
- dangles, cut edges, and invalid rings;
- representative edge IDs and complete provenance source sets;
- deterministic topology diagnostics;
- normalized error family, stage, and witness when execution fails.

Timings, allocator-dependent counters, and platform-specific metadata must not
participate in semantic equality.

Intentionally lossy APIs, such as direct `MultiPolygon` conversion, must declare
which result families they discard. Their tests should compare the retained
contract rather than pretending the API is lossless.

**Done when:** one fixture can be executed through all equivalent paths and CI
reports a field-level semantic diff for any mismatch.

### P0.2 Align the Wasm API contracts

The current high-level Wasm options endpoint is documented as returning a full
result but its GeoJSON return path is polygon-only.

Progress: Complete. The playground uses the canonical options/report API and
projects its exact report coordinates only for visualization.

- [x] Keep the existing polygon-only API for compatibility and document it
  accurately.
- [x] Add a clearly named full-result endpoint,
  `polygonizeReportWithOptions`, returning polygons, dangles, cut edges, invalid
  rings, provenance, diagnostics, and selected options.
- [x] Make the playground use the canonical options/report API rather than the
  legacy positional wrapper.
- [x] Add TypeScript types generated from the same Rust schema and conformance
  tests for both JSON and typed-buffer paths.

**Done when:** function names, generated types, docs, and runtime return values
describe the same contract.

### P0.3 Options and release synchronization

Progress: Complete. Canonical options round-trip across bindings, serialized
reports carry schema metadata, and release versions are checked in CI.

- [x] Round-trip every canonical option through Rust serde, generated
  TypeScript, Python helpers, Wasm, and C JSON options.
- [x] Add a CI gate preventing drift between Rust crate, npm, and PyPI releases.
- [x] Add explicit schema-version metadata to serialized reports and traces.
- [x] Publish migration tests for old supported option payloads.
- [x] Define which legacy positional APIs remain supported through `1.x`.

### P0.4 Versioned C ABI

The Arrow C Data Interface adapter has panic containment and atomic output
publication. ABI discovery, fixed return codes, thread-local structured error
retrieval, canonical JSON request evolution, and ownership-boundary tests are
now available.

- [x] Add an ABI version query.
- [x] Prefer a versioned/size-tagged request struct or the canonical JSON options
  entrypoint over extending the legacy `repr(C)` options struct.
- [x] Define stable numeric status codes.
- [x] Add structured last-error retrieval with error family, stage, message, and
  optional noding witness.
- [x] Document ownership transfer, cleanup, nullability, thread-safety, and
  reentrancy.
- [x] Test failure injection before and after every ownership-transfer boundary.

**Done when:** a C consumer can detect ABI compatibility and diagnose a failure
without parsing logs or relying on Rust enum layout.

Operational limits should not alter topology semantics, so introduce a separate
execution policy rather than extending `PolygonizerOptions`.

### P0.5 Execution budgets

Progress: The core now has an opt-in, non-semantic `ExecutionPolicy` that
rejects oversized input line-string, segment, coordinate, and noded-segment
counts before graph construction, plus noding candidate, exact-intersection,
split-event, and iteration work before split application. Graph node, edge, and
ring limits now stop before classification. Final polygon and output-coordinate
limits stop before a result is returned.

Add opt-in limits for:

- [x] input line strings, segments, and coordinates;
- [x] noded segment expansion;
- [x] candidate pairs and exact intersection calls;
- [x] split events and iterative-noding passes;
- [x] graph nodes, edges, and rings;
- [x] polygons and output coordinates;
- [x] per-stage and total trace bytes;
- [ ] estimated working memory where a reliable bound is available.

Return a typed result such as
`ResourceLimitExceeded { stage, limit, observed }`. Do not return partial
polygons unless a future API explicitly models partial, resumable computation.

Add adversarial tests for dense crossings, overlap explosions, extreme duplicate
multiplicity, deeply nested rings, and output amplification.

Progress: Each named adversarial family now exercises a typed execution-policy
limit in the core regression suite.

Trace-only floating, uniform-grid, and certified noding capture buffers now
consume one shared remaining trace-byte budget before growing and mark the
result truncated when that capture budget is exhausted. Containment candidate,
tiled ownership, and maximal-ring snapshot captures apply the same pre-growth
accounting. All currently known trace-only capture vectors are covered;
callers can now independently bound summary, noding, graph, ring, and output
trace bytes while retaining the existing total-only API as a compatibility
wrapper. Exhausting one stage marks the trace truncated without suppressing
later stages.

### P0.6 Cooperative cancellation

Progress: Native `CancellationToken` values live in `ExecutionPolicy`, not
semantic options. Core checkpoints cover ingest, noding, graph construction,
ring extraction, containment, canonicalization, and output flattening; a
cancelled workspace run is reusable after resetting its token. Python releases
the GIL for owned Rust work and polls signals every 10 ms before cancelling the
worker token. Wasm has cancellable GeoJSON and report calls in disposable
browser workers; aborting terminates the worker rather than claiming that a
synchronous main-thread Wasm export can yield. SIMD, grid, and hot-pixel split
scans poll every 256 work items. Uniform-grid bounds, counting, and population
passes also poll every 256 input segments. Graph dangle pruning also polls node
discovery and its mutable work stack every 256 items; token-aware containment
polls every 256 hole assignments while token-free runs retain the parallel path.
Cut-edge removal and minimal-ring extraction also poll every 256 graph-traversal
work items. Ring classification and invalid-ring filtering poll every 256
ring, coordinate, and containment-comparison work items.
Final polygon assembly and deterministic-ordering preprocessing also poll every
256 items. Standard-library sorts cannot be interrupted, so cancellation-enabled
runs reject every audited sort above 1,000,000 items before sorting begins.
Token-aware graph bulk loading and node-star preparation also poll every 256
nodes or edges. Input validation and Z-conflict reconciliation also poll every
256 lines or endpoints.
Line-string conversion and fixed-grid snapping also poll every 256 input
coordinates or segments.
GeoCompat coordinate restoration also polls every 256 snapped coordinates or
noded segments.
Token-aware noding validation also polls every 256 segments or candidate pairs.

- [x] Add cancellation checkpoints at ingest, candidate enumeration, split
  application, graph construction, ring extraction, containment, canonicalization,
  and output flattening.
- [x] Provide a native cancellation token or callback that does not become part
  of semantic options.
- [x] Release the Python GIL during pure Rust work where safe and check Python
  signals at bounded intervals.
- [x] For Wasm, use a worker-based or genuinely asynchronous/chunked API for
  cancellation. Do not claim `AbortSignal` support for a synchronous main-thread
  Wasm call that cannot yield.
- [x] Ensure cancellation unwinds temporary state safely and never poisons a
  reusable workspace.

**Done when:** every long-running phase can be stopped within a documented work
interval and the same workspace can subsequently execute a valid fixture.

## P1 — Evidence and explainability

### P1.1 Public workload corpus

Create a redistributable corpus with small checked-in clips and optional larger
checksum-pinned downloads covering:

- [x] already-noded cadastral or coverage boundaries;
- [x] OSM/network linework;
- [x] CAD/CFB dirty linework;
- [x] contour or hydrographic boundaries;
- [x] long sparse polylines;
- [x] dense crossing-heavy linework;
- [x] collinear overlaps and duplicate boundaries;
- [x] disconnected but spatially nested rings;
- [x] extreme translations, coordinate spans, and grid sizes;
- [x] 2.5D inputs exercising every Z policy.

Every workload must include provenance/license metadata, expected contract class,
and the precision/noding policy being tested.

### P1.2 Equivalent benchmark lanes

Maintain three distinct comparisons:

1. **Already-noded polygonization:** graph/face extraction against
   GEOS/JTS polygonization on the same fully noded input.
2. **Floating noding plus polygonization:** equivalent floating noding and
   polygonization pipelines.
3. **Certified fixed precision:** hot-pixel snap rounding, independent
   validation, and polygonization against an equivalent JTS fixed-precision
   pipeline.

Do not mix integrated noding on one side with a bare polygonizer on the other.

Before recording time, require:

- [x] full-noding validation where the policy claims it;
- [x] expected compatibility classification;
- [x] canonical topology fingerprint or documented divergence;
- [x] polygon, ring, dangle, cut-edge, invalid-ring, and provenance metrics.

Progress: Benchmark record V1 now encodes these gates and all three lanes as a
strict machine-readable schema. The native runner now enforces them for
already-noded, floating, and certified fixed-precision workloads before timing,
and requires external reference and peak-RSS evidence. A versioned
cross-implementation topology fingerprint and GEOS/Shapely reference runner now
cover the already-noded and floating lanes without pretending Rust-only
provenance or diagnostics are external equality evidence. A pinned JTS
snap-rounding, validation, deduplication, and polygonization reference now
covers every certified-fixed parity workload. Result publication remains
separate work.

Record:

- p50, p95, throughput, and sample count;
- phase times;
- allocations and peak RSS;
- candidate pairs, exact predicates, split events, and segment expansion;
- input and output sizes;
- architecture, OS, compiler, feature flags, dependency versions, and commit SHA.

### P1.3 Durable benchmark reporting

- [x] Publish machine-readable artifacts and a human-readable trend dashboard.
- [x] Separate noisy runner samples from decision-quality measurements.
- [x] Pin GEOS, JTS, Shapely, Rust, and Node versions in comparison jobs.
- [x] Define the minimum effect size and regression budget before each backend or
  dispatch experiment.
- [x] Keep rejected experiments and crossover measurements linked from the
  relevant decision record.

Progress: The correctness jobs pin Rust, Python, Shapely and its bundled GEOS,
plus the Maven/Java image, JTS, and JSON adapter used by the certified lane.
The cross-architecture and maintenance comparisons now pin stable and nightly
Rust, Python, Shapely and its bundled GEOS, JTS, Node, NumPy, and Wasm tooling;
a regression test rejects floating selectors. Neither correctness job publishes
timing artifacts. Benchmark decision policy V1 now classifies
diagnostic results as nonpublishable and requires dedicated-runner repetitions,
warmup, dispersion, correctness, environment, and commit gates for
decision-quality evidence. It also predeclares a 5% minimum effect size and 2%
secondary-metric regression budget. Publication V1 now bundles only records
that pass the policy's runner, warmup, repetition, sample, identity, schema, and
dispersion gates; diagnostic and noisy records cannot enter that artifact.
Decision record V1 now requires checksum-pinned baseline and candidate evidence,
preserves the predeclared thresholds, and refuses unlinked rejected experiments
or measured crossovers. A deterministic Markdown renderer now turns valid
publication and decision artifacts into a human-readable trend view. The
correctness workflow now retains its validated GEOS and JTS reference results
for 30 days. Decision-quality timing publication still requires records from a
dedicated runner; no timing rows are fabricated in their absence.
The manual publication workflow runs only on a labeled dedicated self-hosted
runner, collects five independent correctness-gated records, validates their
dispersion, renders the trend report into the job summary, and retains the
records, publication, checksums, reference, and Markdown report for 90 days.

### P1.4 Differential minimization

- [x] Build a line-set delta debugger that minimizes failures while preserving
  the selected options and observed mismatch.
- [x] Minimize coordinate complexity after feature/segment minimization.
- [x] Preserve source IDs and Z conflicts during minimization.
- [x] Define and validate a strict persisted golden and compatibility
  classification for minimized failures.
- [x] Wire each fuzz or production mismatch producer to emit a reviewed
  persisted-fixture candidate.
- [x] Export a standalone repro bundle containing input, options, versions,
  fingerprint, reference metrics, and witness.

**Done when:** a fuzz or production mismatch can become a checked-in, human-sized
fixture without manually rewriting the geometry.

Progress: A deterministic, doc-hidden line-set ddmin kernel now accepts the
caller's exact mismatch predicate, allowing each adapter to retain its selected
options and recompute both sides for every candidate. Retained segments are
copied unchanged. Shared X/Y values can then be simplified atomically while
source IDs and Z conflicts remain untouched; persisted fixtures and standalone
repro bundles use an exact, versioned JSON representation. Persisting novel
minimized failures no longer requires rewriting geometry: a versioned persisted
fixture now pairs the exact repro bundle with a strict compatibility
classification and validated stable case ID.
The strict persisted-differential corpus is now exercised in CI by decoding
the exact IEEE-754 input, rerunning the recorded options, requiring byte-exact
fingerprint equality, and checking compatibility-classification evidence. It is
seeded with the existing documented floating-microfaces GEOS divergence.
Scheduled fuzzing now compares one-shot and reusable-workspace fingerprints or
normalized errors, runs the persisted corpus before fuzzing, and retains
libFuzzer failure artifacts for admission. The reviewed admission command now
validates a candidate with the strict Rust corpus runner before exclusively
creating its case-ID-named JSON file; existing fixtures cannot be overwritten.
Persisted differential V2 fixtures retain success, normalized-error, and mixed
outcome pairs without changing V1 bytes, and reviewed admission rejects unknown
producers, matching outcomes, and implicit overwrites.
The adapter differential target—the current mismatch producer—shares one raw
artifact decoder and comparator with a standalone replay command. Its scheduled
workflow minimizes every retained mismatch into a review candidate and uploads
the raw and prepared artifacts together. The remaining fuzz targets are crash
and ingestion-safety producers, not two-sided mismatch producers; no production
mismatch producer currently exists.

### P1.5 Trace schema

Add an opt-in, versioned, bounded trace with zero meaningful overhead when
disabled. Trace levels should be selectable so callers do not need to serialize
the entire pipeline.

Candidate events and snapshots:

- [x] normalized input segments and source IDs;
- [x] snapped coordinates, fixed-grid cells, and certified hot pixels;
- [x] candidate pairs and exact intersection/split witnesses;
- [x] noded and dissolved edges with complete source sets;
- [x] graph nodes and directed halfedges;
- [x] dangle pruning and cut-edge classification;
- [x] maximal rings, minimal rings, and invalid-ring reasons;
- [x] shell/hole classification and containment candidates;
- [x] canonical ordering decisions;
- [x] tile ownership and deduplication decisions;
- [x] tile retry and fallback decisions.

Trace output must include byte limits, truncation metadata, schema version,
library version, and canonical options.

Progress: Trace V1 now has explicit summary, noding, graph, ring, and full
levels plus deterministic event sequencing. Its recorder bounds serialized
event bytes, reports truncation, includes version/options metadata, and is not
constructed when tracing is disabled. Pipeline event wiring remains incremental
work. The owned traced entrypoint now records the validated canonical `Line3D`
input representation with exact coordinates and source IDs before noding. The
post-build graph snapshot records exact nodes, noded/dissolved edges with every
source ID, and both directed halfedges before pruning mutates graph state.
Dangle and cut-edge events then capture the exact linework returned by their
classification passes before canonical output sorting. Noding traces also
record the physical post-pre-snap and post-noding segment coordinates, including
fixed-grid output. Certified noding now records the exact sorted hot-pixel set
with integer grid coordinates from the physical snap-rounding pass. Uniform-grid
candidate cells now retain their exact bounds, iteration, segment/source
membership, and global-line fallback membership. The same certified pass also
records every candidate segment pair with source IDs and its exact point,
collinear, or empty intersection witness. Floating SIMD candidate scans now emit
the same evidence from their physical exact-predicate calls; uniform-grid
cell-pair scans now record exact witnesses and whether the cell owned the
result. Global-line fallback scans emit the same exact candidate evidence with
an explicit scan origin. Certified replacement segments additionally retain
their source segment, source ID, and exact emitted endpoints from the physical
split loop. Floating SIMD and uniform-grid noding now record the same replacement
evidence from their shared physical split loop. Wasm now exposes the same
bounded trace and canonical topology report through a dedicated non-semantic
entrypoint so browser tooling does not reconstruct pipeline evidence.
Physical Z reconciliation now records exact XY/Z bit patterns, ordered
candidate/source multiplicity, the selected policy and tolerance, conflicts,
and retained Z. Summary traces reuse the existing phase timings, work counters,
diagnostics, and trace-budget accounting rather than rescanning the pipeline.

### P1.6 Turn the playground into a topology debugger

- [x] Add the playground prominently to the docs navigation.
- [x] Support paste, upload, drag-and-drop, drawing, and fixture selection.
- [x] Expose the canonical options schema, including `Validate` and
  `CertifiedFixedPrecision`.
- [x] Add layer toggles for raw lines, snapped lines, hot pixels, split points,
  graph edges, dangles, cut edges, invalid rings, shells, holes, and final faces.

Progress: the playground now runs the bounded V1 trace in an abortable worker
and toggles raw lines, physical snapped lines, certified hot pixels, exact point
witnesses, dissolved graph edges, and every retained report layer. Trace event
count, byte use, and truncation state remain visible alongside canonical options.
Snapped lines, split witnesses, graph edges, shells, and holes are clickable for
source and representative-edge provenance; rings also show their exact retained
Z bit patterns. Physical Z reconciliation and execution-summary evidence are
rendered directly from the bounded trace, including exact retained bits,
conflicts, timings, work counters, resource budgets, and normalized validator
witnesses.
Small V1 repro envelopes now encode input plus canonical noding controls as
deterministic UTF-8 base64url query data, reject payloads beyond 8 KiB, and load
directly into the worker-backed debugger.

- [x] Make edges/rings clickable to inspect source provenance and retained Z.
- [x] Trace and inspect physical Z reconciliation decisions.
- [x] Show phase timings, work counters, resource budgets, and validator witnesses.
- [x] Compare two option profiles side by side.
- [x] Encode small deterministic repros in shareable URLs.
- [x] Export an exact golden/compatibility fixture bundle.
- [x] Run differential minimization in a worker and visualize each reduction.

Exact minimized segments retain coordinate bit strings and source IDs in the
downloaded fixture, together with both canonical profile outcomes, the preserved
witness, and an explicit compatibility classification. The existing disposable
worker streams every accepted segment/coordinate reduction to the selectable map
overlay and terminates on completion or abort.

**Done when:** a user can move from a failing input to a minimized, exportable
fixture while seeing which topology stage changed the result.

## P2 — Arrangement model and adaptive algorithms

The current graph is half-edge-like and efficient, but it does not yet retain an
explicit face/arrangement model. Build this internally before starting overlay,
shared-edge simplification, or incremental topology.

### P2.1 Arrangement validator

Add a debug/test validator for the live graph:

- [x] twin symmetry is an involution;
- [x] each twin reverses source and destination;
- [x] edge, adjacency, and degree counts agree;
- [x] no live adjacency references a deleted edge;
- [x] every live topology edge has a nonempty, sorted source set;
- [x] angular adjacency order is deterministic;
- [x] ring cycles close and do not accidentally reuse directed edges;
- [x] every directed edge is assigned to the expected maximal/minimal cycle;
- [x] the planar Euler relation `V - E + F = C + 1` holds where the stage
  preconditions apply, including the unbounded face.

The debug/test maximal-ring validator derives face count from closed boundary
cycles and connected components, includes the shared unbounded face, and rejects
an unnoded crossing with a deterministic count witness. Automatic graph-stage
checks run after validated noding postconditions; unchecked profiles retain the
cycle validator because they do not promise planar input. The validated core
pipeline fuzz target exercises the Euler check.

Run it in tests, fuzz targets, and optionally diagnostic builds—not in the
default release hot path.

### P2.2 Explicit `next` and face identity

- [ ] Derive and store directed-edge `next` links after angular ordering.
- [ ] Assign deterministic face/cycle IDs.
- [ ] Identify the unbounded face explicitly.
- [ ] Retain mappings from faces to boundary source sets and Z decisions.
- [ ] Compare the explicit face walk against the current ring extractor on the
  entire golden corpus before replacing anything.
- [ ] Keep the arrangement private until overlay-quality invariants are proven.

This creates a DCEL-like internal arrangement without committing the public API
to a specific graph representation.

### P2.3 Connected-component decomposition

Decompose the fully noded, dissolved graph before expensive graph-local work:

- [x] identify connected components deterministically;
- [ ] perform component-local dangle pruning, cut-edge classification, edge
  sorting, and ring extraction in parallel;
- [ ] merge component results into deterministic global order;
- [ ] reuse component-local scratch buffers and measure peak memory;
- [ ] evaluate flat/CSR adjacency versus `Vec<Vec<_>>` using real workloads.

Debug/test active nodes now receive component IDs ordered by each component's
lexicographically smallest XY coordinate, independent of insertion order. The
Euler validator reuses this identification; promoting it into graph-local
execution remains a separate slice.

Important: disconnected graph components can still be spatially nested. Shell
and hole containment must remain global unless components are grouped by a
proven, disjoint-envelope partition. Do not independently finalize polygons per
graph component and thereby lose nesting relationships.

**Promotion gate:** exact canonical equivalence across all fixtures and bindings,
plus a predeclared end-to-end or peak-memory win on representative
multi-component data.

The existing public backend surface should remain unchanged during research.
Certified fixed precision remains the hot-pixel contract.

### P2.4 Candidate-enumeration boundary

- [ ] Separate broad-phase candidate enumeration from robust exact
  intersection, split accumulation, normalization, and dissolve.
- [ ] Preserve source line-string/segment-string boundaries internally; flattened
  independent segments discard the monotone-chain structure needed by some
  indexes.
- [ ] Add deterministic workload descriptors:
  - segment and line-string counts;
  - average/max chain length;
  - envelope and grid occupancy;
  - candidate and split density;
  - collinear-overlap incidence;
  - coordinate span and grid scale.
- [ ] Feed all experimental candidates through the independent validator and the
  same split/dissolve path.

### P2.5 Evaluate sparse and long-line backends

Prototype internally, without a public enum variant:

- [ ] the existing `geo::Intersections` Bentley–Ottmann sweep implementation for
  sparse-intersection workloads;
- [ ] a monotone-chain indexed candidate generator inspired by JTS
  `MCIndexNoder` for long sparse polylines;
- [ ] current SIMD brute-force and uniform-grid paths as baselines;
- [ ] connected-component-local candidate generation where decomposition helps.

Document overlap, degeneracy, determinism, provenance, Wasm, and parallelism
behavior for every prototype.

### P2.6 Backend promotion gate

A backend or dispatch rule may become production-visible only when:

- [ ] it has zero unexpected validator failures in the golden, compatibility,
  real-world, and fuzz corpora;
- [ ] it preserves canonical output, source sets, Z behavior, and errors;
- [ ] its end-to-end effect exceeds a predeclared meaningful threshold on more
  than one representative workload and supported architecture;
- [ ] it causes no material regression outside its target workload;
- [ ] its maintenance and compile-time costs are documented;
- [ ] its dispatch inputs are deterministic and inspectable.

Prefer simple benchmark-derived rules over opaque runtime learning or
autotuning.

### P2.7 Integerized fixed-grid experiment

Evaluate representing certified fixed-grid XY coordinates as checked integers
internally:

- [ ] convert using a documented origin/scale and checked `i64` arithmetic;
- [ ] detect overflow before topology work;
- [ ] benchmark hashing, equality, ordering, graph construction, and hot-pixel
  operations;
- [ ] prove round-trip behavior at supported coordinate/grid ranges;
- [ ] keep Z and source payloads separate from XY topology identity.

Do not silently clamp, wrap, or reduce precision.

### P2.8 Explicit robustness fallback profile

A staged fallback may be useful for applications that prefer a result over a
single fixed policy, but it must be opt-in and fully observable.

Possible ordered attempts:

1. floating noding plus validation;
2. caller-authorized snap tolerance;
3. bounded self-snap retries;
4. caller-authorized fixed precision;
5. certified hot-pixel noding and validation.

- [ ] Record every attempted policy and failure witness.
- [ ] Never mutate the caller’s precision contract silently.
- [ ] Return the effective policy in the report.
- [ ] Bound retries using the execution policy.
- [ ] Add compatibility fixtures for fallback selection.

## P3 — Production tiling, streaming, and `1.0`

Treat two different algorithms separately:

1. **replicate-and-own tiling** with a bounded halo;
2. **true boundary-graph stitching** across partitions.

### P3.1 Coverage validation for replicate-and-own tiling

Progress: tile and stitching reports now retain definite owned-face halo escapes
with internal boundary sides, polygon envelopes, representative edge source IDs,
and complete aggregate source IDs when provenance is enabled. Conservative
input-boundary evidence now retains stable input geometry indexes, envelopes,
and internal halo sides even when no local face was reconstructed. Missing
regions formed by separate input geometries that share exact endpoints or, when
input noding is enabled, exact segment intersections now produce conservative
excluded-component evidence when their combined envelope intersects a halo but
no member geometry does. Bounded full traces distinguish endpoint-only and
segment-intersection component evidence, so this specific missing-region gap is
covered while the broader coverage-detection rows remain open. Minimized
pre-snap and fixed-grid fixtures also pin the remaining transformation boundary:
separate inputs that connect only after caller-selected precision transformations
are now grouped as conservative component evidence; region coverage remains
observational rather than certified. A minimized certified fixed-grid fixture
also pinned a hot-pixel boundary: the noder could connect a segment through an
endpoint hot pixel even when the tiled preflight's transformed straight
segments had no exact intersection, so that region could remain absent without
observed evidence. The preflight now reuses the bounded certified noder for
that same connectivity case. A partially observed transformed-connected
component, where one member geometry intersects a halo and other members do
not, is now reported as conservative excluded-component evidence.
Opt-in component fallback also recovers a closed single-input boundary region
when retained-face or input-boundary evidence reaches a halo; mixed or open
single-geometry boundaries decline conservatively, so broad missing-region
coverage detection remains observational. A single closed geometry whose
envelope only grazes the ownership domain can also remain absent when its
ownership point falls outside every tile; this pins the remaining envelope-only
boundary without adding an implicit clipping contract. Reconstructed faces
whose ownership points fall outside the configured domain are now reported as
definite ownership-domain evidence when their envelopes overlap that domain;
BestEffort remains domain-scoped, while strict observed validation and the
opt-in whole-input fallback handle the evidence explicitly.

- [ ] Detect owned faces or connected regions that touch an unresolved halo or
  partition boundary.
  - [x] Pin the excluded-component gap where untiled output contains a face but
    no member geometry intersects any tile halo.
  - [x] Report conservative excluded-component evidence for separate input
    geometries connected by exact shared endpoints; intersection-connected
    components remain open.
  - [x] Pin the excluded-component gap for separate input geometries connected
    only through segment-interior intersections.
  - [x] Classify exact segment-intersection components with one indexed global
    candidate pass when input noding is enabled.
  - [x] Trace excluded segment-intersection components without rescanning input.
  - [x] Apply execution-policy segment, candidate, exact-predicate, and
    cancellation bounds during the indexed component preflight.
  - [x] Pin the undetected connected-region case where separate inputs connect
    only after caller-enabled pre-snap.
  - [x] Report caller-enabled pre-snap-connected components distinctly in tile
    reports and bounded traces; broader region coverage detection remains open.
  - [x] Pin and report the corresponding fixed-grid-connected component case
    with bounded trace evidence; broader region coverage detection remains open.
  - [x] Pin the certified fixed-grid hot-pixel connected-region gap; detection
    and bounded evidence now reuse the certified noder.
  - [x] Use indexed connected-component evidence to identify a component that
    is only partially observed in a tile halo; broad undetected-region coverage
    remains open.
  - [x] Report conservative excluded-component evidence when one
    transformed-connected member intersects a halo and other members remain
    outside it.
  - [x] Pin the partially observed fixed-grid-connected component contract,
    including certified fixed-grid noding; broader transformed-region coverage
    remains observational.
  - [x] Pin the single-geometry envelope-only gap where an untiled face's
    ownership point falls outside the configured ownership domain; broad
    missing-region detection remains open.
  - [x] Report reconstructed faces that overlap the ownership domain but have
    no tile-owned ownership point; this remains evidence rather than implicit
    clipping.
  - [x] Pin that caller-enabled whole-input fallback cannot repair the same
    single-geometry ownership-domain gap without observed evidence; no implicit
    clipping is introduced.
- [x] Report source IDs and boundary evidence that were required but not fully
  observed.
  - [x] Distinguish representative edge IDs from complete aggregate boundary
    source IDs explicitly in tile reports and bounded traces.
- [x] Add explicit guarantee levels such as `BestEffort` and
  `ValidateCoverage`; do not rename best effort as certified.
  - [x] Add `BestEffort` and `ValidateOwnedFaces` for the reconstructed-face
    subset; missing-region coverage certification remains open.
  - [x] Add `ValidateObservedCoverage` to reject either owned-face or
    conservative input-boundary evidence without claiming certification.
  - [x] Extend `ValidateObservedCoverage` to reject conservative excluded
    exact-endpoint component evidence without claiming certification.
  - [x] Reject conservative excluded exact-intersection component evidence
    without claiming certification.
- [x] Randomize tile origins, sizes, input orders, and tile traversal order in
  deterministic metamorphic tests with sufficient halos.
  - [x] Exercise bounded in-domain tiled-vs-untiled ownership and deduplication
    inputs in the scheduled fuzz target; a mismatch without observed coverage
    evidence fails the target rather than being silently accepted.
  - [x] Exercise deterministic adversarial closed-face, hole, concavity, and
    dirty-overlap families across shifted ownership origins, tile sizes, input
    rotations, and reversals; an in-domain mismatch without observed evidence
    fails the contract.
  - [x] Exercise the scheduled tiled differential probe with `MultiLineString`
    containers as well as per-segment geometries; a topology mismatch without
    observed evidence fails the target.
- [x] Expand exact tiled/untiled fixtures for nested disconnected rings, long
  faces, narrow concavities, holes crossing boundaries, dangles, cut edges,
  overlaps, and dirty boundary intersections.
  - [x] Cover nested disconnected rings, long faces, narrow concavities,
    boundary-crossing holes, overlaps, and dirty crossings under sufficient
    halos, plus exact one-tile dangle and cut-edge family counts.

### P3.2 Deterministic recovery

When coverage cannot be proven:

- [x] retry only the unresolved tile region with a caller-bounded larger halo;
- [ ] fall back to untiled processing for the unresolved connected region;
  - [x] Pin the global-containment boundary where an excluded outer component
    spatially nests an already retained tile-local face.
  - [x] Add opt-in envelope-closed component fallback that groups interacting
    input, retained-face, and connected-component envelopes and records the
    recovery, including input-boundary-seeded components.
  - [x] Recover a halo-crossing region seeded by a caller-owned closed input
    geometry; decline mixed or open single-geometry regions with deterministic
    evidence rather than appending an unsafe partial result.
  - [x] Recover a nested retained face by polygonizing the closed region once;
    whole-input fallback remains the escape hatch for evidence outside that
    region.
  - [x] Seed the same region-local recovery from input-boundary evidence when
    the indexed connected component is only partially visible in a halo.
  - [x] Recover an indexed component with mixed observed and unobserved
    members while replacing nested retained output before canonical merge.
  - [x] Recover mixed owned-face and indexed component/input-boundary evidence
    only when every owned-face witness lies inside an envelope-closed recovery
    region; decline conservatively when any witness is outside that region.
  - [x] Merge one envelope-disjoint recovered component with retained tile
    output; envelope-closed region replacement now covers interacting retained
    output.
  - [x] Merge multiple pairwise envelope-disjoint recovered components with
    retained tile output; cross-component overlap and graph stitching remain
    open.
  - [x] Apply the configured output limit to component recovery and observe
    cancellation before fallback selection and each recovered component;
    envelope-closure region-selection and retained-region replacement,
    recovered-output merge, and deduplication cancellation are now pinned.
  - [x] Enforce the configured aggregate output limit and cancellation during
    the final canonical merge after tile and region recovery.
  - [x] Observe cancellation during tile input/evidence/ownership filtering,
    including the empty-tile early-return path.
  - [x] Observe cancellation while scanning connected-component member
    envelopes for partial component evidence.
  - [x] Record retained tile polygon counts in component-fallback trace events
    and expose retained/recovered aggregate counts in `StitchingReport`;
    expose replaced-retained counts for canonical region replacement.
  - [x] Provide a caller-enabled whole-input untiled fallback that preserves
    global containment when retries leave observed coverage unresolved.
  - [x] Verify whole-input fallback equality with untiled output across tile
    sizes and input-order permutations.
  - [x] Apply the configured execution policy to whole-input fallback and pin
    exact resource-limit evidence.
- [ ] merge the fallback result canonically with already validated regions;
  - [x] Replace retained polygons intersecting an envelope-closed recovery
    region before canonical deduplication and deterministic ordering.
  - [x] Route non-envelope-closed recovery evidence through the caller-enabled
    whole-input fallback, preserving global containment and tracing the decline;
    region-local reconciliation remains open.
  - [ ] Reconcile fallback regions whose topology crosses a partition boundary
    without a conservative envelope closure.
- [x] stop with a typed coverage error when the configured retry budget is
  exhausted;
- [ ] record every retry and fallback in the stitching report and topology trace.
  - [x] Record deterministic retry attempts and exhausted tiles in tile and
    stitching reports and bounded `tile_halo_retry` trace events.
  - [x] Record whole-input fallback use in the stitching report and a bounded
    `tile_untiled_fallback` trace event; broader declined-fallback coverage
    remains open.
  - [x] Record envelope-closed component/region recovery and retained-polygon
    replacement counts in the stitching report and bounded component-fallback
    trace events.
  - [x] Record enabled-but-declined component recovery in the stitching report
    and bounded `tile_component_fallback_declined` trace events with a
    deterministic reason.
  - [x] Pin an envelope-closed recovery decline for empty polygonization output,
    including report, trace, typed error, and global-fallback ordering; broader
    declined-fallback coverage remains open.
  - [x] Expose the deterministic component-fallback decline reason directly in
    `StitchingReport` as well as the bounded trace event.
  - [x] Preserve that decline reason in typed `CoverageIncomplete` errors.

### P3.3 True graph stitching

After the explicit arrangement model exists:

- [ ] define canonical partition-border node and edge keys;
- [ ] match and reconcile twin boundary halfedges;
- [ ] merge source sets and Z decisions across partitions;
- [ ] reconcile connected components before face extraction;
- [ ] validate the stitched arrangement and its unbounded face;
- [ ] compare exact canonical results with untiled execution.

Promote tiling from hidden experimental API only after a documented input class
has either a validated equivalence contract or a deterministic fallback.

### P3.4 Streaming and out-of-core execution

Begin only after tiling coverage/recovery is credible.

- [ ] Define streaming ingest for Arrow `RecordBatch`, GeoParquet row groups, and
  FlatGeobuf features with bounded memory and backpressure.
- [ ] Preserve source IDs and input profile metadata across chunks.
- [ ] Separate stream partitioning from topology partitioning.
- [ ] Add resumable manifests containing checksums, options, partition state, and
  library version.
- [ ] Evaluate disk-backed or memory-mapped indexes only after profiling shows a
  concrete memory bottleneck.
- [ ] Measure total I/O, peak RSS, temporary storage, recovery behavior, and
  output equivalence—not only kernel time.

This milestone supersedes the broad intent of issue #672.

### P3.5 Stable support policy

- [x] Freeze the stable root facade; keep research backends and graph internals
  private.
- [x] Remove expired aliases and transitional mutable configuration paths.
  `with_snap_grid`, `NodingBackend::Advanced`, and
  `TileOwnershipPolicy::CanonicalBoundaryHash` are removed; mutable options are
  construction-only outside core tests.
- [x] Publish MSRV, target, feature-matrix, and platform support policies.
- [x] Define semver, deprecation, and migration windows.
- [x] Enforce synchronized crates.io, npm, and PyPI release state.
- [x] Document panic, cancellation, resource-limit, and thread-safety behavior.

### P3.6 Complete production documentation

Add or finish dedicated guides for:

- [x] topology and output semantics;
- [x] floating, fixed, validated, and certified noding guarantees;
- [x] Z and provenance behavior;
- [x] compatibility profiles and known divergences;
- [x] tiling guarantees and fallback behavior;
- [x] Wasm memory lifetime, workers, threads, and cancellation;
- [x] Python memory/GIL behavior;
- [x] Arrow C ABI ownership and error retrieval;
- [x] benchmark methodology and how to reproduce claims.

Compile examples, run rustdoc with warnings denied, check links, and ensure the
interactive debugger uses the published package rather than repository-only
shortcuts.

### P3.7 Verification matrix

- [x] Run canonical equality across serial and parallel native builds.
- [x] Run cross-binding conformance on every stable entrypoint.
- [x] Run the certified corpus with zero residual noding failures.
- [x] Run scheduled differential fuzzing and persist novel minimized cases.
- [x] Add selective Miri and sanitizer jobs for unsafe/FFI boundaries where the
  dependencies support them.
- [x] Test minimal, default, all-feature, Wasm scalar/SIMD/threaded, and supported
  Python ABI combinations.
- [x] Verify every public error family has a documented and tested construction
  path.

Progress: Default and no-default builds consume the same canonical success and
normalized-error snapshot while test-only path evidence proves that the default
build enters Rayon graph construction. Stable Rust, Python, Wasm, Arrow, and C
entrypoints now compare the complete fingerprint/error contract or an explicit
retained-field projection for lossy APIs. Every manifest workload permitting the
certified-fixed profile runs its declared certified options and currently has
zero residual noding failures. Scheduled differential fuzzing now converts every
retained adapter-differential failure into a deterministic minimized candidate,
uploads the raw input and review candidate together for 30 days, and leaves
classification and checked-in admission to the existing reviewed command. The
selective Miri job runs the bounded-execution core tests without default
parallel features under a pinned nightly; filesystem-backed corpus tests remain
outside Miri rather than disabling its isolation. A pinned AddressSanitizer job
checks Arrow integration and raw C ownership/error paths on Linux. The error
construction matrix is exhaustive. Unreachable `TopologyFailure` and
`NullPointer` core variants were removed before `1.0`; null C pointers retain
their direct `InvalidArgument` ABI status. `InternalInvariantViolation` remains
a tested bug sentinel and `Panic` remains boundary-only.

### `1.0` exit criteria

`1.0` is eligible when:

- all stable cross-binding conformance tests pass;
- certified fixed-precision mode has no unexplained validator failure in the
  public corpus and documented fuzz budget;
- canonical output agrees across supported serial/parallel targets;
- execution budgets, cancellation, and FFI ownership have tested contracts;
- the real-world benchmark report is reproducible and correctness-gated;
- no known critical or high-severity correctness issue is open;
- the support, semver, and synchronized release policies are enforced.

# Post-`1.0` capability tree

The existing broad feature issues remain useful, but they need explicit
dependencies and narrower scopes.

| Existing issue | Reframed scope | Required predecessors |
|---|---|---|
| [#720 — graph-native Boolean overlay](https://github.com/graydonpleasants/geo-polygonize/issues/720) | Winding-labeled overlay on an explicit arrangement; start with two-input union/intersection and an overlay-specific compatibility corpus. | P2 explicit faces, arrangement validator, robust noding, provenance algebra. |
| [#714 — topology-preserving simplification](https://github.com/graydonpleasants/geo-polygonize/issues/714) | Simplify shared edge chains once, then rebuild/validate all incident faces. | P2 face model, shared-edge identity, arrangement validator. |
| [#688 — robust buffering](https://github.com/graydonpleasants/geo-polygonize/issues/688) | Offset-curve generation followed by certified noding and face selection. Remove the obsolete assumption that it depends on the retired sweep prototype. | Certified noding, arrangement/overlay face selection, dedicated buffer corpus. |
| [#697 — MVT and TopoJSON](https://github.com/graydonpleasants/geo-polygonize/issues/697) | Downstream adapter crates with topology-preserving quantization and shared-edge encoding. | Stable topology-preserving simplification and real consumers; not a core-kernel concern. |
| [#663 — incremental topology](https://github.com/graydonpleasants/geo-polygonize/issues/663) | Separate experimental arrangement API with stable component/face IDs, local invalidation, and delta reports. | P2 explicit arrangement, component decomposition, mutation invariants. |
| [#769 — geodesic polygonization](https://github.com/graydonpleasants/geo-polygonize/issues/769) | Separate spherical/ellipsoidal kernel or crate. Do not add it as a mode inside the planar precision model. | A written geodesic topology contract, anti-meridian/polar corpus, robust spherical predicates. |
| [#664 — database adapters](https://github.com/graydonpleasants/geo-polygonize/issues/664) | Consumer-driven DuckDB/PostGIS adapters around stable Arrow/streaming contracts. | P3 streaming, stable ABI, concrete users and deployment benchmarks. |
| [#771 — GPU point-in-polygon](https://github.com/graydonpleasants/geo-polygonize/issues/771) | GPU broad-phase or batch predicate experiment only after profiling proves transfer/setup amortization. Keep CPU validation and fallback mandatory. | Public large-workload corpus, traceable candidate boundary, measured CPU bottleneck. |

Distributed Ray/Dask/Spark integration should follow the same rule as database
adapters: build it only after the streaming partition contract exists and a real
consumer supplies representative data.

# Recommended PR sequence

This is the suggested dependency-respecting order for the next work:

1. Define canonical topology fingerprint and normalized errors.
2. Build Rust/Python/Wasm/Arrow cross-binding conformance.
3. Align Wasm polygon-only and full-report API contracts.
4. Add schema/report versions and release synchronization gates.
5. Add execution budgets and adversarial amplification tests.
6. Add native/Python/Wasm cancellation contracts.
7. Version and document the C ABI.
8. Establish the public real-world corpus manifest.
9. Split benchmark comparisons into equivalent correctness-gated lanes.
10. Add machine-readable benchmark reports and trend publishing.
11. Add automatic mismatch minimization and repro bundles.
12. Define the bounded topology trace schema.
13. Upgrade the playground into the interactive debugger.
14. Add the internal arrangement validator.
15. Derive explicit `next` links, face IDs, and the unbounded face.
16. Add connected-component-local graph processing with global containment.
17. Instrument workload descriptors and candidate-enumeration boundaries.
18. Evaluate sweep-line and monotone-chain candidate generators.
19. Evaluate checked integer fixed-grid topology.
20. Add tiling coverage validation and deterministic recovery.
21. Implement true boundary-graph stitching if evidence justifies it.
22. Add streaming ingest and only then evaluate out-of-core indexes.
23. Close the `1.0` support, documentation, and verification gates.

# Research references

These references describe useful contracts and candidate algorithms; they are
not dependencies or automatic implementation choices.

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

- Keep core behavior expressible through the canonical semantic options schema
  across Rust, Python, and Wasm.
- Keep execution budgets and cancellation separate from semantic options.
- Preserve deterministic canonical output and structured, actionable errors.
- Preserve complete source provenance through noding, dissolve, graph
  decomposition, tiling, and future topology operations.
- Treat tiled polygonization as experimental until its equivalence, coverage,
  and recovery contracts are closed.
- Do not claim robustness beyond the selected noding policy’s checked
  postconditions.
- Do not silently fall back to another precision or noding guarantee.
- Do not accept a performance win before its correctness gate passes.
- Do not add a stable API path without cross-binding conformance coverage where
  an equivalent path exists.
- Ensure every hard failure can emit a bounded witness or reproducible trace.
- Add the smallest strict regression that would have caught each bug.
