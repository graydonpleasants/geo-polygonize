import type {
  NormalizedPolygonizeErrorV1,
  PolygonizeTraceReportV1,
  PolygonizerOptions,
  polygonizeTraceWithOptionsAsync,
} from 'geo-polygonize';
import { extractNormalizedError } from './error';
import {
  profileDifferenceSignature,
  type ProfileOutcome,
} from './minimize';
import { buildPlaygroundOptions } from './options';
import { parsePlaygroundTraceReport, PLAYGROUND_TRACE_BYTE_LIMIT } from './trace';

const profiles = [
  {
    label: 'Validated floating',
    options: buildPlaygroundOptions(true, 0, 'Validate'),
  },
  {
    label: 'Certified fixed precision',
    options: buildPlaygroundOptions(true, 0.001, 'CertifiedFixedPrecision'),
  },
] satisfies { label: string; options: Partial<PolygonizerOptions> }[];

type ProfileResultBase = {
  label: string;
  options: Partial<PolygonizerOptions>;
};
export type ProfileResult = ProfileResultBase & (
  | { status: 'success'; report: PolygonizeTraceReportV1 }
  | { status: 'error'; error: NormalizedPolygonizeErrorV1 }
);

export type ProfileComparison = {
  results: ProfileResult[];
  diverged: boolean;
};

function outcome(result: ProfileResult): ProfileOutcome {
  if (result.status === 'error') return { status: 'error', value: result.error };
  const { options: _, ...topology } = result.report.topology;
  return { status: 'success', value: topology };
}

export async function comparePlaygroundProfiles(
  input: string,
  signal: AbortSignal,
  run: typeof polygonizeTraceWithOptionsAsync,
): Promise<ProfileComparison> {
  const results = await Promise.all(profiles.map(async ({ label, options }): Promise<ProfileResult> => {
    try {
      const report = parsePlaygroundTraceReport(await run(
        input,
        options,
        'summary',
        PLAYGROUND_TRACE_BYTE_LIMIT,
        { signal },
      ));
      return {
        label,
        options: report.topology.options as Partial<PolygonizerOptions>,
        status: 'success',
        report,
      };
    } catch (error) {
      if (typeof error === 'object' && error !== null && 'name' in error
        && error.name === 'AbortError') throw error;
      const normalized = extractNormalizedError(error);
      if (!normalized) throw new Error('Profile comparison failed without a normalized V1 error');
      return { label, options, status: 'error', error: normalized };
    }
  }));
  const signature = profileDifferenceSignature(outcome(results[0]), outcome(results[1]));
  return {
    results,
    diverged: signature !== null,
  };
}
