import init, { polygonizeWithOptionsBuffer, type PolygonizerOptions } from 'geo-polygonize';
import {
  decodeExactFloat,
  fingerprintDifference,
  minimizeExactSegments,
  type ExactInputSegment,
  type FingerprintDifference,
} from './minimize';

type Request = {
  segments: ExactInputSegment[];
  baselineOptions: Partial<PolygonizerOptions>;
  comparisonOptions: Partial<PolygonizerOptions>;
};

function pack(segments: ExactInputSegment[]) {
  if (segments.length > Math.floor(0xffff_ffff / 2)) {
    throw new Error('Too many trace segments for u32 offsets');
  }
  const coordinates = new Float64Array(segments.length * 6);
  const offsets = new Uint32Array(segments.length + 1);
  const sourceIds = new Uint32Array(segments.length);
  for (let index = 0; index < segments.length; index += 1) {
    const segment = segments[index];
    coordinates.set([
      decodeExactFloat(segment.start.x),
      decodeExactFloat(segment.start.y),
      decodeExactFloat(segment.start.z),
      decodeExactFloat(segment.end.x),
      decodeExactFloat(segment.end.y),
      decodeExactFloat(segment.end.z),
    ], index * 6);
    offsets[index] = index * 2;
    const sourceId = BigInt(segment.sourceId);
    if (sourceId < 0n || sourceId > 0xffff_ffffn) throw new Error('Invalid trace source ID');
    sourceIds[index] = Number(sourceId);
  }
  offsets[segments.length] = segments.length * 2;
  return { coordinates, offsets, sourceIds };
}

function topologyWithoutOptions(value: unknown) {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new Error('Buffer result did not contain a topology fingerprint');
  }
  const { options: _, ...topology } = value as Record<string, unknown>;
  return topology;
}

function run(segments: ExactInputSegment[], options: Partial<PolygonizerOptions>) {
  const { coordinates, offsets, sourceIds } = pack(segments);
  const result = polygonizeWithOptionsBuffer(coordinates, offsets, 3, options, sourceIds);
  try {
    return topologyWithoutOptions(result.topology_fingerprint);
  } finally {
    result.free();
  }
}

self.addEventListener('message', async ({ data }: MessageEvent<Request>) => {
  try {
    await init();
    const signature = fingerprintDifference(
      run(data.segments, data.baselineOptions),
      run(data.segments, data.comparisonOptions),
    );
    if (!signature) throw new Error('Profile difference no longer reproduces');
    const reproduces = async (candidate: ExactInputSegment[]) => {
      try {
        const candidateSignature = fingerprintDifference(
          run(candidate, data.baselineOptions),
          run(candidate, data.comparisonOptions),
        );
        return JSON.stringify(candidateSignature) === JSON.stringify(signature);
      } catch {
        return false;
      }
    };
    const minimized = await minimizeExactSegments(data.segments, reproduces, (reduction) => {
      self.postMessage({ type: 'reduction', reduction });
    });
    if (!minimized) throw new Error('Profile difference was not deterministic');
    self.postMessage({ type: 'result', result: { signature, segments: minimized } });
  } catch (error) {
    self.postMessage({
      type: 'error',
      message: error instanceof Error ? error.message : String(error),
    });
  }
});
