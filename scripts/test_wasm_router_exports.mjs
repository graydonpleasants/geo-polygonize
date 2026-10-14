import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [packageDir, mode] = process.argv.slice(2);
assert(packageDir && ["default", "research"].includes(mode),
  "usage: node scripts/test_wasm_router_exports.mjs <pkg-dir> <default|research>");
const bytes = await readFile(resolve(packageDir, "geo_polygonize_bg.wasm"));
const names = WebAssembly.Module.exports(new WebAssembly.Module(bytes)).map(({ name }) => name);
for (const name of ["__partitionRouterComparison", "__benchmarkPartitionRouter"]) {
  assert.equal(names.includes(name), mode === "research", `${name}: ${mode} exports`);
}
assert(names.includes("polygonizeGeometryWithOptionsBuffer"), "packed geometry export retained");
const wasm = await import(pathToFileURL(resolve(packageDir, "geo_polygonize.js")).href);
await wasm.default({ module_or_path: bytes });
const geometry = wasm.polygonizeGeometryWithOptionsBuffer(
  new Float64Array([0, 0, 1, 0, 1, 1, 0, 1, 0, 0]), new Uint32Array([0]), 2, {},
);
assert.equal(geometry.projection, "geometry");
assert.equal(geometry.polygon_offsets_len(), 1);
geometry.free();

if (mode === "research") {
  const input = JSON.stringify({
    type: "FeatureCollection",
    features: [
      { type: "Feature", geometry: { type: "LineString", coordinates: [[0, 0], [20, 20]] } },
      { type: "Feature", geometry: { type: "LineString", coordinates: [[2, 18], [18, 2]] } },
    ],
  });
  const comparison = JSON.parse(wasm.__partitionRouterComparison(input, 10, 1, {}));
  assert.equal(comparison.schema_version, 1);
  for (const name of ["oracle_difference", "routed_assignment_oracle_difference", "routed_local_snapshot_difference"]) {
    assert.equal(comparison[name], null, name);
  }
  assert.equal(comparison.routed_local_snapshot_checked_partition_count, comparison.assignments.length);
  const benchmark = JSON.parse(wasm.__benchmarkPartitionRouter(input, 10, 1, {}, 1, 2, 2));
  assert.deepEqual(benchmark.router_work, comparison.router_work);
  assert.equal(benchmark.iterations_per_sample, 2);
  assert.equal(benchmark.samples_ms.length, 2);
  assert.throws(() => wasm.__benchmarkPartitionRouter(input, 10, 1, {}, 0, 0, 1), {
    name: "InvalidArgumentType", message: /greater than zero/,
  });
  assert.throws(() => wasm.__benchmarkPartitionRouter(input, 10, 1, {}, 1000000, 1, 1), {
    name: "ResourceLimitExceeded", message: /exceeds 1000000/,
  });
}
console.log(`${mode}: router exports and packed geometry passed`);
