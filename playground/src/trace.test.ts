import { describe, expect, it } from 'vitest';
import type { TopologyTraceV1 } from 'geo-polygonize';
import {
  decodeTraceCoordinate,
  extractTraceLayers,
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
