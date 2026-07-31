import type { NormalizedPolygonizeErrorV1 } from 'geo-polygonize';

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function extractNormalizedError(error: unknown): NormalizedPolygonizeErrorV1 | null {
  if (!isObject(error) || !isObject(error.normalized)) return null;
  const normalized = error.normalized;
  return normalized.schema_version === 1
    && typeof normalized.family === 'string'
    && typeof normalized.code === 'string'
    && typeof normalized.stage === 'string'
    ? normalized as unknown as NormalizedPolygonizeErrorV1
    : null;
}
