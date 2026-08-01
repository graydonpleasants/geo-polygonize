# WASM Integration

The `geo-polygonize` WebAssembly package can be used in the browser or in Node.js.

## Installation

```bash
npm install geo-polygonize
```

## Browser Apps

Use the slim entry point in Vite or similar app builds so the Wasm binaries stay
as assets instead of being inlined into the JavaScript chunk.

```ts
import { cfbRobustOptions, initBest } from "geo-polygonize/slim";
import scalarUrl from "geo-polygonize/geo_polygonize.wasm?url";
import simdUrl from "geo-polygonize/geo_polygonize_simd.wasm?url";

const wasm = await initBest(
  { module_or_path: scalarUrl },
  { module_or_path: simdUrl },
);

const result = wasm.polygonizeWithOptions(
  JSON.stringify(geojson),
  cfbRobustOptions,
);
```

The default `geo-polygonize` import is convenient for quick demos, but it
inlines the Wasm builds into the importing chunk.

## Scalar and SIMD selection

The default and slim entry points use the same feature test and prefer the SIMD
binary when `WebAssembly.validate` accepts a small `simd128` module. Otherwise
they load the scalar binary. Worker-backed calls run the same selection inside
the worker, so direct and cancellable calls do not choose different runtimes.

`initBest` accepts explicit scalar and SIMD modules or URLs. If the SIMD input
is omitted, both outcomes load the scalar input. Initialization is cached by the
default wrapper; do not initialize one wrapper with different binaries over its
lifetime.

## Buffer ownership and memory lifetime

The typed-buffer entry points borrow their `Float64Array` and `Uint32Array`
arguments only for the synchronous call. Rust parses them into owned linework
before polygonization; callers may reuse the JavaScript input arrays after the
call returns.

`WasmPolygonResult` owns its flattened output vectors. Its pointer methods expose
views into Wasm linear memory, not JavaScript-owned copies. Keep the result alive
while reading those views, and recreate every view after any call that may grow
Wasm memory because a growth can detach the previous buffer. Copy the arrays when
they must outlive the result or cross an async boundary. Ring and polygon offsets
are checked against the `u32` range before the result is returned.

The GeoArrow entry points borrow the incoming IPC bytes only for the synchronous
call and return a JavaScript-owned `Uint8Array`; they do not expose borrowed
Arrow C Data Interface pointers.

## Cancellation and workers

Direct Wasm exports are synchronous and cannot consume an `AbortSignal`. The
`polygonizeWithOptionsAsync`, `polygonizeReportWithOptionsAsync`, and
`polygonizeTraceWithOptionsAsync` wrappers instead create one disposable browser
worker per call. Aborting terminates that worker and rejects with `AbortError`;
completion and failure also terminate it. This bounds caller-visible latency but
does not resume or reuse partially computed state.

A pre-aborted signal rejects before a worker is created. Worker APIs require a
browser `Worker`; they reject in runtimes without one. Cold worker startup,
module compilation, and initialization are part of each call, and the package
does not currently provide a worker pool.

## Threads

The experimental `geo-polygonize/threads` entry point is a separate build. Call
its default initializer, then `initThreadPool(count)`, before polygonization. It
requires `SharedArrayBuffer`, atomics, a secure context, and cross-origin
isolation headers:

```text
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
```

The threads package does not add `AbortSignal` support. The disposable-worker
async helpers belong to the standard entry point, while the threads entry point
owns a Rayon pool for its lifetime. Use the scalar fallback when SIMD is not
available; use the standard package when shared memory or isolation is not
available.
