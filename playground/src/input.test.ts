import { describe, expect, it } from 'vitest';
import { parseGeojsonInput } from './input';

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
});
