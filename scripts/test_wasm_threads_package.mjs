import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, resolve, sep } from "node:path";
import { chromium } from "playwright-core";

assert(process.argv[2], "usage: npm run test:threads -- <package.tgz>");
const tarball = resolve(process.argv[2]);
const consumer = await mkdtemp(resolve(tmpdir(), "geo-polygonize-threads-"));
const mime = { ".js": "text/javascript", ".wasm": "application/wasm" };
let browser;
let timeout;
const server = createServer(async (request, response) => {
    response.setHeader("Cross-Origin-Opener-Policy", "same-origin");
    response.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
    const pathname = new URL(request.url, "http://localhost").pathname;
    if (pathname === "/") {
        response.setHeader("Content-Type", "text/html");
        response.end("<!doctype html><title>Threaded package smoke</title>");
        return;
    }
    const path = resolve(consumer, `.${decodeURIComponent(pathname)}`);
    if (!path.startsWith(`${consumer}${sep}`)) {
        response.writeHead(403).end();
        return;
    }
    try {
        const contents = await readFile(path);
        response.setHeader("Content-Type", mime[extname(path)] ?? "application/octet-stream");
        response.end(contents);
    } catch {
        response.writeHead(404).end();
    }
});

try {
    await writeFile(resolve(consumer, "package.json"), '{"private":true,"type":"module"}\n');
    execFileSync("npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund", tarball], {
        cwd: consumer,
        stdio: "inherit",
        timeout: 120_000,
    });
    await new Promise((ready) => server.listen(0, "127.0.0.1", ready));
    browser = await chromium.launch(process.env.CHROME_PATH
        ? { executablePath: process.env.CHROME_PATH, headless: true }
        : { channel: "chrome", headless: true });
    const page = await browser.newPage();
    const workers = [];
    const failedAssets = [];
    page.on("worker", (worker) => workers.push(worker.url()));
    page.on("response", (response) => {
        if (response.status() >= 400 && /\.(js|wasm)$/.test(new URL(response.url()).pathname)) {
            failedAssets.push(`${response.status()} ${response.url()}`);
        }
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const result = await Promise.race([
        page.evaluate(async () => {
            if (!crossOriginIsolated) throw new Error("browser is not cross-origin isolated");
            const api = await import("/node_modules/geo-polygonize/dist/threads/es/index.js");
            await api.default();
            await api.initThreadPool(2);
            const packed = api.polygonizePackedGeometryWithOptions(
                new Float64Array([0, 0, 1, 0, 1, 1, 0, 1, 0, 0]),
                new Uint32Array([0]), 2, {},
            );
            try {
                const sharedMemory = packed.withBorrowed(({ coordinates }) =>
                    coordinates.buffer instanceof SharedArrayBuffer);
                if (!sharedMemory) throw new Error("threaded Wasm memory is not shared");
                const snapshot = packed.snapshot();
                packed.dispose();
                packed.dispose();
                return {
                    sharedMemory,
                    disposed: packed.disposed,
                    polygons: Array.from(snapshot.polygonOffsets),
                    rings: Array.from(snapshot.ringOffsets),
                    coordinates: Array.from(snapshot.coordinates),
                };
            } finally {
                packed.dispose();
            }
        }),
        new Promise((_, reject) => {
            timeout = setTimeout(() => reject(new Error(`threaded package timed out: ${failedAssets.join(", ")}`)), 30_000);
        }),
    ]);
    assert.deepEqual(failedAssets, []);
    assert.equal(workers.length, 2);
    assert(workers.every((url) => url.includes("/dist/threads/pkg-threads/snippets/")));
    assert.equal(result.sharedMemory, true);
    assert.equal(result.disposed, true);
    assert.deepEqual(result.polygons, [0]);
    assert.deepEqual(result.rings, [0]);
    assert.equal(result.coordinates.length, 10);
    assert(result.coordinates.every(Number.isFinite));
    console.log("Threaded tarball smoke passed: shared memory, two workers, packed polygon, snapshot after disposal");
} finally {
    clearTimeout(timeout);
    await browser?.close();
    server.closeAllConnections();
    if (server.listening) await new Promise((closed) => server.close(closed));
    await rm(consumer, { recursive: true, force: true });
}
