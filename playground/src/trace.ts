import type { TopologyTraceV1 } from 'geo-polygonize';

export type TraceCoordinate = [number, number];

export type TraceLine = {
  sequence: number;
  start: TraceCoordinate;
  end: TraceCoordinate;
  sourceIds: string[];
};

export type TracePoint = {
  sequence: number;
  coordinate: TraceCoordinate;
  sourceIds: string[];
};

export type PlaygroundTraceLayers = {
  snappedLines: TraceLine[];
  hotPixels: TracePoint[];
  splitPoints: TracePoint[];
  graphEdges: TraceLine[];
};

const coordinateView = new DataView(new ArrayBuffer(8));

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function decodeTraceCoordinate(value: unknown): TraceCoordinate | null {
  if (!isObject(value) || typeof value.x !== 'string' || typeof value.y !== 'string') {
    return null;
  }
  try {
    const number = (bits: string) => {
      coordinateView.setBigUint64(0, BigInt(bits));
      return coordinateView.getFloat64(0);
    };
    const coordinate: TraceCoordinate = [number(value.x), number(value.y)];
    return coordinate.every(Number.isFinite) ? coordinate : null;
  } catch {
    return null;
  }
}

function sourceIds(payload: Record<string, unknown>) {
  return Array.isArray(payload.source_ids)
    ? payload.source_ids.filter((value): value is string => typeof value === 'string')
    : [];
}

function line(sequence: number, payload: unknown): TraceLine | null {
  if (!isObject(payload)) return null;
  const start = decodeTraceCoordinate(payload.start);
  const end = decodeTraceCoordinate(payload.end);
  return start && end ? { sequence, start, end, sourceIds: sourceIds(payload) } : null;
}

export function extractTraceLayers(trace: TopologyTraceV1): PlaygroundTraceLayers {
  const snappedLines: TraceLine[] = [];
  const hotPixels: TracePoint[] = [];
  const graphEdges: TraceLine[] = [];
  const splitPoints = new Map<string, TracePoint>();

  for (const event of trace.events) {
    if (event.kind === 'fixed_grid_segment' || event.kind === 'noded_segment') {
      const value = line(event.sequence, event.payload);
      if (value) snappedLines.push(value);
      continue;
    }
    if (event.kind === 'dissolved_edge') {
      const value = line(event.sequence, event.payload);
      if (value) graphEdges.push(value);
      continue;
    }
    if (event.kind === 'certified_hot_pixel' && isObject(event.payload)) {
      const coordinate = decodeTraceCoordinate(event.payload.coordinate);
      if (coordinate) hotPixels.push({ sequence: event.sequence, coordinate, sourceIds: [] });
      continue;
    }
    if (!event.kind.endsWith('_candidate_pair') || !isObject(event.payload)) continue;
    const witness = event.payload.witness;
    if (!isObject(witness) || witness.kind !== 'point') continue;
    const coordinate = decodeTraceCoordinate(witness.coordinate);
    if (!coordinate) continue;
    const key = JSON.stringify(coordinate);
    const ids = [event.payload.first_source_id, event.payload.second_source_id]
      .filter((value): value is string => typeof value === 'string');
    const existing = splitPoints.get(key);
    if (existing) {
      existing.sourceIds = [...new Set([...existing.sourceIds, ...ids])].sort();
    } else {
      splitPoints.set(key, { sequence: event.sequence, coordinate, sourceIds: ids.sort() });
    }
  }

  return { snappedLines, hotPixels, splitPoints: [...splitPoints.values()], graphEdges };
}
