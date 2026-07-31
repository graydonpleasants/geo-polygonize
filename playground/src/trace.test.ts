import { describe, expect, it } from 'vitest';
import type { TopologyTraceV1 } from 'geo-polygonize';
import {
  decodeTraceCoordinate,
  extractExecutionEvidence,
  extractTraceLayers,
  extractZReconciliationDecisions,
  parsePlaygroundTraceReport,
} from './trace';

const view = new DataView(new ArrayBuffer(8));
const bits = (value: number) => {
  view.setFloat64(0, value);
  return `0x${view.getBigUint64(0).toString(16).padStart(16, '0')}`;
};
const point = (x: number, y: number) => ({
  x: bits(x),
  y: bits(y),
  z: null,
});

function trace(events: TopologyTraceV1['events']): TopologyTraceV1 {
  return {
    schema_version: 1,
    library_version: 'test',
    level: 'full',
    byte_limit: 4096,
    bytes_used: 0,
    truncated: false,
    options: {},
    events,
  };
}

describe('trace layers', () => {
  it('decodes physical noding and graph events', () => {
    const start = point(0, 0);
    const end = point(2, 1);
    const result = extractTraceLayers(trace([
      { sequence: 0, stage: 'noding', kind: 'fixed_grid_segment', payload: { start, end, source_ids: ['a'] } },
      { sequence: 1, stage: 'noding', kind: 'certified_hot_pixel', payload: { coordinate: end } },
      { sequence: 2, stage: 'noding', kind: 'certified_candidate_pair', payload: { first_source_id: 'b', second_source_id: 'a', witness: { kind: 'point', coordinate: end } } },
      { sequence: 3, stage: 'graph', kind: 'dissolved_edge', payload: { start, end, source_ids: ['a', 'b'] } },
    ]));

    expect(result.snappedLines).toEqual([{ sequence: 0, start: [0, 0], end: [2, 1], sourceIds: ['a'] }]);
    expect(result.hotPixels[0].coordinate).toEqual([2, 1]);
    expect(result.splitPoints).toEqual([{ sequence: 2, coordinate: [2, 1], sourceIds: ['a', 'b'] }]);
    expect(result.graphEdges[0].sourceIds).toEqual(['a', 'b']);
  });

  it('ignores malformed coordinates', () => {
    expect(decodeTraceCoordinate({ x: 'bad', y: '0x0' })).toBeNull();
  });

  it('rejects unsupported trace envelopes', () => {
    expect(() => parsePlaygroundTraceReport('{"schema_version":2}'))
      .toThrow('Unsupported topology trace report');
  });
});

describe('Z reconciliation decisions', () => {
  it('preserves physical order and exact evidence bits', () => {
    const result = extractZReconciliationDecisions(trace([
      {
        sequence: 4,
        stage: 'noding',
        kind: 'z_reconciliation',
        payload: {
          x: bits(1),
          y: bits(2),
          policy: 'InterpolateAlongEdge',
          conflict_tolerance: bits(5),
          candidates: [
            { source_id: '0x00000009', z: bits(20) },
            { source_id: '0x00000007', z: bits(30) },
          ],
          conflict: true,
          retained_z: bits(30),
        },
      },
      {
        sequence: 5,
        stage: 'noding',
        kind: 'z_reconciliation',
        payload: {
          x: bits(3),
          y: bits(4),
          policy: 'First',
          conflict_tolerance: bits(0),
          candidates: [{ source_id: '0x00000001', z: bits(10) }],
          conflict: false,
          retained_z: bits(10),
        },
      },
    ]));

    expect(result.map(({ sequence }) => sequence)).toEqual([4, 5]);
    expect(result[0]).toMatchObject({
      coordinate: [1, 2],
      coordinateBits: { x: bits(1), y: bits(2) },
      policy: 'InterpolateAlongEdge',
      conflictTolerance: bits(5),
      candidates: [
        { sourceId: '0x00000009', z: bits(20) },
        { sourceId: '0x00000007', z: bits(30) },
      ],
      conflict: true,
      retainedZ: bits(30),
    });
  });

  it('ignores malformed decisions instead of inventing evidence', () => {
    expect(extractZReconciliationDecisions(trace([
      {
        sequence: 0,
        stage: 'noding',
        kind: 'z_reconciliation',
        payload: {
          x: bits(1),
          y: bits(2),
          policy: 'First',
          conflict_tolerance: bits(0),
          candidates: [{ source_id: 'a' }],
          conflict: false,
          retained_z: bits(3),
        },
      },
    ]))).toEqual([]);
  });
});

describe('execution evidence', () => {
  it('extracts only existing summary timing, work, and budget fields', () => {
    const phaseTimes = { ingest_and_node: { secs: 0, nanos: 42 } };
    const workStats = { candidate_pairs: 7, exact_intersection_calls: 3 };
    const traceBudget = {
      total: { limit: 4096, bytes_used_before_summary: 1200, truncated_before_summary: false },
      noding: { limit: 2000, bytes_used_before_summary: 900, truncated_before_summary: false },
    };
    expect(extractExecutionEvidence(trace([
      {
        sequence: 8,
        stage: 'summary',
        kind: 'polygonizer_summary',
        payload: {
          diagnostics: {
            phase_times: phaseTimes,
            noding_work_stats: workStats,
            noding_iterations: 2,
          },
          trace_budget: traceBudget,
        },
      },
    ]))).toEqual({
      phase_times: phaseTimes,
      noding_work_stats: workStats,
      noding_iterations: 2,
      trace_budget: traceBudget,
    });
  });

  it('rejects incomplete summary evidence', () => {
    expect(extractExecutionEvidence(trace([
      {
        sequence: 0,
        stage: 'summary',
        kind: 'polygonizer_summary',
        payload: { diagnostics: {}, trace_budget: {} },
      },
    ]))).toBeNull();
  });
});
