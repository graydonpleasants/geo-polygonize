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

### Code-generation audit (September 2026)

Dispatch labels are not evidence of the executed vector width. `wide` 0.7
selects the `f64x4` representation with compile-time
`cfg(target_feature = "avx")`: one `__m256d` when the whole crate is built with
AVX, otherwise two `f64x2` halves. A `multiversion` clone enables AVX2 for the
clone's body but cannot re-run `wide`'s conditional compilation, so the clone
still operates on the two-half representation.

`scripts/simd_codegen_audit.sh` emits release assembly (`codegen-units = 1`,
no default features) and summarizes each function that holds the
`contains_simd` loop. Results at `727ee5c`, Rust 1.96.1:

| Build | Function holding the loop | 256-bit ops | Vector ops per 4 edges |
|---|---|---:|---|
| x86-64 generic | `contains_simd` SSE2 clone | 0 | 2 × 128-bit, legacy SSE encoding |
| x86-64 generic | `contains_simd` AVX clone | 0 | 2 × 128-bit, VEX encoding |
| x86-64 generic | `contains_simd` AVX2 clone | 0 | identical to the AVX clone |
| `target-cpu=x86-64-v3` | inlined into `SimdRing::contains` | 18 | 1 × 256-bit, `vdivpd ymm`, `popcnt` |
| AArch64 (Apple host) | inlined into `SimdRing::contains` | n/a | 2 × 128-bit NEON `.2d` |

Observations:

- Portable x86-64 artifacts (wheels, crates.io consumers without target flags)
  never execute 256-bit arithmetic in this kernel. The AVX2 clone gains only
  three-operand VEX encoding over SSE2; its instruction stream matches the AVX
  clone. The AVX2 target entry is therefore redundant with AVX for this kernel.
- Only an AVX-enabled whole-crate build produces a 256-bit loop. That build
  changes the hardware floor and must not be used for portable artifacts.
- In the two-half builds the overlapping `x[i + 1..i + 5]` load is assembled
  lane by lane (`vmovsd`/`vunpcklpd`/`vmovhpd` on x86, `mov.d`/`ld1.d` on NEON)
  instead of one unaligned load; the 256-bit build uses `vmovupd` plus one
  `vinsertf128`.
- `move_mask` on NEON is emulated with about ten scalar lane extractions per
  batch before `cnt`/`addv`.
- No hot-loop helper calls or stack spills appear in any build; the only calls
  are cold bounds-check panics. Runtime dispatch in the generic build is one
  indirect jump per ring traversal, not per batch.

Same-host timings on Apple M-series (`point_in_ring_crossover/repeated`,
1,024 queries) show generic and `target-cpu=native` within noise:

| Edges | scalar generic | wide generic | scalar native | wide native |
|---:|---:|---:|---:|---:|
| 32 | 28.5 µs | 27.1 µs | 28.7 µs | 27.3 µs |
| 128 | 117.6 µs | 107.7 µs | 120.9 µs | 106.4 µs |
| 256 | 220.7 µs | 216.4 µs | 213.2 µs | 216.0 µs |
| 1,024 | 787.1 µs | 858.1 µs | 792.4 µs | 901.7 µs |

This is consistent with the existing 257-coordinate scalar crossover on
non-Linux-AArch64 hosts.

The cross-architecture workflow runs the same group on a GitHub-hosted Linux
x86-64 runner, once with the generic target and once with
`-C target-cpu=x86-64-v3` in the same job (run 36373484118):

| Edges | scalar generic | wide generic | scalar v3 | wide v3 |
|---:|---:|---:|---:|---:|
| 32 | 40.8 µs | 37.7 µs | 30.3 µs | 24.3 µs |
| 128 | 160.1 µs | 143.8 µs | 154.9 µs | 93.7 µs |
| 256 | 322.8 µs | 291.4 µs | 266.1 µs | 189.0 µs |
| 1,024 | 1.23 ms | 1.14 ms | 936 µs | 773 µs |

The 256-bit loop makes `wide` about 1.5× faster than the two-half loop that
portable x86-64 artifacts execute. Part of the gap is compiler-only: scalar
also improves by about 24% at 1,024 edges under v3. Shared runners vary by
about 10% between groups, so compare only within one job.

The benchmark copy of the kernel in `hole_sort_bench.rs` places the
`multiversion` boundary around the whole ring traversal, and the
`.filter().count()` iterator sits outside it. The iterator-closure
target-feature propagation issue reported upstream (linebender/fearless_simd
#380) therefore does not apply to the current benchmark. It may still apply to
the archived Fearless SIMD comparison from PR #795, where `dispatch!` wrapped a
`.filter().count()` chain around the kernel. Its published numbers were
measured on an M1 Max, where NEON is part of the AArch64 baseline, so a
lost target-feature context would not change the executed instructions there.
The confound can only matter for x86 runs of that harness. Those runs need an
explicit-loop comparison before their numbers are reused.

### Empty-batch skip experiment (September 2026)

`wide_contains` computes the intersection x-coordinate, including a division,
for every batch even when no lane straddles the probe's y. The scalar kernel
short-circuits before that arithmetic. `hole_sort_bench.rs` now carries a
benchmark-only `wide_contains_skip_empty` that tests `in_range.move_mask() == 0`
first and otherwise evaluates the unchanged expression. The
`point_in_ring_empty_batch_skip` group compares it with scalar, `wide`, and the
production adaptive `SimdRing::contains` on two fixtures:

- `circle`: about two straddling edges per probe, so nearly every batch is
  empty;
- `sawtooth`: a zigzag boundary where nearly every edge straddles every probe.

Apple M-series, generic build, 1,024 probes, Criterion mean:

| Fixture | Edges | scalar | wide | wide_skip_empty | adaptive |
|---|---:|---:|---:|---:|---:|
| circle | 32 | 31.8 µs | 29.0 µs | 20.7 µs | 35.5 µs |
| circle | 128 | 130.1 µs | 118.5 µs | 58.5 µs | 125.2 µs |
| circle | 256 | 237.2 µs | 231.4 µs | 116.7 µs | 264.6 µs |
| circle | 1,024 | 870.6 µs | 901.5 µs | 446.3 µs | 927.1 µs |
| sawtooth | 32 | 53.2 µs | 28.9 µs | 48.2 µs | 32.3 µs |
| sawtooth | 128 | 216.2 µs | 112.4 µs | 170.0 µs | 131.3 µs |
| sawtooth | 256 | 440.3 µs | 236.3 µs | 314.9 µs | 454.4 µs |
| sawtooth | 1,024 | 1.63 ms | 1.03 ms | 1.49 ms | 1.73 ms |

Reading:

- On sparse-straddle rings the skip halves kernel time at every size and beats
  scalar at 1,024 edges, where production currently selects scalar. If
  promoted, the 257-coordinate crossover would need to be re-derived.
- On dense-straddle rings the extra mask extraction and branch cost 30–45%
  against plain `wide`, but the skip still beats scalar at every size.
- Crossing counts are asserted equal to scalar for every kernel and fixture.
  Active batches evaluate the same floating-point expression, so crossing
  decisions cannot change.

GitHub-hosted Linux runners (cross-architecture workflow, run 36373484118),
1,024 probes, kernel time relative to plain `wide` in the same job:

| Runner | circle, skip vs `wide` | sawtooth, skip vs `wide` | circle 1,024, skip vs scalar |
|---|---|---|---|
| x86-64 generic | 0.43–0.57× | 1.08–1.12× | 0.51× |
| x86-64 v3 | 0.46–0.57× | 1.01–1.06× | 0.34× |
| AArch64 | 0.36–0.61× | 1.42–1.46× | 0.67× |

- On x86-64 the downside on dense rings is small (at most 12% generic, 6% v3),
  while sparse rings run about twice as fast as either `wide` or scalar.
- On Linux AArch64, production always selects scalar. Plain `wide` is about
  1.8× slower than scalar there, which confirms that choice, but the skip beats
  scalar by 33% on circles and loses to it by about 14% on sawtooth rings.

Decision: keep as benchmark-only evidence until straddle density is measured on
real hole-assignment workloads. If real rings resemble the circle fixture,
promote the skip into `contains_simd`, re-derive the scalar crossover per
target, and confirm with the end-to-end containment benchmarks under the
promotion gate.

### Straddle density on real rings (September 2026)

`crates/geo-polygonize-core/examples/ring_straddle_density.rs` polygonizes
inputs and replays approximations of the two production point-in-ring query
families:

- hole-assignment probes near each hole vertex, tested against every output
  shell whose envelope contains the probe;
- interior-probe candidates near each ring's own vertices, tested against that
  ring.

For each query it counts the full four-edge batches that contain at least one
straddling edge. The skip variant can bypass only the batches without one.

```bash
cargo run --release -p geo-polygonize-core --example ring_straddle_density -- \
  fixtures/cfb/cases crates/geo-polygonize-core/tests/workloads/clips examples/data
```

Results for every local input (35 files, 14 with output rings). The CFB
`large_lot_block_anon_curb_snap_gap` fixture supplies about 93% of the queries
(1,253 shells, 10 holes, rings up to 820 coordinates):

| Family / ring coordinates | Queries | Active batches | Straddling edges per query |
|---|---:|---:|---:|
| hole assignment / 5–32 | 1,775 | 31.6% | 1.67 |
| hole assignment / 33–128 | 2,788 | 12.0% | 1.94 |
| hole assignment / 129–256 | 3,128 | 4.8% | 2.17 |
| hole assignment / 257+ | 5,356 | 2.4% | 3.20 |
| interior probe / 5–32 | 23,930 | 32.9% | 1.56 |
| interior probe / 33–128 | 23,440 | 12.2% | 1.96 |
| interior probe / 129–256 | 11,070 | 4.9% | 2.31 |
| interior probe / 257+ | 17,056 | 2.6% | 3.14 |
| calibration circle / 32 edges | 1,024 | 18.3% | 1.46 |
| calibration circle / 256 edges | 1,024 | 2.3% | 1.46 |
| calibration circle / 1,024 edges | 1,024 | 0.6% | 1.46 |
| calibration sawtooth / any size | 1,024 | 100% | edges − 2 |

Reading:

- Real rings cross a probe's horizontal line about two or three times,
  whatever their size. That matches the circle fixture and is far from the
  sawtooth fixture, where every batch is active.
- Small rings have a higher active fraction than the 32-edge circle, because
  there are few batches and any straddle activates one. Even at 5–32
  coordinates, two thirds of full batches are empty.
- The evidence comes mostly from one anonymized CFB block. The OSM production
  corpus described in `docs/guide/production-corpus.md` has not been
  materialized for this measurement, and the synthetic clips produce few or no
  rings under default options.

Decision: the straddle-density precondition for promoting the empty-batch skip
is met on the available real input. The next step is a production change to
`contains_simd`, gated on the end-to-end containment benchmarks and a
re-derived scalar crossover per target. Confirm the density figures on the
production corpus when it is materialized.

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
- Does the runtime-dispatched context contain the entire hot loop and its
  helpers, with no iterator closure or non-inlined call between the dispatch
  boundary and the vector operations?
- Does the emitted assembly for the portable build show the claimed vector
  width? Rerun `scripts/simd_codegen_audit.sh` for kernels built on
  compile-time-selected SIMD types such as `wide`.
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
