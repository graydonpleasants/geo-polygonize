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
