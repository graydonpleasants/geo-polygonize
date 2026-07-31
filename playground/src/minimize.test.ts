import { describe, expect, it, vi } from 'vitest';
import {
  decodeExactFloat,
  extractExactInputSegments,
  fingerprintDifference,
  minimizeExactSegments,
  type ExactInputSegment,
} from './minimize';
import type { TopologyTraceV1 } from 'geo-polygonize';

const view = new DataView(new ArrayBuffer(8));
const bits = (value: number) => {
  view.setFloat64(0, value);
  return `0x${view.getBigUint64(0).toString(16).padStart(16, '0')}`;
};
const segment = (x: number, sourceId: string, z: number): ExactInputSegment => ({
  start: { x: bits(x), y: bits(5.5), z: bits(z) },
  end: { x: bits(x + 1), y: bits(5.5), z: bits(z + 1) },
  sourceId,
});

describe('profile minimization', () => {
  it('extracts only a complete exact normalized input trace', () => {
    const segments = [segment(0, '0x00000007', 10), segment(2, '0x00000009', 20)];
    const trace: TopologyTraceV1 = {
      schema_version: 1,
      library_version: 'test',
      level: 'full',
      byte_limit: 4096,
      bytes_used: 0,
      truncated: false,
      options: {},
      events: [
        ...segments.map((value, index) => ({
          sequence: index,
          stage: 'noding' as const,
          kind: 'normalized_input_segment',
          payload: { index, start: value.start, end: value.end, source_ids: [value.sourceId] },
        })),
        {
          sequence: 2,
          stage: 'summary',
          kind: 'polygonizer_summary',
          payload: { diagnostics: { input_segment_count: 2 } },
        },
      ],
    };
    expect(extractExactInputSegments(trace)).toEqual(segments);
    (trace.events[2].payload as { diagnostics: { input_segment_count: number } })
      .diagnostics.input_segment_count = 3;
    expect(extractExactInputSegments(trace)).toBeNull();
  });

  it('matches deterministic first structural difference ordering', () => {
    expect(fingerprintDifference(
      { polygons: [{ exterior: ['a'] }] },
      { polygons: [{ exterior: ['b'] }] },
    )).toEqual({ path: '$.polygons[0].exterior[0]', expected: 'a', actual: 'b' });
  });

  it('removes segments and simplifies shared XY without changing source IDs or Z', async () => {
    const input = [
      segment(10.25, '0x00000007', 100),
      segment(20.25, '0x00000009', 200),
      segment(30.25, '0x0000000b', 300),
    ];
    const reductions = vi.fn();
    const result = await minimizeExactSegments(
      input,
      async (candidate) => candidate.some(({ sourceId }) => sourceId === '0x00000009'),
      reductions,
    );

    expect(result).toHaveLength(1);
    expect(result?.[0].sourceId).toBe('0x00000009');
    expect(result?.[0].start.z).toBe(bits(200));
    expect(result?.[0].end.z).toBe(bits(201));
    expect(decodeExactFloat(result![0].start.x)).toBe(0);
    expect(reductions.mock.calls.some(([reduction]) => reduction.phase === 'segments')).toBe(true);
    expect(reductions.mock.calls.some(([reduction]) => reduction.phase === 'coordinates')).toBe(true);
  });
});
