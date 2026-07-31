import { describe, expect, it } from 'vitest';
import { extractNormalizedError } from './error';

describe('normalized worker errors', () => {
  it('extracts the preserved V1 error and witness without accepting unknown shapes', () => {
    const normalized = {
      schema_version: 1,
      family: 'noding',
      code: 'intersection',
      stage: 'noding',
      field: null,
      expected: null,
      actual: null,
      limit: null,
      observed: null,
      witness: { ids: ['0x01'], coordinate: null },
    };

    expect(extractNormalizedError(Object.assign(new Error('failed'), { normalized })))
      .toEqual(normalized);
    expect(extractNormalizedError({ normalized: { schema_version: 2 } })).toBeNull();
  });
});
