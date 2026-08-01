# Python memory and GIL behavior

The `geo-polygonize-py` package accepts coordinate iterables or contiguous
NumPy buffers and returns one of three output projections. The default remains
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

| Mode | Flat NumPy buffers | `SimplePolygon` objects | Fingerprint and timings |
| --- | --- | --- | --- |
| `buffers` | yes | no | no |
| `objects` | no | yes | no |
| `report` | yes | yes | yes |

Buffer flattening and fingerprint generation run on the Rust worker without the
GIL. Python object construction necessarily holds the GIL. The native binding
checks Python signals every 256 converted items while constructing polygon,
dangle, cut-edge, and invalid-ring objects.

`return_polygons=True` is a wrapper projection over `objects` or `report`: it
imports Shapely and converts each `SimplePolygon` after the native call. It is
incompatible with `buffers`, does not preserve the binding's provenance object
on Shapely geometries, and adds Python/GIL-bound conversion cost. Prefer
`buffers` for bulk numeric consumers and `report` only when complete conformance
evidence is required.

Report mode exposes `thread_spawn_ms`, `buffer_conversion_ms`,
`fingerprint_ms`, and `python_objects_ms`. These are phase observations, not a
stable performance promise.

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
