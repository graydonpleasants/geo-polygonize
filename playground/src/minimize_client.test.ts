import { afterEach, describe, expect, it, vi } from 'vitest';
import { minimizeProfileDifference } from './minimize_client';

const request = {
  segments: [],
  baselineOptions: {},
  comparisonOptions: {},
};

describe('minimization worker client', () => {
  afterEach(() => vi.unstubAllGlobals());

  it('terminates its dedicated worker when aborted', async () => {
    const terminate = vi.fn();
    const constructed = vi.fn();
    const postMessage = vi.fn();
    class WorkerMock {
      onmessage = null;
      onerror = null;
      constructor() { constructed(); }
      postMessage = postMessage;
      terminate = terminate;
    }
    vi.stubGlobal('Worker', WorkerMock);
    const controller = new AbortController();

    const result = minimizeProfileDifference(request, { signal: controller.signal });
    controller.abort();

    await expect(result).rejects.toMatchObject({ name: 'AbortError' });
    expect(constructed).toHaveBeenCalledTimes(1);
    expect(postMessage).toHaveBeenCalledWith(request);
    expect(terminate).toHaveBeenCalledTimes(1);
  });
});
