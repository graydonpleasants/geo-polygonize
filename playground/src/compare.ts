import type {
  PolygonizeTraceReportV1,
  PolygonizerOptions,
  polygonizeTraceWithOptionsAsync,
} from 'geo-polygonize';
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

export type ProfileResult = {
  label: string;
  report: PolygonizeTraceReportV1;
};

export type ProfileComparison = {
  results: ProfileResult[];
  diverged: boolean;
};

function fingerprint({ options: _, ...topology }: PolygonizeTraceReportV1['topology']) {
  return JSON.stringify(topology);
}

export async function comparePlaygroundProfiles(
  input: string,
  signal: AbortSignal,
  run: typeof polygonizeTraceWithOptionsAsync,
): Promise<ProfileComparison> {
  const results = await Promise.all(profiles.map(async ({ label, options }) => ({
    label,
    report: parsePlaygroundTraceReport(await run(
      input,
      options,
      'summary',
      PLAYGROUND_TRACE_BYTE_LIMIT,
      { signal },
    )),
  })));
  return {
    results,
    diverged: fingerprint(results[0].report.topology) !== fingerprint(results[1].report.topology),
  };
}
