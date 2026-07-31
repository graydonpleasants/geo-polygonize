import { describe, expect, it, vi } from 'vitest';
import type { PolygonizeTraceReportV1 } from 'geo-polygonize';
import { comparePlaygroundProfiles } from './compare';

function report(polygonCount: number): PolygonizeTraceReportV1 {
  return {
    schema_version: 1,
    topology: {
      schema_version: 1,
      options: {},
      polygons: Array.from({ length: polygonCount }, () => ({
        exterior: [],
        interiors: [],
        exterior_edge_ids: [],
        provenance: null,
      })),
      dangles: [],
      cut_edges: [],
      invalid_rings: [],
      diagnostics: null,
    },
    trace: {
      schema_version: 1,
      library_version: 'test',
      level: 'summary',
      byte_limit: 4096,
      bytes_used: 0,
      truncated: false,
      options: {},
      events: [],
    },
  };
}

describe('profile comparison', () => {
  it('runs the validated and certified profiles and detects topology divergence', async () => {
    const run = vi.fn()
      .mockResolvedValueOnce(JSON.stringify(report(1)))
      .mockResolvedValueOnce(JSON.stringify(report(2)));
    const signal = new AbortController().signal;

    const comparison = await comparePlaygroundProfiles('{}', signal, run);

    expect(run).toHaveBeenCalledTimes(2);
    expect(run.mock.calls.map((call) => call[1].noding.guarantee))
      .toEqual(['Validate', 'CertifiedFixedPrecision']);
    expect(run.mock.calls.every((call) => call[4].signal === signal)).toBe(true);
    expect(comparison.results.map((result) => result.report.topology.polygons.length))
      .toEqual([1, 2]);
    expect(comparison.diverged).toBe(true);
  });
});
