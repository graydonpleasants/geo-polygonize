import type { NodingGuarantee, PolygonizerOptions } from 'geo-polygonize';

export function buildPlaygroundOptions(
  nodeInput: boolean,
  snapGridSize: number,
  guarantee: NodingGuarantee,
): Partial<PolygonizerOptions> {
  const certified = guarantee === 'CertifiedFixedPrecision';
  return {
    node_input: certified || nodeInput,
    precision_model: certified || (nodeInput && snapGridSize > 0)
      ? { type: 'fixed_grid', grid_size: snapGridSize }
      : { type: 'floating' },
    snap_strategy: 'Grid',
    noding: { backend: 'Snap', guarantee },
  };
}
