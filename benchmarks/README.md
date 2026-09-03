# Correctness-gated benchmark records

`benchmark-record-v1.schema.json` defines the only publishable timing record.
It keeps three lanes separate:

1. already-noded polygonization;
2. floating noding plus polygonization;
3. certified fixed-precision noding plus polygonization.

A runner must validate its promised noding postcondition and compare the
topology fingerprint before serializing a record. Expected divergence requires
both the manifest classification and a reason. Failed or unexplained runs are
test artifacts, not timing records.

Records pin the commit, compiler, operating system, architecture, implementation,
features, dependency versions, and the selected workload clip's manifest
SHA-256. Phase names and throughput units are explicit strings so different
runners can report their native phases without pretending unlike pipelines are
equivalent.

`production-corpus-v1.json` separately pins redistributable production-scale
source metadata. It contains no geometry. Use
`scripts/materialize_production_workloads.py` to create an out-of-tree runner
manifest and derived clips, then pass that manifest with `--manifest` to the
reference and Rust runners. See the [production corpus guide](../docs/guide/production-corpus.md)
for acquisition, attribution, and validation requirements.

The dedicated publication workflow accepts the same operator-staged manifest
without copying the source or derived clips into Git. Dispatch it with an
absolute manifest path that exists on the `benchmark-dedicated` runner:

```bash
gh workflow run benchmark-publication.yml --ref main \
  -f workload=osm-california-highways-10k-v1 \
  -f lane=floating \
  -f manifest_path=/mnt/geo-polygonize/runner-manifest-v1.json
```

Pass `stitched_tile_size` and `stitched_buffer` to add correctness-gated
stitched-output timing, allocation, and peak-RSS evidence to the record. The
stitched result must also match the same-options untiled result and the
external reference; it remains an additive research field and does not change
the published polygonization path.

The workflow defaults to the `parallel` core feature set. Pass
`-f feature_set=serial` for a matched no-default-features publication; the
record retains the selected feature set in `implementation.features`.

Its `noding_path` input defaults to `production`. Selecting
`mcindex-experiment` is limited to the floating lane and emits a separate,
correctness-gated record labeled `geo-polygonize-core-mcindex-experiment`; it
does not change production dispatch.

The selected clip path is resolved relative to that manifest and its declared
SHA-256 is verified by the GEOS reference, Rust benchmark runner, and JTS
reference before any correctness or timing work. Certified-fixed dispatches
mount the manifest parent into the JTS container; a manifest must explicitly
permit that lane. The workflow publishes only the resulting gated records and
does not download or retain the source material.

`benchmark-decision-policy-v1.json` separates diagnostic runs from evidence
that can support a performance decision. Shared or smoke-test measurements are
diagnostic and nonpublishable. Decision-quality measurements require a
dedicated runner, five independent processes with at least 30 samples each,
warmup, no more than 3% relative median absolute deviation, a passed correctness
gate, a pinned environment, and the same commit.

Before an experiment starts, its primary claim must meet the policy's 5%
minimum effect size while every secondary metric stays within the 2% regression
budget. The current hosted correctness jobs collect no timings and therefore
remain diagnostic.

`reference-result-v1.schema.json` defines the external correctness evidence.
Its topology fingerprint compares only fields both implementations can produce:
canonical XY polygon rings, dangles, cut edges, and invalid rings. Rust-only
edge IDs, provenance, options, and diagnostics remain in the benchmark record
but cannot be used as cross-implementation equality evidence.

Generate a GEOS/Shapely reference for the already-noded or floating lane:

```bash
python3 benchmarks/reference_geos.py \
  --lane already-noded \
  --workload already-noded-coverage-v1 \
  --output target/reference-result.json
```

The benchmark runner validates that the reference workload, lane, dependency
versions, payload hash, and topology all match before timing. It also rejects
any timed sample whose fingerprint changes and performs five untimed warmups by
default:

```bash
cargo run -p geo-polygonize-core --release --example benchmark_record -- \
  --lane already-noded \
  --workload already-noded-coverage-v1 \
  --samples 30 \
  --repetition 1 \
  --peak-rss-bytes <externally-measured-peak> \
  --reference-result target/reference-result.json \
  --output target/benchmark-record.json
```

Use `--check-only` to run the complete external correctness gate without
collecting timings or requiring a peak-RSS measurement. CI runs this mode for
every GEOS-comparable parity workload using `reference-requirements.txt`.

Peak RSS remains an explicit harness input because the Rust standard library
does not expose a portable process peak. The runner measures allocations with
the repository's existing `dhat` allocator. Each generated record also carries
`work.component_memory`, which reports deterministic component distribution,
global `Vec<Vec<DirEdgeId>>` adjacency capacity, reusable scratch high-water
capacities, scratch-state instances, configured execution workers, and merged
output buffering.
Those fields are element capacities rather than byte estimates; use them with
allocation and peak-RSS measurements when evaluating a layout change.
Decision-quality records also carry an optional `measurement.layout_candidate`
shadow result. It checks packed-CSR component and face-successor traversal
against the current adjacency lists and times those shared operations; it does
not change production dispatch or establish an end-to-end layout decision.

Run the benchmark in five separate processes with unique `--repetition` values,
then gate the records before publication:

```bash
python3 benchmarks/publish_benchmark.py \
  --runner-class dedicated \
  --warmup-iterations 5 \
  --record target/benchmark-record-1.json \
  --record target/benchmark-record-2.json \
  --record target/benchmark-record-3.json \
  --record target/benchmark-record-4.json \
  --record target/benchmark-record-5.json \
  --output target/benchmark-publication.json
```

The publisher rejects shared runners, too few warmups, process repetitions or
samples, duplicate record IDs, mixed commits/environments/workloads, schema
failures, and p50 relative median absolute deviation above 3%.

`production-baseline-suite-v1.json` is the fail-closed matrix for the next
roadmap decisions. It requires seven publications: already-noded coverage and
nested containment, floating and certified-fixed dense crossings, and floating
California highway tiers at approximately 1k, 10k, and 100k input segments.
Every entry must retain `work.component_memory`, and the suite must use one
implementation, architecture, OS, compiler, and commit. Aggregate downloaded
`publication.json` artifacts only after each entry has passed the individual
publication gate:

```bash
python3 benchmarks/validate_baseline_suite.py \
  --suite benchmarks/production-baseline-suite-v1.json \
  --publication artifacts/already-noded-coverage/publication.json \
  --publication artifacts/component-nested-containment/publication.json \
  --publication artifacts/floating-dense-crossings/publication.json \
  --publication artifacts/certified-fixed-dense-crossings/publication.json \
  --publication artifacts/production-network-1k/publication.json \
  --publication artifacts/production-network-10k/publication.json \
  --publication artifacts/production-network-100k/publication.json \
  --output artifacts/production-baseline-evidence-v1.json
```

The validator emits a checksum-linked evidence summary and rejects missing,
duplicate, mixed-environment, undersized, or incomplete publications. The
matrix is a publication contract, not evidence by itself; the P1.2 roadmap
gate remains open until these artifacts are produced on the dedicated runner.

`benchmark-decision-v1.schema.json` defines the durable decision record for an
experiment. Store records under `benchmarks/decisions/` when real evidence
exists. Each record preserves the predeclared policy thresholds and links
checksum-pinned baseline and candidate publications. Rejected experiments
require their own publication links; measured crossovers require both linked
publications and an explicit descriptor, range, and unit.

Render schema-valid publications and decisions as a deterministic Markdown
trend view:

```bash
python3 benchmarks/render_benchmark_trends.py \
  --publication artifacts/benchmark-publication.json \
  --decision benchmarks/decisions/candidate-layout-v1.json \
  --output artifacts/benchmark-trends.md
```

The renderer refuses invalid or diagnostic artifacts and emits no fabricated
rows when one evidence class is absent.

Create a deterministic component-memory report from one or more published
artifacts after the individual publication gates pass:

```bash
python3 benchmarks/analyze_component_memory.py \
  --publication artifacts/production-network-1k/publication.json \
  --publication artifacts/production-network-10k/publication.json \
  --publication artifacts/production-network-100k/publication.json \
  --output artifacts/component-memory-evidence-v1.json
```

The report keeps the raw component, partition-capacity, scratch, worker, peak
RSS, and allocation evidence and adds only deterministic ratios for component
balance, partition-capacity overhead, and scratch instances per worker. It
requires one implementation, environment, dedicated runner, and decision-
quality publication class. It is measurement evidence, not a layout or
execution-policy promotion decision.

Use `--lane floating` with a parity-class workload that permits the floating
profile to measure floating noding plus polygonization. The runner performs an
untimed full-noding validation of that pipeline before collecting samples.
Generate its reference with `reference_geos.py --lane floating`.

Use `--lane certified-fixed` only with a parity-class `certified-fixed`
workload. Its untimed gate and timed samples both retain
`CertifiedFixedPrecision`, so the lane measures hot-pixel snap rounding and
never substitutes the floating or iterative snap backend. GEOS/Shapely does not
provide the equivalent certified pipeline, so the GEOS reference generator
deliberately rejects that lane.

The certified reference uses pinned JTS `1.20.0`
`GeometryNoder` snap rounding with validation, deduplicates the resulting
segments, and feeds them to JTS `Polygonizer`:

```bash
mvn -q -f benchmarks/jts-reference/pom.xml package
java -jar benchmarks/jts-reference/target/geo-polygonize-jts-reference-1.0.0.jar \
  --root "$PWD" \
  --workload dense-crossings-v1 \
  --output target/jts-reference.json
```

Pass that result to `benchmark_record --lane certified-fixed
--reference-result target/jts-reference.json`. CI runs this correctness-only
gate for every certified-fixed parity workload before any timings are allowed.
