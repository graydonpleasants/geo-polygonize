import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ExactInputSegment } from './minimize';

const wasm = vi.hoisted(() => ({
  init: vi.fn(async () => {}),
  polygonizeWithOptionsBuffer: vi.fn(),
}));

vi.mock('geo-polygonize', () => ({
  default: wasm.init,
  polygonizeWithOptionsBuffer: wasm.polygonizeWithOptionsBuffer,
}));

const view = new DataView(new ArrayBuffer(8));
const bits = (value: number) => {
  view.setFloat64(0, value);
  return `0x${view.getBigUint64(0).toString(16).padStart(16, '0')}`;
};
const segments: ExactInputSegment[] = [{
  start: { x: bits(1), y: bits(2), z: bits(30) },
  end: { x: bits(3), y: bits(4), z: bits(40) },
  sourceId: '0x0000002a',
}];
const normalized = {
  schema_version: 1,
  family: 'topology',
  code: 'z_conflict',
  stage: 'z_reconciliation',
  field: null,
  expected: null,
  actual: null,
  limit: null,
  observed: null,
  witness: { ids: ['0x0000002a'], coordinate: null },
};

type WorkerHandler = (event: { data: unknown }) => Promise<void>;

async function loadWorker() {
  let handler: WorkerHandler | undefined;
  const postMessage = vi.fn();
  vi.stubGlobal('self', {
    addEventListener: vi.fn((_type: string, listener: WorkerHandler) => { handler = listener; }),
    postMessage,
  });
  await import('./minimize_worker');
  if (!handler) throw new Error('worker handler was not registered');
  return { handler, postMessage };
}

describe('profile minimization worker outcomes', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  it('minimizes success-vs-error using only the normalized structural signature', async () => {
    const free = vi.fn();
    wasm.polygonizeWithOptionsBuffer.mockImplementation(
      (_coordinates, _offsets, _stride, options) => {
        if (options.input_profile_id === 'comparison') {
          throw { message: 'wording may change', normalized };
        }
        return {
          topology_fingerprint: { options, polygons: [] },
          free,
        };
      },
    );
    const { handler, postMessage } = await loadWorker();

    await handler({
      data: {
        segments,
        baselineOptions: { input_profile_id: 'baseline' },
        comparisonOptions: { input_profile_id: 'comparison' },
      },
    });

    expect(wasm.init).toHaveBeenCalledTimes(1);
    expect(wasm.polygonizeWithOptionsBuffer.mock.calls[0][0])
      .toEqual(new Float64Array([1, 2, 30, 3, 4, 40]));
    expect(wasm.polygonizeWithOptionsBuffer.mock.calls[0][1])
      .toEqual(new Uint32Array([0, 2]));
    expect(wasm.polygonizeWithOptionsBuffer.mock.calls[0][2]).toBe(3);
    expect(wasm.polygonizeWithOptionsBuffer.mock.calls[0][4])
      .toEqual(new Uint32Array([42]));
    expect(wasm.polygonizeWithOptionsBuffer.mock.calls.map((call) => call[3].input_profile_id))
      .toEqual(['baseline', 'comparison', 'baseline', 'comparison', 'baseline', 'comparison']);
    expect(free).toHaveBeenCalledTimes(3);
    expect(postMessage).toHaveBeenLastCalledWith({
      type: 'result',
      result: {
        signature: {
          kind: 'outcome_kinds',
          baseline: 'success',
          comparison: 'error',
        },
        segments: [],
      },
    });
  });

  it('stops when the buffer API does not provide a normalized error', async () => {
    wasm.polygonizeWithOptionsBuffer.mockImplementation(
      (_coordinates, _offsets, _stride, options) => {
        if (options.input_profile_id === 'comparison') throw new Error('do not parse this wording');
        return { topology_fingerprint: { options, polygons: [] }, free: vi.fn() };
      },
    );
    const { handler, postMessage } = await loadWorker();

    await handler({
      data: {
        segments,
        baselineOptions: { input_profile_id: 'baseline' },
        comparisonOptions: { input_profile_id: 'comparison' },
      },
    });

    expect(postMessage).toHaveBeenLastCalledWith({
      type: 'error',
      message: 'Buffer API failure did not include a normalized V1 error',
    });
  });

  it('preserves both complete normalized errors when their structures differ', async () => {
    const comparisonError = {
      ...normalized,
      code: 'interior_intersection',
      witness: { ids: ['0x0000002a', '0x0000002b'], coordinate: null },
    };
    wasm.polygonizeWithOptionsBuffer.mockImplementation(
      (_coordinates, _offsets, _stride, options) => {
        throw {
          message: options.input_profile_id === 'baseline' ? 'first wording' : 'second wording',
          normalized: options.input_profile_id === 'baseline' ? normalized : comparisonError,
        };
      },
    );
    const { handler, postMessage } = await loadWorker();

    await handler({
      data: {
        segments,
        baselineOptions: { input_profile_id: 'baseline' },
        comparisonOptions: { input_profile_id: 'comparison' },
      },
    });

    expect(postMessage).toHaveBeenLastCalledWith({
      type: 'result',
      result: {
        signature: {
          kind: 'normalized_errors',
          baseline: normalized,
          comparison: comparisonError,
        },
        segments: [],
      },
    });
  });
});
