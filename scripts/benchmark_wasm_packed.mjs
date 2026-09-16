import { createServer } from "node:http";
import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import { chromium } from "playwright-core";
import { runPackedBenchmark } from "./benchmark_wasm_packed_runner.mjs";

const samples = Number(process.env.WASM_PACKED_SAMPLES ?? 30);
if (!Number.isInteger(samples) || samples < 2) throw new Error("WASM_PACKED_SAMPLES must be at least 2");

const initializedAt = performance.now();
const nodeWasm = await import("../dist/standard/es/index.js");
await nodeWasm.default();
const nodeColdInitializationMs = performance.now() - initializedAt;
const node = await runPackedBenchmark(nodeWasm, samples);

const root = process.cwd();
const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, "http://localhost").pathname;
  if (pathname === "/") return response.end("<!doctype html>");
  const path = resolve(root, `.${pathname}`);
  if (!path.startsWith(`${root}/`)) return response.writeHead(403).end();
  response.setHeader(
    "Content-Type",
    extname(path) === ".wasm" ? "application/wasm" : "text/javascript",
  );
  try {
    response.end(await readFile(path));
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((ready) => server.listen(0, "127.0.0.1", ready));

let browser;
let browserResult;
try {
  const { port } = server.address();
  browser = await chromium.launch(process.env.CHROME_PATH
    ? { executablePath: process.env.CHROME_PATH, headless: true }
    : { channel: "chrome", headless: true });
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/`);
  browserResult = await page.evaluate(async (sampleCount) => {
    const initializedAt = performance.now();
    const wasm = await import("/dist/standard/es/index.js");
    await wasm.default();
    const coldInitializationMs = performance.now() - initializedAt;
    const { runPackedBenchmark } = await import("/scripts/benchmark_wasm_packed_runner.mjs");
    return {
      coldInitializationMs,
      workloads: await runPackedBenchmark(wasm, sampleCount),
      userAgent: navigator.userAgent,
    };
  }, samples);
} finally {
  await browser?.close();
  await new Promise((closed) => server.close(closed));
}

console.log(JSON.stringify({
  schemaVersion: 1,
  commit: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(),
  generatedAt: new Date().toISOString(),
  packageEntrypoint: "geo-polygonize default standard ESM bundle",
  initializationMode: "inlined scalar/SIMD auto-selection",
  options: "diagnostics timings and complete provenance enabled; otherwise canonical defaults",
  runtime: {
    node: process.version,
    platform: `${process.platform}-${process.arch}`,
    selectedVariant: "automatic scalar/SIMD selection",
  },
  node: { coldInitializationMs: nodeColdInitializationMs, workloads: node },
  browser: browserResult,
}, null, 2));
