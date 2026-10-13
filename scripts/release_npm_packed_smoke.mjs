function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function equal(actual, expected, message) {
  const ordered = (_key, value) => value && typeof value === "object" && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value).sort(([left], [right]) => left.localeCompare(right)))
    : value;
  assert(JSON.stringify(actual, ordered) === JSON.stringify(expected, ordered), message);
}

function rejects(call, pattern, message) {
  try {
    call();
  } catch (error) {
    if (pattern.test(error.message)) return;
    throw error;
  }
  throw new Error(message);
}

// The same check can run against an installed package in Node or a browser.
export function checkPacked(name, module, required, square) {
  const exports = [
    "polygonizePackedWithOptions",
    "polygonizePackedGeometryWithOptions",
    "packedBuffersToGeoJSON",
  ];
  if (!required && exports.every((key) => module[key] === undefined)) return "not-required";
  for (const key of exports) {
    assert(typeof module[key] === "function", `${name} missing packed export ${key}`);
  }

  const expected = JSON.parse(module.polygonizeWithOptions(square, {}));
  const fingerprint = JSON.parse(module.polygonizeReportWithOptions(square, {}));
  const coordinates = new Float64Array(JSON.parse(square).coordinates.flat());
  const offsets = new Uint32Array([0]);
  const keys = ["coordinates", "ringOffsets", "polygonOffsets", "representativeLineIds"];
  for (const [entrypoint, projection] of [
    ["polygonizePackedWithOptions", "full"],
    ["polygonizePackedGeometryWithOptions", "geometry"],
  ]) {
    const result = module[entrypoint](coordinates, offsets, 2, {});
    try {
      assert(result.projection === projection && !result.disposed, `${name} packed projection`);
      if (projection === "full") {
        equal(result.fullReport.topologyFingerprint, fingerprint, `${name} packed fingerprint`);
      } else {
        assert(result.fullReport === undefined, `${name} geometry projection retained a report`);
      }
      const snapshot = result.snapshot();
      assert(snapshot.layout === "geo-polygonize-packed-v1", `${name} snapshot layout`);
      assert(snapshot.stride === 2 && snapshot.projection === projection, `${name} snapshot projection`);
      const borrowed = result.withBorrowed((buffers) => {
        assert(buffers.stride === 2 && buffers.polygonOffsets.length === 1, `${name} borrowed square`);
        assert(buffers.representativeLineIds.length * buffers.stride === buffers.coordinates.length, `${name} packed representative IDs`);
        for (const key of keys) {
          const type = key === "coordinates" ? Float64Array : Uint32Array;
          assert(buffers[key] instanceof type && snapshot[key] instanceof type, `${name} packed ${key} type`);
          assert(snapshot[key].buffer !== buffers[key].buffer, `${name} snapshot aliases Wasm ${key}`);
        }
        return keys.map((key) => Array.from(buffers[key]));
      });
      equal(keys.map((key) => Array.from(snapshot[key])), borrowed, `${name} snapshot/borrow parity`);
      equal(result.toGeoJSON(), expected, `${name} packed/GeoJSON parity`);
      equal(module.packedBuffersToGeoJSON(snapshot), expected, `${name} snapshot/GeoJSON parity`);
      rejects(() => result.withBorrowed(async () => 1), /synchronously/, `${name} accepted an async borrow`);
      result.dispose();
      result.dispose();
      assert(result.disposed, `${name} packed disposal`);
      for (const call of [() => result.snapshot(), () => result.toGeoJSON(), () => result.withBorrowed(() => 1), () => result.fullReport]) {
        rejects(call, /disposed/, `${name} accepted access after disposal`);
      }
      equal(keys.map((key) => Array.from(snapshot[key])), borrowed, `${name} disposal changed snapshot`);
      equal(module.packedBuffersToGeoJSON(snapshot), expected, `${name} disposed snapshot/GeoJSON parity`);
    } finally {
      result.dispose();
    }
  }
  return "passed";
}
