import { describe, expect, it } from 'vitest';
import {
  decodePlaygroundRepro,
  encodePlaygroundRepro,
  MAX_REPRO_QUERY_LENGTH,
  type PlaygroundReproV1,
} from './repro';

const repro = (input: Record<string, unknown>): PlaygroundReproV1 => ({
  schema_version: 1,
  input,
  node_input: true,
  snap_grid_size: 0.25,
  noding_guarantee: 'Validate',
});

describe('shareable playground repros', () => {
  it('round trips UTF-8 input and canonical options', () => {
    const value = repro({ type: 'FeatureCollection', label: 'café', features: [] });
    expect(decodePlaygroundRepro(encodePlaygroundRepro(value))).toEqual(value);
  });

  it('is deterministic across object key order', () => {
    expect(encodePlaygroundRepro(repro({ b: 2, a: 1 })))
      .toBe(encodePlaygroundRepro(repro({ a: 1, b: 2 })));
  });

  it('rejects oversized query payloads', () => {
    expect(() => decodePlaygroundRepro('a'.repeat(MAX_REPRO_QUERY_LENGTH + 1)))
      .toThrow('Shared repro exceeds the URL limit');
  });
});
