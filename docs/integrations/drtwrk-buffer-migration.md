# drtwrk packed-buffer migration

## Implementation and publication status

- Implementation commit: `c8f82831218685f78925fca74def506eac6a4a3b`
- Source baseline: `cf04ab525f4330f15dd541f41fe803c1b4d2e2b2`
- npm package checked on 2026-09-15: published `geo-polygonize@1.1.0`
- Publication status: **not published**. The source build also reports version
  `1.1.0`; do not confuse this local artifact with the registry tarball.
- Consumable artifact:
  `/Users/graydonpleasants/.codex/artifacts/geo-polygonize/geo-polygonize-1.1.0-c8f8283.tgz`
- Artifact SHA-256:
  `e8d0c9cca7e1990e4bef7b87d3500a1c52158ccde289b72c22be21078af9fdfa`

Publishing to npm or changing drtwrk production dependencies remains a
separate authorized action.

## Public imports and signatures

The default, slim, and threads entrypoints export the same packed facade:

```ts
function polygonizePackedWithOptions(
  coordinates: Float64Array,
  lineStarts: Uint32Array,
  stride: number,
  options: Partial<PolygonizerOptions>,
  lineIds?: Uint32Array | null,
): PackedPolygonResult;

function polygonizePackedGeometryWithOptions(
  coordinates: Float64Array,
  lineStarts: Uint32Array,
  stride: number,
  options: Partial<PolygonizerOptions>,
  lineIds?: Uint32Array | null,
): PackedPolygonResult;
```

Import `PackedPolygonResult`, `BorrowedPackedBuffers`,
`PackedPolygonSnapshot`, `packedBuffersToGeoJSON`, and
`packedSnapshotTransferList` from `geo-polygonize`,
`geo-polygonize/slim`, or `geo-polygonize/threads`. The low-level generated
binding also exposes `polygonizeGeometryWithOptionsBuffer`; drtwrk should use
the managed facade.

## Initialization and asset loading

For Vite, use the slim entrypoint so the Wasm remains an asset:

```ts
import {
  initBest,
  polygonizePackedGeometryWithOptions,
} from "geo-polygonize/slim";
import scalarUrl from "geo-polygonize/geo_polygonize.wasm?url";
import simdUrl from "geo-polygonize/geo_polygonize_simd.wasm?url";

await initBest(
  { module_or_path: scalarUrl },
  { module_or_path: simdUrl },
);
```

`geo-polygonize` remains the inlined standard entrypoint. The threads
entrypoint exposes the facade after its normal initializer, but still requires
the existing cross-origin isolation and thread-pool setup. This change adds no
new worker, shared-memory, or isolation requirement.

## Minimal consumer

This exact shape was tested from a clean tarball install with both the standard
and slim entrypoints and with TypeScript's bundler module resolution:

```ts
import {
  packedSnapshotTransferList,
  polygonizePackedGeometryWithOptions,
} from "geo-polygonize/slim";

const result = polygonizePackedGeometryWithOptions(
  coordinates,
  lineStarts,
  2,
  options,
  lineIds,
);

try {
  result.withBorrowed((buffers) => {
    drawFlatPolygons(
      buffers.coordinates,
      buffers.ringOffsets,
      buffers.polygonOffsets,
      buffers.stride,
    );
  });

  const snapshot = result.snapshot();
  worker.postMessage(snapshot, packedSnapshotTransferList(snapshot));
} finally {
  result.dispose();
}
```

`withBorrowed` must finish synchronously. Treat its arrays as read-only and do
not retain them. `snapshot` is independent JavaScript storage and may survive
disposal, later Wasm calls, `await`, and transfer. Never transfer Wasm memory.

## Layout and compatibility

The layout identifier is `geo-polygonize-packed-v1`.

- `coordinates` is flat `Float64Array` data with stride 2 or 3.
- `ringOffsets` contains ring starts in coordinate tuples, without a terminal
  sentinel. The final ring ends at `coordinates.length / stride`.
- `polygonOffsets` contains polygon starts in rings, without a terminal
  sentinel. The final polygon ends at `ringOffsets.length`.
- The first ring is the exterior; following rings are holes. Core ordering and
  repeated ring-closure coordinates are preserved.
- `representativeLineIds` is exact `u32` data aligned one-for-one with output
  coordinate tuples. It is not complete provenance; the existing raw layout
  uses `0` when no representative exists for a coordinate.
- Empty output has four empty arrays.

`polygonizePackedWithOptions` preserves the existing exact
`TopologyFingerprintV1`, provenance, dangles, cut edges, invalid rings, and
diagnostics in `fullReport`. `polygonizePackedGeometryWithOptions` omits all of
those report fields by design and retains complete packed polygon geometry,
stride, and representative IDs. Projection choice is separate from semantic
options and both paths use the same core run.

Existing full-result and GeoJSON entrypoints are unchanged. `toGeoJSON()` is an
explicit compatibility allocation and preserves the existing XYZ GeoJSON
shape, adding `z = 0` for stride-2 packed results. Input typed arrays still copy
through generated `wasm-bindgen` glue and Rust still constructs owned
`Line3D`s. The removed work is eager nested output/GeoJSON materialization for
packed consumers; the geometry projection additionally skips report creation.

## Evidence

Reproduce the package and tests with:

```sh
npm run build
npm test
cargo clippy --locked -p geo-polygonize-wasm --target wasm32-unknown-unknown -- -D warnings
WASM_PACKED_SAMPLES=30 node scripts/benchmark_wasm_packed.mjs
npm pack --pack-destination artifacts
```

Machine-readable Node 22.22 and Headless Chrome 152 results on Darwin arm64 are
in `benchmarks/results/wasm-packed-c8f8283.json`. Cold initialization was
115.4 ms in Node and 111.7 ms in Chrome. For the 256-disconnected-polygon
profile, Node p50 was 4.11 ms for the raw full result, 3.67 ms for managed full,
1.25 ms for geometry-only borrowed access, and 1.09 ms including an owned
snapshot; Chrome p50 was 3.1, 3.0, 1.0, and 1.1 ms respectively. These are
boundary/projection measurements, not a faster polygonizer claim. The
geometry-only comparison intentionally omits reports.

The real-Wasm suite covers the canonical fixture, holes, disconnected output,
dangles, cut edges, invalid rings, closure, 2D/3D and Z, representative IDs,
normalized core errors, malformed input, empty output, two live results, later
calls, non-shared memory growth and borrow reacquisition, separate standard and
slim module memories, exception cleanup, double disposal, snapshot survival,
and transfer. Existing CFB fixtures and scalar/SIMD builds run in the same npm
suite. The threads bundle builds and exports the facade; a cross-origin-isolated
threaded browser lifecycle run was not performed for this handoff.

Allocation counts and isolated `wasm-bindgen` slice-copy duration are not
available from the current toolchain. The benchmark records copied input bytes,
packed output bytes, Wasm memory capacity high-water observations, report and
flatten timing, and unsupported metrics rather than inferring them.
