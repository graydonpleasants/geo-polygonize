import { describe, expect, it } from 'vitest';
import type { PolygonizeTraceReportV1 } from 'geo-polygonize';
import {
  createDebuggerEvidenceBundle,
  createDebuggerFixtureBundle,
  serializeDebuggerEvidence,
  serializeDebuggerFixture,
} from './evidence';
import type { ExactInputSegment } from './minimize';

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
      comparison: {
        results: [{
          label: 'test',
          options: { node_input: true },
          status: 'success',
          report,
        }],
        diverged: false,
      },
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

  it('exports exact minimized compatibility fixtures deterministically', () => {
    const segments: ExactInputSegment[] = [{
      start: { x: '0x0000000000000000', y: '0x0000000000000000', z: '0x4024000000000000' },
      end: { x: '0x3ff0000000000000', y: '0x0000000000000000', z: '0x4034000000000000' },
      sourceId: '7',
    }];
    const comparison = {
      results: [
        { label: 'baseline', options: { node_input: true }, status: 'success' as const, report },
        {
          label: 'comparison',
          options: { node_input: true },
          status: 'error' as const,
          error: {
            schema_version: 1,
            family: 'topology',
            code: 'interior_intersection',
            stage: 'noding_validation',
          },
        },
      ],
      diverged: true,
    };
    const bundle = createDebuggerFixtureBundle({
      caseId: 'profile-crossing-001',
      classification: 'expected_divergence',
      segments,
      profileComparison: comparison,
      witness: { kind: 'outcome_kinds', baseline: 'success', comparison: 'error' },
    });

    const encoded = serializeDebuggerFixture(bundle);
    expect(JSON.parse(encoded)).toMatchObject({
      schema_version: 1,
      kind: 'geo_polygonize_compatibility_fixture',
      case_id: 'profile-crossing-001',
      classification: 'expected_divergence',
      input: [{ sourceId: '7', start: { z: '0x4024000000000000' } }],
      baseline: { status: 'success' },
      comparison: { status: 'error' },
    });
    expect(encoded).not.toContain('trace_run');
    expect(encoded).not.toContain('normalized_input_segment');
    expect(serializeDebuggerFixture(JSON.parse(encoded))).toBe(encoded);
  });

  it('rejects unsafe fixture IDs and non-differences', () => {
    expect(() => createDebuggerFixtureBundle({
      caseId: '../fixture',
      classification: 'invalid_ambiguous',
      segments: [{
        start: { x: '0x0', y: '0x0', z: '0x0' },
        end: { x: '0x0', y: '0x0', z: '0x0' },
        sourceId: '1',
      }],
      profileComparison: { results: [], diverged: false },
      witness: { kind: 'outcome_kinds', baseline: 'success', comparison: 'error' },
    })).toThrow('lowercase and filesystem-safe');
  });
});
