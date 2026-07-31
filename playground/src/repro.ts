import type { NodingGuarantee } from 'geo-polygonize';

export const MAX_REPRO_QUERY_LENGTH = 8192;

export type PlaygroundReproV1 = {
  schema_version: 1;
  input: Record<string, unknown>;
  node_input: boolean;
  snap_grid_size: number;
  noding_guarantee: NodingGuarantee;
};

function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (typeof value !== 'object' || value === null) return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left < right ? -1 : left > right ? 1 : 0)
      .map(([key, child]) => [key, canonicalize(child)]),
  );
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function encodePlaygroundRepro(repro: PlaygroundReproV1): string {
  const bytes = new TextEncoder().encode(JSON.stringify(canonicalize(repro)));
  let binary = '';
  for (const byte of bytes) binary += String.fromCharCode(byte);
  const encoded = btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '');
  if (encoded.length > MAX_REPRO_QUERY_LENGTH) {
    throw new Error('Repro is too large for a shareable URL');
  }
  return encoded;
}

export function decodePlaygroundRepro(encoded: string): PlaygroundReproV1 {
  if (encoded.length > MAX_REPRO_QUERY_LENGTH) {
    throw new Error('Shared repro exceeds the URL limit');
  }
  const base64 = encoded.replaceAll('-', '+').replaceAll('_', '/');
  const padded = base64.padEnd(Math.ceil(base64.length / 4) * 4, '=');
  const binary = atob(padded);
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  const value: unknown = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
  if (!isObject(value) || value.schema_version !== 1 || !isObject(value.input)
    || typeof value.node_input !== 'boolean'
    || typeof value.snap_grid_size !== 'number' || !Number.isFinite(value.snap_grid_size)
    || !['Unchecked', 'Validate', 'CertifiedFixedPrecision'].includes(
      value.noding_guarantee as string,
    )) {
    throw new Error('Unsupported shared repro');
  }
  return value as PlaygroundReproV1;
}
