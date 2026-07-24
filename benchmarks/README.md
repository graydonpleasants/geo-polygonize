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
any timed sample whose fingerprint changes:

```bash
cargo run -p geo-polygonize-core --release --example benchmark_record -- \
  --lane already-noded \
  --workload already-noded-coverage-v1 \
  --samples 30 \
  --peak-rss-bytes <externally-measured-peak> \
  --reference-result target/reference-result.json \
  --output target/benchmark-record.json
```

Peak RSS remains an explicit harness input because the Rust standard library
does not expose a portable process peak. The runner measures allocations with
the repository's existing `dhat` allocator.

Use `--lane floating` with a parity-class workload that permits the floating
profile to measure floating noding plus polygonization. The runner performs an
untimed full-noding validation of that pipeline before collecting samples.
Generate its reference with `reference_geos.py --lane floating`.

Use `--lane certified-fixed` only with a parity-class `certified-fixed`
workload. Its untimed gate and timed samples both retain
`CertifiedFixedPrecision`, so the lane measures hot-pixel snap rounding and
never substitutes the floating or iterative snap backend. GEOS/Shapely does not
provide the equivalent certified JTS pipeline, so the GEOS reference generator
deliberately rejects that lane.
