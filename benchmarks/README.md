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

The already-noded runner requires an externally established fingerprint and
reference dependency version, validates that the input is fully noded before
timing, and rejects any timed sample whose fingerprint changes:

```bash
cargo run -p geo-polygonize-core --release --example benchmark_record -- \
  --lane already-noded \
  --workload already-noded-coverage-v1 \
  --samples 30 \
  --expected-fingerprint-sha256 <64-lowercase-hex-digits> \
  --peak-rss-bytes <externally-measured-peak> \
  --reference-dependency geos=3.13.1 \
  --output target/benchmark-record.json
```

Peak RSS remains an explicit harness input because the Rust standard library
does not expose a portable process peak. The runner measures allocations with
the repository's existing `dhat` allocator.

Use `--lane floating` with a parity-class workload that permits the floating
profile to measure floating noding plus polygonization. The runner performs an
untimed full-noding validation of that pipeline before collecting samples.

Use `--lane certified-fixed` only with a parity-class `certified-fixed`
workload. Its untimed gate and timed samples both retain
`CertifiedFixedPrecision`, so the lane measures hot-pixel snap rounding and
never substitutes the floating or iterative snap backend.
