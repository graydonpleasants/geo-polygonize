import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import { chromium } from "playwright-core";

const variant = process.argv[2];
if (!new Set(["scalar", "threads"]).has(variant)) {
  throw new Error("usage: node scripts/benchmark_wasm_browser.mjs <scalar|threads> [thread-count]");
}
const requestedThreadCount = Number(process.argv[3] ?? 4);
if (!Number.isInteger(requestedThreadCount) || requestedThreadCount < 1) {
  throw new Error("thread-count must be a positive integer");
}
const sizes = (process.env.WASM_BENCH_SIZES ?? "100,200,500,1000")
  .split(",")
  .map(Number);
if (sizes.some((size) => !Number.isInteger(size) || size < 1)) {
  throw new Error("WASM_BENCH_SIZES must be comma-separated positive integers");
}

const root = process.cwd();
const contentTypes = new Map([
  [".html", "text/html"],
  [".js", "text/javascript"],
  [".wasm", "application/wasm"],
]);

const server = createServer(async (request, response) => {
  response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
  response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
  response.setHeader("Cross-Origin-Resource-Policy", "same-origin");
  response.setHeader("Cache-Control", "no-store");

  let pathname = new URL(request.url, "http://localhost").pathname;
  if (pathname === "/") {
    response.setHeader("Content-Type", "text/html");
    response.end("<!doctype html><title>geo-polygonize benchmark</title>");
    return;
  }
  // wasm-bindgen-rayon's generated worker imports the package directory.
  if (pathname === "/pkg-threads/") pathname += "geo_polygonize.js";

  const path = resolve(root, `.${decodeURIComponent(pathname)}`);
  if (!path.startsWith(`${root}/`)) {
    response.writeHead(403).end();
    return;
  }

  try {
    response.setHeader("Content-Type", contentTypes.get(extname(path)) ?? "application/octet-stream");
    response.end(await readFile(path));
  } catch {
    response.writeHead(404).end();
  }
});

await new Promise((ready) => server.listen(0, "127.0.0.1", ready));
const { port } = server.address();
const launchOptions = process.env.CHROME_PATH
  ? { executablePath: process.env.CHROME_PATH, headless: true }
  : { channel: "chrome", headless: true };

let browser;
try {
  browser = await chromium.launch(launchOptions);
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/`);
  const result = await page.evaluate(async ({ selectedVariant, requestedThreadCount, sizes }) => {
    const wasm = await import(`/pkg-${selectedVariant}/geo_polygonize.js`);
    const instance = await wasm.default();

    let threadCount = 1;
    if (selectedVariant === "threads") {
      if (!crossOriginIsolated) throw new Error("threaded Wasm requires cross-origin isolation");
      if (!(instance.memory.buffer instanceof SharedArrayBuffer)) {
        throw new Error("threaded build did not export shared Wasm memory");
      }
      threadCount = Math.min(requestedThreadCount, navigator.hardwareConcurrency || requestedThreadCount);
      await wasm.initThreadPool(threadCount);
    }

    const summarize = (samples) => {
      const sorted = [...samples].sort((a, b) => a - b);
      const middle = sorted.length / 2;
      return {
        minMs: sorted[0],
        medianMs: (sorted[middle - 1] + sorted[middle]) / 2,
        meanMs: sorted.reduce((sum, sample) => sum + sample, 0) / sorted.length,
        p95Ms: sorted[Math.ceil(sorted.length * 0.95) - 1],
      };
    };
    const durationMs = ({ secs, nanos }) => Number(secs) * 1_000 + nanos / 1_000_000;
    const nodingOptions = {
      node_input: true,
      snap_grid_size: 1e-10,
      pre_snap_tolerance: 0,
      extract_only_polygonal: false,
      snap_strategy: "Grid",
      noding: { backend: "Snap" },
      containment: { touch_policy: "AllowPointTouchDisallowEdgeShare" },
      determinism: {
        canonical_sort: false,
        canonical_ring_rotation: false,
        stable_tie_breaks: false,
      },
      diagnostics: { enabled: false, report_mode: false, timings: true },
      provenance: { enabled: false, include_boundary_line_ids: false },
      input_profile_id: null,
    };
    const profileSize = Math.max(...sizes);
    const results = sizes.map((size) => {
      let state = 42;
      const random = () => {
        state += 0x6d2b79f5;
        let value = state;
        value = Math.imul(value ^ (value >>> 15), value | 1);
        value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
        return ((value ^ (value >>> 14)) >>> 0) / 4294967296;
      };
      const features = Array.from({ length: size }, () => ({
        type: "Feature",
        properties: {},
        geometry: {
          type: "LineString",
          coordinates: [
            [random() * 100, random() * 100],
            [random() * 100, random() * 100],
          ],
        },
      }));
      const collection = { type: "FeatureCollection", features };
      const input = JSON.stringify(collection);
      const run = () => {
        const wasmStarted = performance.now();
        const output = wasm.polygonize(input, true, 1e-10, false, false);
        const wasmMs = performance.now() - wasmStarted;
        const parseStarted = performance.now();
        const polygonCount = JSON.parse(output).features.length;
        return { polygonCount, wasmMs, parseMs: performance.now() - parseStarted };
      };

      for (let index = 0; index < 3; index += 1) run();
      const wasmSamplesMs = [];
      const parseSamplesMs = [];
      const totalSamplesMs = [];
      let polygonCount = 0;
      for (let index = 0; index < 10; index += 1) {
        const sample = run();
        polygonCount = sample.polygonCount;
        wasmSamplesMs.push(sample.wasmMs);
        parseSamplesMs.push(sample.parseMs);
        totalSamplesMs.push(sample.wasmMs + sample.parseMs);
      }
      const wasmSummary = summarize(wasmSamplesMs);
      const parseSummary = summarize(parseSamplesMs);
      const totalSummary = summarize(totalSamplesMs);
      const result = {
        size,
        polygonCount,
        samples: wasmSamplesMs.length,
        ...totalSummary,
      };

      if (size === profileSize) {
        const coords = new Float64Array(size * 4);
        for (let index = 0; index < size; index += 1) {
          coords.set(features[index].geometry.coordinates.flat(), index * 4);
        }
        const offsets = Uint32Array.from({ length: size }, (_, index) => index * 2);
        const runBuffer = () => {
          const started = performance.now();
          const output = wasm.polygonizeWithOptionsBuffer(coords, offsets, 2, nodingOptions);
          const totalMs = performance.now() - started;
          const diagnostics = output.diagnostics;
          const count = output.polygon_offsets_len();
          output.free();
          return {
            count,
            totalMs,
            phases: Object.fromEntries(
              Object.entries(diagnostics.phase_times).map(([name, duration]) => [name, durationMs(duration)]),
            ),
          };
        };
        for (let index = 0; index < 3; index += 1) runBuffer();
        const bufferSamples = [];
        for (let index = 0; index < 10; index += 1) bufferSamples.push(runBuffer());
        if (bufferSamples.some((sample) => sample.count !== polygonCount)) {
          throw new Error("GeoJSON and buffer polygon counts differ");
        }
        const stringifySamplesMs = [];
        for (let index = 0; index < 10; index += 1) {
          const started = performance.now();
          JSON.stringify(collection);
          stringifySamplesMs.push(performance.now() - started);
        }
        result.stageProfile = {
          jsonInputStringify: summarize(stringifySamplesMs),
          geoJsonWasm: wasmSummary,
          jsonOutputParse: parseSummary,
          bufferTotal: summarize(bufferSamples.map((sample) => sample.totalMs)),
          corePhases: Object.fromEntries(
            Object.keys(bufferSamples[0].phases).map((phase) => [
              phase,
              summarize(bufferSamples.map((sample) => sample.phases[phase])),
            ]),
          ),
        };
      }
      return result;
    });

    const sparse = Array.from({ length: 512 }, (_, i) => [[0, i * 2], [1, i * 2 + 0.5]]);
    const dense = Array.from({ length: 256 }, (_, i) => {
      const angle = Math.PI * i / 256;
      return [[-Math.cos(angle), -Math.sin(angle)], [Math.cos(angle), Math.sin(angle)]];
    });
    const skewed = Array.from({ length: 600 }, (_, i) => {
      const end = i * 0.0001;
      return [[0, 0], [end, end + 0.00001]];
    });
    skewed.push([[100, 100], [101, 101]]);
    const crossing = Array.from({ length: 4 * 4 * 2 }, (_, index) => {
      const cell = Math.floor(index / 2);
      const x = Math.floor(cell / 4) * 2;
      const y = (cell % 4) * 2;
      return index % 2 === 0
        ? [[x, y], [x + 1, y + 1]]
        : [[x + 1, y], [x, y + 1]];
    });
    const nodingWorkloads = Object.entries({ sparse, dense, skewed, crossing }).map(
      ([name, lines]) => {
        const coords = new Float64Array(lines.flat(2));
        const offsets = Uint32Array.from({ length: lines.length }, (_, index) => index * 2);
        const run = () => {
          const started = performance.now();
          const output = wasm.polygonizeWithOptionsBuffer(coords, offsets, 2, nodingOptions);
          const totalMs = performance.now() - started;
          const ingestAndNodeMs = durationMs(output.diagnostics.phase_times.ingest_and_node);
          output.free();
          return { totalMs, ingestAndNodeMs };
        };
        for (let index = 0; index < 3; index += 1) run();
        const samples = Array.from({ length: 10 }, run);
        return {
          name,
          segments: lines.length,
          total: summarize(samples.map(({ totalMs }) => totalMs)),
          ingestAndNode: summarize(samples.map(({ ingestAndNodeMs }) => ingestAndNodeMs)),
        };
      },
    );
    return {
      variant: selectedVariant,
      threadCount,
      results,
      nodingWorkloads,
    };
  }, { selectedVariant: variant, requestedThreadCount, sizes });
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser?.close();
  await new Promise((closed) => server.close(closed));
}
