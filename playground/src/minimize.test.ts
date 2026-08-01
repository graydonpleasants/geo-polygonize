import { describe, expect, it, vi } from 'vitest';
import {
  decodeExactFloat,
  extractExactInputSegments,
  fingerprintDifference,
  minimizeExactSegments,
  profileDifferenceSignature,
  sameProfileDifferenceSignature,
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
const normalizedError = (code: string, ids: string[] = []) => ({
  schema_version: 1,
  family: 'topology',
  code,
  stage: 'noding_validation',
  field: null,
  expected: null,
  actual: null,
  limit: null,
  observed: null,
  witness: ids.length > 0 ? { ids, coordinate: null } : null,
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

  it('keeps successful signatures unchanged and preserves complete normalized error pairs', () => {
    expect(profileDifferenceSignature(
      { status: 'success', value: { polygons: ['a'] } },
      { status: 'success', value: { polygons: ['b'] } },
    )).toEqual({ path: '$.polygons[0]', expected: 'a', actual: 'b' });

    const baseline = normalizedError('interior_intersection', ['0x01', '0x02']);
    const comparison = normalizedError('collinear_overlap', ['0x03', '0x04']);
    const signature = profileDifferenceSignature(
      { status: 'error', value: baseline },
      { status: 'error', value: comparison },
    );
    expect(signature).toEqual({ kind: 'normalized_errors', baseline, comparison });
    expect(sameProfileDifferenceSignature(signature!, { ...signature! })).toBe(true);
    expect(profileDifferenceSignature(
      { status: 'error', value: baseline },
      { status: 'error', value: baseline },
    )).toBeNull();
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
