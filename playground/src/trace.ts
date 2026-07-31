import type { PolygonizeTraceReportV1, TopologyTraceV1 } from 'geo-polygonize';

export const PLAYGROUND_TRACE_BYTE_LIMIT = 4 * 1024 * 1024;

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

export type ZReconciliationDecision = {
  sequence: number;
  coordinate: TraceCoordinate;
  coordinateBits: { x: string; y: string };
  policy: string;
  conflictTolerance: string;
  candidates: Array<{ sourceId: string; z: string }>;
  conflict: boolean;
  retainedZ: string;
};

export type ExecutionEvidence = {
  phase_times: Record<string, unknown>;
  noding_work_stats: Record<string, unknown>;
  noding_iterations: unknown[];
  trace_budget: Record<string, unknown>;
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

export function parsePlaygroundTraceReport(text: string): PolygonizeTraceReportV1 {
  const value: unknown = JSON.parse(text);
  if (!isObject(value) || value.schema_version !== 1
    || !isObject(value.topology) || value.topology.schema_version !== 1
    || !isObject(value.trace) || value.trace.schema_version !== 1
    || !Array.isArray(value.trace.events)) {
    throw new Error('Unsupported topology trace report');
  }
  return value as unknown as PolygonizeTraceReportV1;
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

export function extractZReconciliationDecisions(
  trace: TopologyTraceV1,
): ZReconciliationDecision[] {
  return trace.events.flatMap((event) => {
    if (event.kind !== 'z_reconciliation' || !isObject(event.payload)) return [];
    const {
      x,
      y,
      policy,
      conflict_tolerance: conflictTolerance,
      candidates,
      conflict,
      retained_z: retainedZ,
    } = event.payload;
    const coordinate = decodeTraceCoordinate({ x, y });
    if (!coordinate || typeof x !== 'string' || typeof y !== 'string'
      || typeof policy !== 'string' || typeof conflictTolerance !== 'string'
      || !Array.isArray(candidates) || typeof conflict !== 'boolean'
      || typeof retainedZ !== 'string') return [];
    const parsedCandidates = candidates.flatMap((candidate) => (
      isObject(candidate) && typeof candidate.source_id === 'string'
        && typeof candidate.z === 'string'
        ? [{ sourceId: candidate.source_id, z: candidate.z }]
        : []
    ));
    if (parsedCandidates.length !== candidates.length) return [];
    return [{
      sequence: event.sequence,
      coordinate,
      coordinateBits: { x, y },
      policy,
      conflictTolerance,
      candidates: parsedCandidates,
      conflict,
      retainedZ,
    }];
  });
}

export function extractExecutionEvidence(trace: TopologyTraceV1): ExecutionEvidence | null {
  const summary = trace.events.find(({ kind }) => kind === 'polygonizer_summary');
  if (!summary || !isObject(summary.payload)) return null;
  const { diagnostics, trace_budget: traceBudget } = summary.payload;
  if (!isObject(diagnostics) || !isObject(traceBudget)
    || !isObject(diagnostics.phase_times) || !isObject(diagnostics.noding_work_stats)
    || !Array.isArray(diagnostics.noding_iterations)) return null;
  return {
    phase_times: diagnostics.phase_times,
    noding_work_stats: diagnostics.noding_work_stats,
    noding_iterations: diagnostics.noding_iterations,
    trace_budget: traceBudget,
  };
}
