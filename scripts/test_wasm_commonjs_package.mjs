import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { access, cp, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repository = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const consumer = await mkdtemp(resolve(tmpdir(), "geo-polygonize-commonjs-"));
const tarball = process.argv[2];
try {
    await writeFile(resolve(consumer, "package.json"), '{"private":true,"type":"commonjs"}\n');
    if (tarball) {
        execFileSync("npm", ["install", "--ignore-scripts", "--no-audit", "--no-fund", resolve(tarball)], {
            cwd: consumer, stdio: "inherit", timeout: 120_000,
        });
    } else {
        const packageDirectory = resolve(consumer, "node_modules/geo-polygonize");
        await mkdir(packageDirectory, { recursive: true });
        await cp(resolve(repository, "package.json"), resolve(packageDirectory, "package.json"));
        await cp(resolve(repository, "dist"), resolve(packageDirectory, "dist"), { recursive: true });
    }
    for (const path of ["standard/cjs/index.js", "slim/cjs/index_slim.js"]) {
        await assert.rejects(access(resolve(consumer, "node_modules/geo-polygonize/dist", path)), { code: "ENOENT" });
    }
    await cp(resolve(repository, "scripts/release_npm_packed_smoke.mjs"), resolve(consumer, "packed_smoke.mjs"));
    await writeFile(resolve(consumer, "smoke.cjs"), `
const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
(async () => {
    const mode = process.argv[2];
    const name = mode === "standard" ? "geo-polygonize" : "geo-polygonize/slim";
    const api = require(name);
    assert(require.resolve(name).endsWith(".cjs"), "require must resolve genuine CommonJS");
    const bytes = () => readFileSync(require.resolve("geo-polygonize/geo_polygonize.wasm"));
    if (mode === "standard") await api.default();
    else if (mode === "slim-default") await api.initBest();
    else if (mode === "slim-bytes") await api.initBest(bytes());
    else if (mode === "slim-module") await api.initBest({ module: await WebAssembly.compile(bytes()) });
    else if (mode === "slim-simd") await api.initBest(
        { module_or_path: bytes() },
        { module: readFileSync(require.resolve("geo-polygonize/geo_polygonize_simd.wasm")) },
    );
    else throw new Error("unknown initialization mode");
    const { checkPacked } = await import("./packed_smoke.mjs");
    const square = JSON.stringify({ type: "LineString", coordinates: [[0,0],[1,0],[1,1],[0,1],[0,0]] });
    assert.equal(checkPacked(mode, api, true, square), "passed");
    if (mode === "standard") {
        await assert.rejects(api.polygonizeWithOptionsAsync(square, {}), /requires a browser Worker/);
    }
    console.log(mode + "=passed");
})().catch((error) => { console.error(error); process.exitCode = 1; });
`);
    for (const mode of ["standard", "slim-default", "slim-bytes", "slim-module", "slim-simd"]) {
        const output = execFileSync(process.execPath, [resolve(consumer, "smoke.cjs"), mode], {
            cwd: tmpdir(), encoding: "utf8", timeout: 30_000,
        });
        assert(output.includes(`${mode}=passed`));
        process.stdout.write(output);
    }
    const manifest = JSON.parse(await readFile(resolve(consumer, "node_modules/geo-polygonize/package.json"), "utf8"));
    console.log(`CommonJS ${manifest.version} ${tarball ? "installed tarball" : "built package"} smoke passed`);
} finally {
    await rm(consumer, { recursive: true, force: true });
}
