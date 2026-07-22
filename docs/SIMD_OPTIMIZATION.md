# SIMD and Data-Parallel Optimization Playbook

This document is the implementation supplement for the SIMD and adaptive
optimization work in [ROADMAP.md](../ROADMAP.md). It is intentionally detailed
enough for an agent to design, benchmark, review, and either promote or reject an
optimization without inventing a new correctness contract.

## Status and scope

This is an evidence-gated research plan, not a promise to add another public
backend.

The current production contract remains:

- certified fixed-precision noding uses the existing hot-pixel implementation and
  independent full-noding validation;
- iterative grid noding remains explicitly unchecked unless validation is
  requested;
- canonical output, complete provenance, Z policy behavior, typed errors, and
  cross-binding semantics must not change;
- new SIMD code stays internal until it passes the backend promotion gates in
  this document and `ROADMAP.md`.

The primary source of inspiration is Erin Catto's July 2026
[SIMD for Collision](https://box2d.org/posts/2026/07/simd-for-collision/)
article. The transferable lesson is not “use AVX2.” It is:

> Process several independent geometric work units in parallel, use a layout that
> keeps lanes full, count setup and merge costs, and retain scalar paths for
> simple or irregular work.

Box3D benefits when thousands of independent edge combinations amortize SoA
packing and mask handling; simple box-box collision does not. `geo-polygonize`
should expect the same pattern: wide work may help dense candidate batches or
many repeated containment queries, while small and sparse inputs should remain
scalar or use a better candidate algorithm.

## Current baseline

Agents should understand the existing implementation before proposing a new
kernel.

### Noding broad phase

`crates/geo-polygonize-core/src/utils/soa.rs` stores segment bounding boxes as
four arrays:

```text
min_x
min_y
max_x
max_y
```

`SoALines::intersects_bbox_batch_splatted` loads four target boxes and returns a
four-bit overlap mask.

`SnapNoder::check_intersection_simd` in
`crates/geo-polygonize-core/src/noding/snap.rs` is already a wide-work pattern:

```text
one query segment × four target segment bounding boxes
```

For each active lane, however, it currently calls the scalar
`geo::line_intersection` path. Therefore the first useful exact-predicate
experiment is a narrow extension of the current design rather than a wholesale
noder rewrite.

### Point-in-ring

`crates/geo-polygonize-core/src/utils/simd.rs` currently evaluates:

```text
one probe point × four consecutive ring edges
```

This is edge-wide SIMD. Runtime selection already depends on architecture and
ring size; Linux AArch64 currently selects scalar, while some shorter rings on
x86-64 select the portable-wide implementation. Those choices were measured and
recorded in `ROADMAP.md`.

A distinct experiment is point-wide containment:

```text
four or eight probe points × one prepared ring
```

This may win when many holes or shell probes query the same prepared shell.

### Existing benchmark locations

Extend rather than replace these suites:

- `crates/geo-polygonize-core/benches/polygonize_bench.rs`
  - end-to-end grids, bowties, random lines, forced grid/SIMD noding;
- `crates/geo-polygonize-core/benches/hole_sort_bench.rs`
  - scalar/wide point-in-ring crossovers, prepared locator costs, containment;
- `crates/geo-polygonize-core/benches/iai_bench.rs`
  - instruction-count and cache-sensitive comparisons;
- scheduled architecture runs and machine-readable benchmark artifacts described
  in `ROADMAP.md`.

Kernel microbenchmarks are necessary for diagnosis, but promotion is based on
correctness-gated end-to-end results.

## Non-negotiable principles

### 1. Vectorize independent work, not coordinate components

Prefer lanes that each represent a candidate pair or query. Putting one point's
`x` and `y` into adjacent lanes is usually narrow SIMD and rarely exposes enough
independent work.

### 2. Reduce the work before making it wider

Connected-component decomposition, uniform-grid filtering, monotone chains,
sweep enumeration, duplicate removal, and overlap normalization can remove
orders of magnitude more work than SIMD.

A lower candidate count outranks a faster evaluation of unnecessary pairs.

### 3. Setup is part of the algorithm

Count all of the following:

- SoA or AoSoA preparation;
- candidate sorting and compaction;
- mask extraction;
- scalar tails;
- robust fallback;
- split-event emission;
- thread-local buffer allocation;
- deterministic merge and sort;
- extra memory traffic.

Do not report only the innermost vector loop.

### 4. Robust scalar fallback is a feature

Wide floating-point arithmetic may cheaply classify obvious cases. It must not
replace adaptive or robust handling for ambiguous cases merely to increase lane
utilization.

Near-zero determinants, collinear overlap, endpoint touch, precision-grid
boundary cases, overflow risk, and any unsupported lane must fall back to the
existing validated scalar path.

### 5. SIMD and threading must compose

Each worker should process independent candidate batches into thread-local event
and fallback buffers. Global atomics or a shared event vector inside the
predicate loop are presumptively wrong.

The merge must be deterministic and produce the same canonical event order as
serial execution.

### 6. ISA width is an implementation detail

Do not add `Avx2`, `Avx512`, `Neon`, or lane width to `PolygonizerOptions`.

The semantic options select topology behavior. Internal runtime dispatch may
select scalar or wide kernels using measured, deterministic workload
descriptors.

### 7. Separate compiler/ISA gains from explicit kernels

Benchmark independently:

1. scalar baseline built for the generic target;
2. the same implementation built with an architecture-enabled target;
3. portable-wide or explicit-intrinsic implementation.

This avoids crediting a custom kernel for gains produced by the compiler's
target selection.

### 8. Every result returns to one shared topology path

Experimental candidate and predicate kernels must feed the existing:

```text
split accumulation
→ normalization
→ coincident-edge dissolve
→ graph build
→ polygonization
→ independent validation
```

Do not fork provenance, Z interpolation, overlap normalization, or error
semantics per SIMD backend.

## Dependencies and ordering

Do not start implementation before the following roadmap foundations exist:

1. canonical topology fingerprints and normalized errors;
2. correctness-gated public workloads;
3. phase-level tracing and work counters;
4. connected-component decomposition where it provides independent work;
5. the P2 candidate-enumeration boundary separating broad phase from exact
   predicates and split/dissolve.

After those foundations, execute the experiments below in order. Stop when an
experiment fails its predeclared promotion threshold.

## Target architecture

### Stable candidate identity

Every candidate pair needs a stable ID independent of thread scheduling and lane
packing.

For source segment indices `left` and `right`:

```rust
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct CandidatePairId {
	left: u32,
	right: u32,
}

impl CandidatePairId {
	fn new(left: u32, right: u32) -> Self {
		Self {
			left: left.min(right),
			right: left.max(right),
		}
	}
}
```

Use wider indices if corpus measurements show that `u32` is insufficient. Reject
overflow rather than truncating.

### Width-independent batch contract

The candidate producer should not depend on SSE, AVX, NEON, Wasm SIMD, or the
`wide` crate's current lane width.

A conceptual internal contract:

```rust
struct CandidateBatch<const LANE_COUNT: usize> {
	pair_ids: Array<CandidatePairId, LANE_COUNT>,
	left_indices: Array<u32, LANE_COUNT>,
	right_indices: Array<u32, LANE_COUNT>,
	active_lane_count: usize,
}
```

The concrete implementation may use arrays, `SmallVec`, fixed blocks, or a
backend-specific wrapper. The important properties are:

- stable pair identity;
- deterministic lane order;
- an explicit active mask or active count;
- no public exposure;
- cheap reuse from `PolygonizerWorkspace`;
- provenance and Z payloads remain addressable through stable segment indices.

### Segment pair SoA/AoSoA

Evaluate both:

#### Query-major batches

```text
one left segment × N right segments
```

This extends the current noding broad phase and amortizes splatted left endpoint
data.

#### Pair-major batches

```text
N unrelated left/right segment pairs
```

This is more general after grid, monotone-chain, sweep, or component-local
candidate generation and may maintain higher occupancy.

A pair-major block conceptually needs:

```text
left_start_x[N]   left_start_y[N]
left_end_x[N]     left_end_y[N]
right_start_x[N]  right_start_y[N]
right_end_x[N]    right_end_y[N]
```

Prefer an array of small SoA blocks, or AoSoA, when it avoids constructing a
second full copy of the input. Measure both packing cost and cache behavior.

### Thread-local output

Each batch processor should emit into a reusable local structure:

```rust
struct CandidateBatchOutput {
	split_events: Vec<SplitEvent>,
	scalar_fallbacks: Vec<CandidatePairId>,
	work_stats: CandidateBatchStats,
}
```

After parallel processing:

1. concatenate thread-local results in deterministic partition order;
2. evaluate or merge scalar fallbacks deterministically;
3. sort events by the existing canonical event key;
4. deduplicate through the existing path;
5. run the selected validator.

## Experiment A — Batch instrumentation before new math

Add the counters needed to know whether wide work is plausible:

- candidate pairs produced;
- number of batches;
- lane width;
- full, partial, and empty batches;
- active-lane utilization;
- masked-lane rate;
- scalar-tail count;
- candidate-packing bytes and time;
- batch-processing time;
- event-emission time;
- deterministic merge/sort time;
- peak scratch capacity;
- candidate source: SIMD brute force, grid, sweep, monotone chain, or component
  partition.

Keep this instrumentation cheap or disabled by default. Full timing should use
the existing diagnostics/trace controls rather than unconditional clocks in hot
loops.

**Exit decision:** do not implement a wide exact predicate unless at least two
representative workloads produce sufficiently full batches to plausibly
amortize setup.

## Experiment B — Refactor the existing wide AABB path

Move the current one-query-versus-four-target implementation behind the new
candidate batch boundary without changing its math.

Goals:

- establish scalar and current-wide baselines through the same API;
- prove that batch abstraction overhead is negligible;
- preserve exact event ordering, work counters, provenance, and Z behavior;
- measure query-major packing versus current `SoALines` construction;
- test serial, Rayon, Wasm scalar, and Wasm SIMD builds.

**Promotion threshold:** the refactor itself should be effectively neutral
end-to-end. A measurable regression means the abstraction needs revision before
additional kernels are added.

## Experiment C — Wide exact-intersection filter

The first exact-predicate prototype should be a conservative filter, not a
replacement for the robust scalar implementation.

### Suggested stages

1. Wide AABB rejection.
2. Wide orientation determinant evaluation for the four endpoint/segment
   combinations.
3. Conservative lane classification:
   - definitely disjoint;
   - clearly proper crossing;
   - ambiguous.
4. For clearly proper crossings, compute parametric `t`/`u` and provisional XY.
5. Send every ambiguous lane to the existing scalar robust intersection path.
6. Run Z interpolation and split-event creation through shared code.
7. Independently validate the fully noded result.

### Ambiguous cases

At minimum, scalar fallback should handle:

- any determinant inside a documented floating-point error bound;
- collinear or nearly collinear segments;
- endpoint-on-interior and endpoint-touching cases;
- non-finite intermediate values;
- subnormal or extreme coordinate ranges where classification is not proven;
- fixed-grid boundary or hot-pixel cases;
- any lane for which `t` or `u` cannot be proven inside the required interval.

Do not use an arbitrary global epsilon. If safe-lane classification uses an error
bound, document its derivation and test it against the robust scalar predicate.

### Deliberately deferred work

Keep these scalar/shared initially:

- collinear-overlap endpoint extraction;
- overlap dissolve;
- split sorting and deduplication;
- provenance merging;
- hot-pixel construction;
- full-noding validation.

These can be revisited only if profiles show they dominate after the proper
intersection filter succeeds.

## Experiment D — Point-wide repeated containment

The existing locator processes one point across several ring edges. Add an
internal experiment that groups multiple probe points by prepared shell and
processes several points per edge.

Potential API:

```rust
impl SimdRing {
	fn contains_many(
		&self,
		points: &[Coord<f64>],
		results: &mut [bool],
		scratch: &mut ContainsManyScratch,
	);
}
```

Evaluate:

- point-wide batching against one ring;
- current edge-wide `contains`;
- scalar ring traversal;
- the adaptive interval-tree locator;
- shell-grouped and ungrouped query ordering;
- one-shot, 16, 64, 1,024, and production-observed queries per shell.

Preserve the current boundary semantics. A fast locator that disagrees on
boundary points is a different algorithm, not an optimization.

Likely dispatch inputs:

- ring edge count;
- query count for the prepared shell;
- envelope candidate count;
- architecture;
- observed or predicted active-lane density.

## Experiment E — Safe fixed-grid bulk operations

Some fixed-grid operations may be data-parallel without changing topology:

- floating-to-grid coordinate scaling after range validation;
- round-to-grid conversion;
- endpoint AABB generation;
- integer-key construction after checked conversion;
- bulk comparison or hashing preparation.

Do not vectorize checked integer conversion until scalar preflight proves that
every lane is in range. SIMD must not hide overflow, saturation, or precision
loss.

Certified hot-pixel semantics and the independent validator remain authoritative.

## Dispatch policy

Do not dispatch on input segment count alone.

Candidate descriptors should include:

- total candidates;
- average and maximum batch fill;
- active-lane density after AABB filtering;
- candidate source;
- split density;
- collinear/ambiguous incidence;
- component size distribution;
- line-string chain length;
- ring edge count and queries per shell;
- target architecture and available instruction set.

Prefer a small deterministic decision tree generated from benchmark evidence.

Example shape—not a prescribed implementation:

```text
few candidates or poor batch fill
    → scalar

long sparse chains
    → monotone-chain or sweep candidate generation
    → scalar or pair-major predicate batches

dense uniform candidates with high active-lane density
    → grid candidates
    → wide exact filter with scalar robust fallback

many probes against one prepared shell
    → point-wide contains_many

small or irregular containment workload
    → current scalar/edge-wide/indexed locator crossover
```

No online learning, randomized autotuning, or machine-specific persistent state
belongs in the topology pipeline.

## Determinism and correctness requirements

For every prototype, assert:

- identical candidate pair set after canonical sorting;
- identical split-event set;
- identical noded and dissolved segments;
- identical canonical topology fingerprint;
- identical complete source provenance;
- identical Z outputs and conflict diagnostics;
- identical normalized errors and failure witnesses;
- identical serial/parallel results;
- no unexpected full-noding validation failure;
- no workspace poisoning after cancellation or error.

Testing only polygon count or union area is insufficient.

Add differential tests that compare the experimental path against the existing
scalar path before comparing against GEOS/JTS.

## Benchmark matrix

### Kernel benches

Measure separately:

- candidate packing;
- AABB batch tests;
- orientation classification;
- safe proper-intersection calculation;
- scalar fallback;
- event emission;
- deterministic merge/sort;
- `contains_many`;
- workspace preparation and reuse.

### End-to-end workloads

At minimum:

- 2–64 segments, where SIMD setup should usually lose;
- existing grid and bowtie sizes;
- random sparse lines;
- long sparse polylines;
- dense crossing-heavy cells;
- high collinear-overlap incidence;
- duplicate boundaries with large source sets;
- near-degenerate and expected-divergence compatibility fixtures;
- multi-component workloads;
- repeated hole assignment against long rings;
- real CAD/CFB and public-corpus clips.

### Targets

Decision-quality runs should cover:

- Linux x86-64;
- Linux AArch64;
- Wasm scalar;
- Wasm SIMD;
- serial and parallel native builds.

Add other targets only when they are supported and reproducible.

### Required reporting

Record:

- p50, p95, throughput, and sample count;
- input, candidate, split, and output sizes;
- candidate packing and compaction;
- active-lane utilization;
- robust-fallback rate;
- scalar-tail fraction;
- exact predicate and event-emission time;
- merge/sort time;
- allocations and peak RSS;
- instruction counts and cache data where stable;
- architecture, compiler, features, dependency versions, and commit SHA.

## Promotion gate

A new wide kernel or dispatch rule remains internal unless all of the following
are true:

1. Zero unexpected validator failures in golden, compatibility, real-world, and
   scheduled fuzz corpora.
2. Exact canonical equivalence for geometry, result families, provenance, Z,
   diagnostics, and normalized errors.
3. A predeclared end-to-end improvement, including packing and merge costs, on
   more than one representative workload.
4. A repeatable benefit on more than one supported target, unless the kernel is
   explicitly target-specific and isolated.
5. No material regression on small, sparse, degenerate, or fallback-heavy
   workloads.
6. Bounded scratch memory and no unacceptable compile-time or binary-size cost.
7. Deterministic and inspectable dispatch inputs.
8. A decision record containing raw artifacts, rejected alternatives, and the
   crossover range.

A microbenchmark-only win is not sufficient.

## Agent execution sequence

Implement as a stack of small PRs. Do not combine all stages into one change.

### PR 1 — Counters and fixture matrix

- add batch-utilization and fallback counters;
- add benchmark cases without changing dispatch;
- record baseline artifacts;
- state promotion thresholds in the PR body.

### PR 2 — Internal candidate batch contract

- introduce stable candidate IDs and width-independent batch types;
- add scalar adapter;
- prove exact candidate/event equivalence;
- no new SIMD math.

### PR 3 — Current broad phase through the batch API

- move existing AABB-wide behavior behind the boundary;
- reuse workspace buffers;
- measure abstraction and packing cost;
- reject or revise if the refactor regresses.

### PR 4 — Query-major and pair-major layout comparison

- implement both layouts behind private feature/test switches;
- measure occupancy, packing, cache behavior, and memory;
- keep the better layout, or keep both only with clear workload separation.

### PR 5 — Conservative wide proper-intersection filter

- implement safe-lane classification;
- scalar-fallback every ambiguous lane;
- share Z/provenance/event code;
- run validators and metamorphic ordering tests.

### PR 6 — Thread-local parallel batches

- compose the selected batch path with Rayon;
- avoid shared hot-loop mutation;
- prove serial/parallel canonical equivalence;
- measure merge overhead and scaling.

### PR 7 — Point-wide containment

- add `contains_many` internally;
- group containment queries by prepared shell;
- compare against current scalar, edge-wide, and interval-tree paths;
- preserve boundary semantics.

### PR 8 — Dispatch experiment

- derive a small deterministic rule from collected descriptors;
- keep forced strategies benchmark/test-only;
- evaluate all targets and real workloads;
- do not expose a public enum until the roadmap promotion gate passes.

### PR 9 — Decision and cleanup

- promote, retain as research, or delete each prototype;
- remove losing layouts and unused counters;
- document measured crossovers;
- update `ROADMAP.md` and benchmark decision records.

Each PR body should include:

```md
## Hypothesis

## Target workloads and non-targets

## Semantic invariants

## Benchmark plan

## Predeclared promotion threshold

## Results

## Correctness evidence

## Decision
```

## Review checklist

Reviewers and agents should reject a SIMD change when any answer is unclear:

- What independent work unit occupies one lane?
- What algorithm produced the candidates?
- What is the full cost of packing and unpacking?
- How are inactive lanes represented?
- Which lanes fall back to robust scalar code?
- How are pair IDs and event order made deterministic?
- Where are provenance and Z handled?
- Which validator checks the output?
- Which small/sparse workloads regress?
- What is the architecture-enabled scalar baseline?
- What evidence justifies the dispatch threshold?
- Can the prototype be removed cleanly if it loses?

## Anti-goals

This plan does not authorize:

- a public SIMD backend or lane-width option;
- replacing certified hot-pixel noding;
- replacing robust scalar predicates with unproven approximate math;
- AVX-512-first development;
- GPU compute before a CPU candidate boundary and workload corpus exist;
- graph coloring for immutable candidate predicates;
- vectorizing an all-pairs algorithm when a better index removes the pairs;
- keeping multiple kernels merely because they are technically interesting.

Graph coloring may matter for a future mutable/incremental arrangement where
parallel operations write shared topology. It is not required for independent
candidate tests and should not be introduced here.

## Definition of done

This optimization program is complete when:

- the candidate and containment batch boundaries are stable internally;
- all retained wide kernels pass the independent correctness and conformance
  gates;
- dispatch uses measured workload descriptors rather than folklore;
- setup, fallback, merge, memory, and binary costs are included in reports;
- losing experiments are removed or clearly archived;
- the roadmap records the final decision and supported crossover ranges;
- no public topology option was added solely to expose an implementation detail.
