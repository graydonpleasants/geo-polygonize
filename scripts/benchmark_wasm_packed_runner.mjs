function summarize(samples) {
  const sorted = [...samples].sort((left, right) => left - right);
  return {
    p50Ms: sorted[Math.floor(sorted.length * 0.5)],
    p95Ms: sorted[Math.ceil(sorted.length * 0.95) - 1],
  };
}

function fixture(name, squareCount, dirty = false) {
  const lines = Array.from({ length: squareCount }, (_, index) => {
    const x = (index % 32) * 2;
    const y = Math.floor(index / 32) * 2;
    return [[x, y], [x + 1, y], [x + 1, y + 1], [x, y + 1], [x, y]];
  });
  if (dirty) lines.push([[0.25, 0.25], [0.75, 0.75]], [[1000, 1000], [1001, 1000]]);
  const packStarted = performance.now();
  const coordinates = new Float64Array(lines.flat(2));
  const offsets = new Uint32Array(lines.length);
  const lineIds = new Uint32Array(lines.length);
  let coordinateOffset = 0;
  for (let index = 0; index < lines.length; index += 1) {
    offsets[index] = coordinateOffset;
    coordinateOffset += lines[index].length;
    lineIds[index] = index + 1;
  }
  const inputPackingMs = performance.now() - packStarted;
  return {
    name,
    lines,
    coordinates,
    offsets,
    lineIds,
    inputPackingMs,
    geojson: JSON.stringify({
      type: "FeatureCollection",
      features: lines.map((coordinates) => ({
        type: "Feature",
        properties: null,
        geometry: { type: "LineString", coordinates },
      })),
    }),
  };
}

function sample(samples, run) {
  for (let index = 0; index < 3; index += 1) run();
  const values = Array.from({ length: samples }, run);
  return {
    elapsed: summarize(values.map((value) => value.elapsedMs)),
    last: values.at(-1),
    values,
  };
}

export async function runPackedBenchmark(wasm, samples = 30) {
  const options = {
    diagnostics: { enabled: true, timings: true },
    provenance: { enabled: true, include_boundary_line_ids: true },
  };
  const workloads = [
    fixture("interactive", 1),
    fixture("disconnected-256", 256),
    fixture("dirty", 16, true),
  ];

  return workloads.map((workload) => {
    const args = [
      workload.coordinates,
      workload.offsets,
      2,
      options,
      workload.lineIds,
    ];
    const geojson = sample(samples, () => {
      const started = performance.now();
      const result = JSON.parse(wasm.polygonizeWithOptions(workload.geojson, options));
      return { elapsedMs: performance.now() - started, polygonCount: result.features.length };
    });
    const rawFull = sample(samples, () => {
      const started = performance.now();
      const result = wasm.polygonizeWithOptionsBuffer(...args);
      const polygonCount = result.polygon_offsets_len();
      const metrics = result.boundary_metrics;
      const fingerprint = result.topology_fingerprint;
      result.free();
      return { elapsedMs: performance.now() - started, polygonCount, metrics, fingerprint };
    });
    const managedFull = sample(samples, () => {
      const started = performance.now();
      const result = wasm.polygonizePackedWithOptions(...args);
      const consumeStarted = performance.now();
      const polygonCount = result.withBorrowed((buffers) => buffers.polygonOffsets.length);
      const consumeMs = performance.now() - consumeStarted;
      const metrics = result.boundaryMetrics;
      const fingerprint = result.fullReport.topologyFingerprint;
      result.dispose();
      return { elapsedMs: performance.now() - started, polygonCount, consumeMs, metrics, fingerprint };
    });
    const geometry = sample(samples, () => {
      const started = performance.now();
      const result = wasm.polygonizePackedGeometryWithOptions(...args);
      const consumeStarted = performance.now();
      const memoryBytes = result.withBorrowed((buffers) => {
        void buffers.coordinates[0];
        return buffers.coordinates.buffer.byteLength;
      });
      const consumeMs = performance.now() - consumeStarted;
      const polygonCount = result.withBorrowed((buffers) => buffers.polygonOffsets.length);
      const metrics = result.boundaryMetrics;
      result.dispose();
      return { elapsedMs: performance.now() - started, polygonCount, consumeMs, memoryBytes, metrics };
    });
    const snapshot = sample(samples, () => {
      const started = performance.now();
      const result = wasm.polygonizePackedGeometryWithOptions(...args);
      const snapshotStarted = performance.now();
      const owned = result.snapshot();
      const snapshotMs = performance.now() - snapshotStarted;
      result.dispose();
      return {
        elapsedMs: performance.now() - started,
        polygonCount: owned.polygonOffsets.length,
        snapshotMs,
        outputBytes: owned.coordinates.byteLength
          + owned.ringOffsets.byteLength
          + owned.polygonOffsets.byteLength
          + owned.representativeLineIds.byteLength,
      };
    });

    const counts = [geojson, rawFull, managedFull, geometry, snapshot]
      .map((result) => result.last.polygonCount);
    if (new Set(counts).size !== 1) throw new Error(`${workload.name}: polygon counts differ`);
    if (JSON.stringify(rawFull.last.fingerprint) !== JSON.stringify(managedFull.last.fingerprint)) {
      throw new Error(`${workload.name}: full-result fingerprints differ`);
    }

    return {
      name: workload.name,
      lineCount: workload.lines.length,
      polygonCount: counts[0],
      samples,
      knownInputCopyBytes: workload.coordinates.byteLength
        + workload.offsets.byteLength
        + workload.lineIds.byteLength,
      inputPackingMs: workload.inputPackingMs,
      timings: {
        geojson: geojson.elapsed,
        rawFull: rawFull.elapsed,
        managedFull: managedFull.elapsed,
        geometry: geometry.elapsed,
        snapshot: snapshot.elapsed,
        managedBorrowConsume: summarize(managedFull.values.map((value) => value.consumeMs)),
        geometryBorrowConsume: summarize(geometry.values.map((value) => value.consumeMs)),
        snapshotCopy: summarize(snapshot.values.map((value) => value.snapshotMs)),
      },
      boundary: {
        rawFull: rawFull.last.metrics,
        managedFull: managedFull.last.metrics,
        geometry: geometry.last.metrics,
      },
      packedOutputBytes: snapshot.last.outputBytes,
      wasmMemoryHighWaterBytes: geometry.last.memoryBytes,
      unsupportedMetrics: [
        "separate wasm-bindgen slice-copy duration",
        "Rust allocator counts",
        "browser JS heap when performance.memory is unavailable",
      ],
    };
  });
}
