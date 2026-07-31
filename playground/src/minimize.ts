import type { TopologyTraceV1 } from 'geo-polygonize';

export type ExactCoordinate = { x: string; y: string; z: string };
export type ExactInputSegment = {
  start: ExactCoordinate;
  end: ExactCoordinate;
  sourceId: string;
};
export type FingerprintDifference = { path: string; expected: unknown; actual: unknown };
export type MinimizationReduction = {
  phase: 'input' | 'segments' | 'coordinates';
  segments: ExactInputSegment[];
};
export type MinimizationResult = {
  signature: FingerprintDifference;
  segments: ExactInputSegment[];
};

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function coordinate(value: unknown): ExactCoordinate | null {
  if (!isObject(value) || typeof value.x !== 'string'
    || typeof value.y !== 'string' || typeof value.z !== 'string') return null;
  return { x: value.x, y: value.y, z: value.z };
}

export function extractExactInputSegments(trace: TopologyTraceV1): ExactInputSegment[] | null {
  const summary = trace.events.find(({ kind }) => kind === 'polygonizer_summary');
  const diagnostics = isObject(summary?.payload) ? summary.payload.diagnostics : null;
  const expectedCount = isObject(diagnostics) ? diagnostics.input_segment_count : null;
  if (typeof expectedCount !== 'number' || !Number.isSafeInteger(expectedCount)) return null;

  const segments: ExactInputSegment[] = [];
  for (const event of trace.events) {
    if (event.kind !== 'normalized_input_segment' || !isObject(event.payload)) continue;
    const start = coordinate(event.payload.start);
    const end = coordinate(event.payload.end);
    const sourceIds = event.payload.source_ids;
    if (event.payload.index !== segments.length || !start || !end
      || !Array.isArray(sourceIds) || sourceIds.length !== 1
      || typeof sourceIds[0] !== 'string') return null;
    segments.push({ start, end, sourceId: sourceIds[0] });
  }
  return segments.length === expectedCount ? segments : null;
}

export function fingerprintDifference(
  expected: unknown,
  actual: unknown,
  path = '$',
): FingerprintDifference | null {
  if (Array.isArray(expected) && Array.isArray(actual)) {
    for (let index = 0; index < Math.max(expected.length, actual.length); index += 1) {
      if (index >= expected.length || index >= actual.length) {
        return { path: `${path}[${index}]`, expected: expected[index] ?? null, actual: actual[index] ?? null };
      }
      const difference = fingerprintDifference(expected[index], actual[index], `${path}[${index}]`);
      if (difference) return difference;
    }
    return null;
  }
  if (isObject(expected) && isObject(actual) && !Array.isArray(expected) && !Array.isArray(actual)) {
    const keys = [...new Set([...Object.keys(expected), ...Object.keys(actual)])]
      .sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
    for (const key of keys) {
      if (!(key in expected) || !(key in actual)) {
        return { path: `${path}.${key}`, expected: expected[key] ?? null, actual: actual[key] ?? null };
      }
      const difference = fingerprintDifference(expected[key], actual[key], `${path}.${key}`);
      if (difference) return difference;
    }
    return null;
  }
  return Object.is(expected, actual) ? null : { path, expected, actual };
}

const floatView = new DataView(new ArrayBuffer(8));
export function decodeExactFloat(bits: string) {
  floatView.setBigUint64(0, BigInt(bits));
  return floatView.getFloat64(0);
}
function encodeExactFloat(value: number) {
  floatView.setFloat64(0, value);
  return `0x${floatView.getBigUint64(0).toString(16).padStart(16, '0')}`;
}

function snapshot(segments: ExactInputSegment[]) {
  return segments.map(({ start, end, sourceId }) => ({
    start: { ...start },
    end: { ...end },
    sourceId,
  }));
}

export async function minimizeExactSegments(
  input: ExactInputSegment[],
  reproduces: (candidate: ExactInputSegment[]) => Promise<boolean>,
  onReduction: (reduction: MinimizationReduction) => void = () => {},
): Promise<ExactInputSegment[] | null> {
  if (!await reproduces(input)) return null;
  if (await reproduces([])) return [];

  let current = snapshot(input);
  let partitions = 2;
  while (current.length >= 2) {
    const chunkSize = Math.ceil(current.length / partitions);
    let reduced = false;
    for (let start = 0; start < current.length; start += chunkSize) {
      const candidate = [...current.slice(0, start), ...current.slice(start + chunkSize)];
      if (await reproduces(candidate)) {
        current = candidate;
        partitions = Math.max(2, partitions - 1);
        reduced = true;
        onReduction({ phase: 'segments', segments: snapshot(current) });
        break;
      }
    }
    if (!reduced) {
      if (partitions >= current.length) break;
      partitions = Math.min(current.length, partitions * 2);
    }
  }

  for (const axis of ['x', 'y'] as const) {
    const values = [...new Set(current.flatMap(({ start, end }) => [start[axis], end[axis]]))]
      .sort((left, right) => (BigInt(left) < BigInt(right) ? -1 : BigInt(left) > BigInt(right) ? 1 : 0));
    for (const bits of values) {
      const value = decodeExactFloat(bits);
      const sign = value < 0 || Object.is(value, -0) ? -1 : 1;
      for (const replacement of [0, sign, Math.trunc(value)]) {
        const replacementBits = encodeExactFloat(replacement);
        if (!Number.isFinite(replacement) || replacementBits === bits) continue;
        const candidate = snapshot(current);
        for (const segment of candidate) {
          for (const endpoint of [segment.start, segment.end]) {
            if (endpoint[axis] === bits) endpoint[axis] = replacementBits;
          }
        }
        if (await reproduces(candidate)) {
          current = candidate;
          onReduction({ phase: 'coordinates', segments: snapshot(current) });
          break;
        }
      }
    }
  }
  return current;
}

export function segmentsToGeojson(segments: ExactInputSegment[]) {
  return {
    type: 'FeatureCollection',
    features: segments.map(({ start, end, sourceId }) => ({
      type: 'Feature',
      properties: { source_id: sourceId },
      geometry: {
        type: 'LineString',
        coordinates: [start, end].map(({ x, y, z }) => (
          [decodeExactFloat(x), decodeExactFloat(y), decodeExactFloat(z)]
        )),
      },
    })),
  };
}
