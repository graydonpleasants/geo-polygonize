type GeoJsonObject = Record<string, unknown>;

function isObject(value: unknown): value is GeoJsonObject {
  return typeof value === 'object' && value !== null;
}

export function parseGeojsonInput(text: string): GeoJsonObject {
  const value: unknown = JSON.parse(text);
  if (!isObject(value)) throw new Error('GeoJSON must be an object');

  if (value.type === 'FeatureCollection') {
    if (!Array.isArray(value.features)) {
      throw new Error('FeatureCollection.features must be an array');
    }
    return value;
  }

  if (value.type === 'Feature') {
    return { type: 'FeatureCollection', features: [value] };
  }

  if (typeof value.type === 'string') {
    return {
      type: 'FeatureCollection',
      features: [{ type: 'Feature', properties: null, geometry: value }],
    };
  }

  throw new Error('GeoJSON object is missing a type');
}
