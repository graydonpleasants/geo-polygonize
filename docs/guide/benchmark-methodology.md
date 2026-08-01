# Benchmark methodology

Published performance claims use the versioned schemas and decision policy in
`benchmarks/`. Criterion output, a single local timing, and hosted-runner smoke
tests are diagnostic only.

## Correctness before timing

Choose one lane declared by the workload manifest:

1. `already-noded` measures polygonization without noding;
2. `floating` measures floating noding plus polygonization;
3. `certified-fixed` measures certified fixed-precision noding plus
   polygonization.

The benchmark runner first validates the promised noding postcondition and
compares canonical XY topology with a pinned external reference. GEOS/Shapely
supplies the already-noded and floating references. JTS `GeometryNoder` and
`Polygonizer` supply the certified-fixed reference. Expected divergence is
publishable only when the workload manifest declares it and records a reason.

No timing record is emitted when the gate fails. Every timed sample is checked
against the accepted fingerprint again so a post-gate topology change also
stops publication.

## Reproduce one record

Install the pinned reference dependencies and build the runner:

```bash
python3 -m pip install -r benchmarks/reference-requirements.txt
cargo build --locked --release -p geo-polygonize-core --example benchmark_record
```

Generate a reference and run the correctness gate without timing:

```bash
python3 benchmarks/reference_geos.py \
  --lane already-noded \
  --workload already-noded-coverage-v1 \
  --output target/reference.json

target/release/examples/benchmark_record \
  --lane already-noded \
  --workload already-noded-coverage-v1 \
  --reference-result target/reference.json \
  --check-only
```

For a timing record, measure peak RSS outside the process and pass the byte
count explicitly. The runner measures allocations, performs five warmups by
default, and requires at least 30 samples for decision-quality publication:

```bash
target/release/examples/benchmark_record \
  --lane already-noded \
  --workload already-noded-coverage-v1 \
  --reference-result target/reference.json \
  --peak-rss-bytes 12345678 \
  --samples 30 \
  --warmup-iterations 5 \
  --repetition 1 \
  --output target/record-1.json
```

The certified lane uses the pinned Maven image and JTS command documented in
`benchmarks/README.md`; it must not substitute a GEOS reference or an unchecked
fixed-grid backend.

## Decision-quality publication

Run five independent processes with repetition IDs 1 through 5 on a dedicated
runner. Then pass all five records to `benchmarks/publish_benchmark.py`. The
publisher rejects mixed commits, environments, workloads, lanes, or correctness
results; duplicate IDs; insufficient warmup or samples; shared runners; and p50
relative median absolute deviation above 3%.

An optimization experiment must be declared before measurement. Its primary
p50 claim must improve by at least 5%, while p95, allocated bytes, and peak RSS
must remain within the 2% regression budget. Rejections and measured crossovers
remain linked to their checksum-pinned publications rather than disappearing
from the evidence record.

The manual `Publish benchmark evidence` workflow performs this process on a
dedicated self-hosted runner and retains the reference, raw records,
publication, checksums, and rendered trend report for 90 days. The hosted
benchmark-evidence workflow checks correctness only and publishes no timings.

## Reading and comparing claims

`benchmark-record-v1.schema.json` records p50, p95, throughput, samples, phase
times, allocations, peak RSS, topology metrics, candidate pairs, exact
predicates, split events, segment expansion, architecture, compiler,
dependencies, and commit SHA. Compare only records with the same workload,
lane, retained semantics, and pinned environment.

Microbenchmarks explain a phase but cannot promote an optimization. Likewise,
the playground exercises the package-name exports used by published consumers;
it is a debugger and reproduction surface, not a decision-quality timing
runner.
