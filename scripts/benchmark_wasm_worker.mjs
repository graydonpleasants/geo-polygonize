import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, resolve } from "node:path";
import { chromium } from "playwright-core";

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
const { port } = server.address();
const browser = await chromium.launch(
  process.env.CHROME_PATH
    ? { executablePath: process.env.CHROME_PATH, headless: true }
    : { channel: "chrome", headless: true },
);

try {
  const page = await browser.newPage();
  await page.goto(`http://127.0.0.1:${port}/`);
  const result = await page.evaluate(async () => {
    const input = JSON.stringify({
      type: "FeatureCollection",
      features: [{
        type: "Feature",
        properties: {},
        geometry: {
          type: "LineString",
          coordinates: [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]],
        },
      }],
    });
    const worker = new Worker("/dist/standard/es/polygonize_worker.js", { type: "module" });
    const request = (operation) => new Promise((resolve, reject) => {
      worker.onmessage = ({ data }) => data.error ? reject(data.error) : resolve(data.result);
      worker.onerror = reject;
      worker.postMessage({ id: 0, operation, geojson: input, options: {} });
    });
    const startupStarted = performance.now();
    const variant = await request("initialize");
    const startupMs = performance.now() - startupStarted;
    const polygonizeStarted = performance.now();
    await request("polygons");
    const polygonizeMs = performance.now() - polygonizeStarted;
    worker.terminate();
    return { variant, coldWorkerStartupMs: startupMs, polygonizeMs };
  });
  console.log(JSON.stringify(result, null, 2));
} finally {
  await browser.close();
  await new Promise((closed) => server.close(closed));
}
