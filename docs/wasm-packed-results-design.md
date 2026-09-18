# Wasm packed result design

## Decision

The JavaScript package keeps the existing Rust-owned `WasmPolygonResult` as the
single backing representation and adds `PackedPolygonResult` as its lifetime
and memory-safe facade. This follows the useful part of the pmndrs/math bridge
design: one backing store, convenient access over it, and explicit allocation.
It does not add a renderer dependency or change the geometry engine.

`polygonizePackedWithOptions` retains the complete existing report.
`polygonizePackedGeometryWithOptions` is an explicitly lossy projection that
retains only coordinates, ring starts, polygon starts, representative line IDs,
stride, and non-conformance boundary metrics. Both call the same core
polygonizer and flattening implementation. The geometry projection skips
fingerprint construction and serialization plus dangle, cut-edge, invalid-ring,
provenance, and diagnostics conversion.

## Boundary copies and allocations

The input typed arrays are copied by generated `wasm-bindgen` glue into linear
memory. Rust then validates them and constructs owned `Line3D` values; typed
array input is therefore not zero-copy. `parse_buffer_lines` reserves its exact
segment capacity but no input arena or pointer ABI is introduced.

Core polygonization is unchanged. Flattening allocates four result vectors once
with precomputed capacities. `withBorrowed` creates four typed-array views over
those vectors and no per-vertex objects. `snapshot` copies those four arrays
into independent JavaScript `ArrayBuffer`s. `toGeoJSON` is the explicit
compatibility path and intentionally allocates nested arrays and objects.

The full projection still constructs `TopologyFingerprintV1`, serializes it in
Rust, and parses it into JavaScript before flattening. `boundaryMetrics`
separates `report_materialize_ms` from `output_flatten_ms`; these timings and
the packed byte count are excluded from conformance values.

## Lifetime

Each facade captures the exact `WebAssembly.Memory` returned by the initializer
that created it. Every scoped borrow reads the current `memory.buffer`, so it
reacquires after non-shared memory detachment and after shared memory growth.
The Rust owner remains live until deterministic `dispose`, `free`, or
`Symbol.dispose`; later polygonization calls cannot recycle its vectors.

Borrowed views are read-only by contract, not runtime-immutable. They are valid
only during the synchronous `withBorrowed` callback. The facade rejects async
callbacks, re-entry through the same handle, disposal during a borrow, and all
access after disposal. JavaScript cannot prevent a callback from retaining or
writing an escaped typed array, nor can it intercept unrelated direct Wasm
calls; consumers must not do either. A snapshot is the supported representation
across `await`, disposal, worker transfer, or mutation.

## Deferred work

Input arenas, raw-pointer input ABIs, reusable workspaces, shared-memory
protocols, persistent editing, and core storage changes require independent
evidence. None is needed for this result boundary.
