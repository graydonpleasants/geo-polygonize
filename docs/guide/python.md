# Python memory and GIL behavior

The `geo-polygonize-py` package accepts coordinate iterables or contiguous
NumPy buffers and returns one of four output projections. The default remains
`report` during the compatibility window.

## Input ownership

The Python wrapper converts `lines` into contiguous `float64` coordinates and
`uint32` offsets. Existing arrays are normalized with `numpy.ascontiguousarray`,
which reuses them when their dtype and layout already match and copies them
otherwise.

The native extension validates those borrowed NumPy slices and copies their
segments into owned Rust linework while holding the GIL. Polygonization then
runs on a dedicated Rust worker with the GIL released. Independent calls own
independent geometry and may execute concurrently; thread creation is currently
paid once per call.

## Output modes

Request only the representation the caller needs:

| Mode | Flat NumPy buffers | Geometry objects | Diagnostics¹ and binding timings | Fingerprint |
| --- | --- | --- | --- | --- |
| `buffers` | yes | no | no | no |
| `objects` | no | yes | no | no |
| `report` (default) | yes | yes | yes | yes |
| `lean_report` (opt-in) | yes | yes | yes | no |

¹ Diagnostics are included only when enabled by the caller’s core options.
Geometry objects include polygons, dangles, cut edges and invalid rings.

Buffer flattening and fingerprint generation run on the Rust worker without the
GIL. Python object construction necessarily holds the GIL. The native binding
checks Python signals every 256 converted items while constructing polygon,
dangle, cut-edge, and invalid-ring objects.

`return_polygons=True` is a wrapper projection over `objects`, `report`, or
`lean_report`: it
imports Shapely and converts each `SimplePolygon` after the native call. It is
incompatible with `buffers`, does not preserve the binding's provenance object
on Shapely geometries, and adds Python/GIL-bound conversion cost. Prefer
`buffers` for bulk numeric consumers and `report` only when complete conformance
evidence is required.

Report mode exposes `thread_spawn_ms`, `buffer_conversion_ms`,
`fingerprint_ms`, and `python_objects_ms`. These are phase observations, not a
stable performance promise. `fingerprint_ms` covers fingerprint construction,
not its serialization and JSON decoding into Python.

### Opt-in lean reporting

`output="lean_report"` has exactly the report result keys except
`topology_fingerprint`. It preserves packed arrays, `SimplePolygon` values,
representative ring IDs, complete boundary-source provenance, non-polygon
geometry, and enabled diagnostics. It does not change semantic core options,
precision, noding, output filtering, ordering, limits, or errors. Full `report`
remains the default and retains its existing shape.

This avoids both native fingerprint construction and its Rust-JSON-Python
materialization. It still constructs packed arrays and geometry objects, so it
is deliberately a conservative projection rather than a minimum-memory packed
API. Callers requiring the exact conformance fingerprint must request `report`.
There is no hidden lazy audit or automatic repeat of polygonization: lean
results are ordinary mutable dictionaries and do not retain the native result.
An application that requires fingerprints must explicitly approve any reporting
contract change before adopting lean mode.

NumPy output arrays adopt the flattened Rust vectors. This does not make the
whole path zero-copy: input segments are copied into owned Rust lines, output
flattening copies geometry into vectors, and geometry objects remain a separate
materialization.

### Detailed diagnostic profiling

`profile_binding=True` adds `binding_profile` for any output mode, without
changing the existing four-key `binding_timings` dictionary. It records:

- `input_conversion_ms`: NumPy validation and copy into owned native segments.
- `native_polygonize_ms`: the complete core call on the worker.
- `thread_spawn_ms`: thread-start overhead, which can overlap worker activity.
- `buffer_conversion_ms`: flattening into owned output vectors.
- `buffer_export_ms`: transferring vectors into NumPy arrays.
- `fingerprint_build_ms`: exact fingerprint construction; zero for lean.
- `fingerprint_json_ms`: fingerprint serialization and Python `json.loads`;
  zero for lean.
- `python_objects_ms`: polygon and non-polygon object construction.
- `diagnostics_json_ms`: enabled diagnostic serialization and `json.loads`.
- `binding_body_ms`: internal binding-body elapsed time, after native argument
  and options parsing and before profile construction/final result destruction.

These observations are not an additive accounting model. The binding-body
subtotal excludes Python wrapper normalization, options serialization and Shapely
construction. Measure wall time around the desired whole pipeline separately.
To include core subphase observations, explicitly enable `diagnostics.timings`
in the semantic options; profiling itself does not enable diagnostics.

`benchmarks/python_lean_report.py` is a reproducible local diagnostic experiment.
It alternates full/lean sample order and checks exact buffers, source IDs,
provenance, non-polygon geometry and diagnostics after every sample. Independent
analytical and GEOS geometry checks precede timing; gates remain active under
`python -O`. Its GEOS timings apply only to the labeled noding-plus-polygonize
grid lane, and GEOS does not provide native provenance or all diagnostic
metadata. This is neither a production grading benchmark nor the dedicated-runner
publication protocol in the benchmark methodology.

`benchmarks/python_lean_prepared.py` applies the same exact gates to external
checksum-pinned prepared XY LineStrings. Its GEOS lane is extraction-only:
`polygonize_full` receives the same already-noded segments. It explicitly
excludes the original snapping/noding/preparation work and treats supplied IDs
as prepared-input IDs, not as verified original drawing-entity attribution.
Private inputs must remain outside the source repository.

### Correctness-baseline qualification

The lean projection is independent of core correctness PRs [#1561](https://github.com/graydonpleasants/geo-polygonize/pull/1561)
and [#1562](https://github.com/graydonpleasants/geo-polygonize/pull/1562). This change
does not copy those fixes into its diff. Normal CI always compares full and lean
outputs for the endpoint-region and point-touch inputs, including source IDs.
Three expected-correctness assertions (two coordinate strides for the owned
point-touch hole, and one endpoint closure) additionally require both fixes.
They are skipped on the standalone main-based projection branch; after applying
both correctness PRs, enable the combined adoption gate explicitly:

```sh
GEO_POLYGONIZE_COMBINED_CORRECTNESS=1 python -m pytest -q python/test_lean_report.py
```

This switch does not detect or silently skip missing prerequisites. With it
enabled, missing endpoint/ring behavior fails the expected geometry, hole, and
source assertions. A passing standalone projection suite is not evidence that
the combined correctness baseline or downstream grading has been qualified.

## Signals, cancellation, and limits

Before starting work, the binding checks pending Python signals. While the GIL
is released it wakes every 10 ms to check again. A signal cancels the core's
cooperative `CancellationToken`, waits for the Rust worker to stop, and then
raises the Python signal exception. The 10 ms wake interval is not a hard
cancellation-latency bound: the worker stops only at core polling points, and an
individual standard-library sort cannot be interrupted.

The optional `execution_limits` dictionary maps to `ExecutionPolicy` and is
kept separate from semantic polygonizer options. Limits stop work with a
normalized `ResourceLimitExceeded` error; they are logical work and output
guards, not a process-memory sandbox. Unknown limit names are rejected.

Flat ring and polygon offsets are checked before conversion to `uint32`. Python
exceptions retain the normalized structural error JSON on their `normalized`
attribute; callers should inspect that contract instead of matching message
text.
