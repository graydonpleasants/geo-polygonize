import type { PolygonizerOptions } from 'geo-polygonize';
import type {
  ExactInputSegment,
  MinimizationReduction,
  MinimizationResult,
} from './minimize';

type WorkerRequest = {
  segments: ExactInputSegment[];
  baselineOptions: Partial<PolygonizerOptions>;
  comparisonOptions: Partial<PolygonizerOptions>;
};
type WorkerReply =
  | { type: 'reduction'; reduction: MinimizationReduction }
  | { type: 'result'; result: MinimizationResult }
  | { type: 'error'; message: string };

export function minimizeProfileDifference(
  request: WorkerRequest,
  { signal, onReduction }: {
    signal?: AbortSignal;
    onReduction?: (reduction: MinimizationReduction) => void;
  } = {},
): Promise<MinimizationResult> {
  if (signal?.aborted) return Promise.reject(new DOMException('Minimization cancelled', 'AbortError'));
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL('./minimize_worker.ts', import.meta.url), { type: 'module' });
    const cleanup = () => {
      signal?.removeEventListener('abort', abort);
      worker.terminate();
    };
    const abort = () => {
      cleanup();
      reject(new DOMException('Minimization cancelled', 'AbortError'));
    };
    worker.onmessage = ({ data }: MessageEvent<WorkerReply>) => {
      if (data.type === 'reduction') {
        onReduction?.(data.reduction);
      } else {
        cleanup();
        if (data.type === 'result') resolve(data.result);
        else reject(new Error(data.message));
      }
    };
    worker.onerror = ({ message }) => {
      cleanup();
      reject(new Error(message));
    };
    signal?.addEventListener('abort', abort, { once: true });
    worker.postMessage(request);
  });
}
