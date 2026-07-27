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
features, and dependency versions. Phase names and throughput units are explicit
strings so different runners can report their native phases without pretending
unlike pipelines are equivalent.

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
the repository's existing `dhat` allocator.

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

`benchmark-decision-v1.schema.json` defines the durable decision record for an
experiment. Store records under `benchmarks/decisions/` when real evidence
exists. Each record preserves the predeclared policy thresholds and links
checksum-pinned baseline and candidate publications. Rejected experiments
require their own publication links; measured crossovers require both linked
publications and an explicit descriptor, range, and unit.

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
