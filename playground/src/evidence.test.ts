import { describe, expect, it } from 'vitest';
import type { PolygonizeTraceReportV1 } from 'geo-polygonize';
import {
  createDebuggerEvidenceBundle,
  serializeDebuggerEvidence,
} from './evidence';

const report: PolygonizeTraceReportV1 = {
  schema_version: 1,
  topology: {
    schema_version: 1,
    options: { node_input: true },
    polygons: [],
    dangles: [],
    cut_edges: [],
    invalid_rings: [],
    diagnostics: null,
  },
  trace: {
    schema_version: 1,
    library_version: 'test',
    level: 'full',
    byte_limit: 4096,
    bytes_used: 200,
    truncated: false,
    options: { node_input: true },
    events: [{
      sequence: 0,
      stage: 'noding',
      kind: 'normalized_input_segment',
      payload: { start: { x: '0x0', y: '0x0', z: '0x4024000000000000' } },
    }],
  },
};

describe('debugger evidence bundles', () => {
  it('serializes current evidence deterministically without claiming golden truth', () => {
    const bundle = createDebuggerEvidenceBundle({
      input: {
        type: 'FeatureCollection',
        features: [{
          properties: { source_id: 7 },
          geometry: { coordinates: [[0, 0, 10], [1, 0, 20]], type: 'LineString' },
          type: 'Feature',
        }],
      },
      requestedOptions: { node_input: true },
      topology: report.topology,
      trace: report.trace,
      comparison: { results: [{ label: 'test', report }], diverged: false },
      normalizedError: null,
    });

    const encoded = serializeDebuggerEvidence(bundle);
    const decoded = JSON.parse(encoded);
    expect(decoded).toMatchObject({
      schema_version: 1,
      kind: 'geo_polygonize_debugger_evidence',
      input: { features: [{ properties: { source_id: 7 } }] },
      requested_options: { node_input: true },
      trace_run: { trace: { library_version: 'test' } },
    });
    expect(decoded.input.features[0].geometry.coordinates[0][2]).toBe(10);
    expect(encoded).not.toContain('golden');
    expect(encoded).not.toContain('reference_metrics');
    expect(serializeDebuggerEvidence(decoded)).toBe(encoded);
  });

  it('rejects an unpaired topology or trace', () => {
    expect(() => createDebuggerEvidenceBundle({
      input: {},
      requestedOptions: {},
      topology: report.topology,
      trace: null,
      comparison: null,
      normalizedError: null,
    })).toThrow('requires topology and trace together');
  });
});
