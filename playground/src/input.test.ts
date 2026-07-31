import { describe, expect, it } from 'vitest';
import { appendLineString, parseGeojsonInput } from './input';

describe('parseGeojsonInput', () => {
  it('normalizes geometry and feature roots for the playground', () => {
    const geometry = parseGeojsonInput('{"type":"LineString","coordinates":[[0,0],[1,1]]}');
    const feature = parseGeojsonInput('{"type":"Feature","properties":null,"geometry":null}');

    expect(geometry.features).toHaveLength(1);
    expect(feature.features).toHaveLength(1);
  });

  it('rejects malformed feature collections', () => {
    expect(() => parseGeojsonInput('{"type":"FeatureCollection"}')).toThrow(
      'FeatureCollection.features must be an array',
    );
  });

  it('appends drawn linework without mutating the input', () => {
    const input = parseGeojsonInput('{"type":"FeatureCollection","features":[]}');
    const output = appendLineString(input, [[0, 0], [1, 1]]);

    expect(input.features).toHaveLength(0);
    expect(output.features).toHaveLength(1);
  });
});
