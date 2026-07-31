import { describe, expect, it } from 'vitest';
import { buildPlaygroundOptions } from './options';

describe('buildPlaygroundOptions', () => {
  it('selects independent validation for floating noding', () => {
    expect(buildPlaygroundOptions(true, 0, 'Validate')).toMatchObject({
      node_input: true,
      precision_model: { type: 'floating' },
      noding: { backend: 'Snap', guarantee: 'Validate' },
    });
  });

  it('enforces the certified fixed-precision prerequisites', () => {
    expect(buildPlaygroundOptions(false, 0.1, 'CertifiedFixedPrecision')).toMatchObject({
      node_input: true,
      precision_model: { type: 'fixed_grid', grid_size: 0.1 },
      snap_strategy: 'Grid',
    });
  });
});
