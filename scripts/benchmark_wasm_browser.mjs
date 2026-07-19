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
      const input = JSON.stringify({ type: "FeatureCollection", features });
      const run = () => JSON.parse(wasm.polygonize(input, true, 1e-10, false, false)).features.length;

      for (let index = 0; index < 3; index += 1) run();
      const samplesMs = [];
      let polygonCount = 0;
      for (let index = 0; index < 10; index += 1) {
        const start = performance.now();
        polygonCount = run();
        samplesMs.push(performance.now() - start);
      }
      samplesMs.sort((a, b) => a - b);
      return {
        size,
        polygonCount,
        samples: samplesMs.length,
        minMs: samplesMs[0],
        medianMs: (samplesMs[4] + samplesMs[5]) / 2,
        meanMs: samplesMs.reduce((sum, sample) => sum + sample, 0) / samplesMs.length,
        p95Ms: samplesMs[9],
      };
    });
    return {
      variant: selectedVariant,
      threadCount,
      results,
    };
  }, { selectedVariant: variant, requestedThreadCount, sizes });
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser?.close();
  await new Promise((closed) => server.close(closed));
}
