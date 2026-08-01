import type {
  NormalizedPolygonizeErrorV1,
  PolygonizerOptions,
  TopologyFingerprintV1,
  TopologyTraceV1,
} from 'geo-polygonize';
import type { ProfileComparison } from './compare';
import type { ExactInputSegment, ProfileDifferenceSignature } from './minimize';

export const DEBUGGER_EVIDENCE_FILENAME = 'geo-polygonize-debugger-evidence-v1.json';

export type CompatibilityClassification =
  | 'expected_parity'
  | 'expected_divergence'
  | 'invalid_ambiguous';

type FixtureProfileOutcome = {
  label: string;
  options: Partial<PolygonizerOptions>;
} & (
  | { status: 'success'; topology: TopologyFingerprintV1 }
  | { status: 'error'; error: NormalizedPolygonizeErrorV1 }
);

export type DebuggerFixtureBundleV1 = {
  schema_version: 1;
  kind: 'geo_polygonize_compatibility_fixture';
  case_id: string;
  classification: CompatibilityClassification;
  input: ExactInputSegment[];
  baseline: FixtureProfileOutcome;
  comparison: FixtureProfileOutcome;
  witness: ProfileDifferenceSignature;
};

export type DebuggerEvidenceBundleV1 = {
  schema_version: 1;
  kind: 'geo_polygonize_debugger_evidence';
  input: unknown;
  requested_options: Partial<PolygonizerOptions>;
  trace_run: {
    topology: TopologyFingerprintV1;
    trace: TopologyTraceV1;
  } | null;
  profile_comparison: ProfileComparison | null;
  normalized_error: NormalizedPolygonizeErrorV1 | null;
};

function canonicalize(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalize);
  if (typeof value !== 'object' || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0))
      .map(([key, child]) => [key, canonicalize(child)]),
  );
}

export function createDebuggerEvidenceBundle({
  input,
  requestedOptions,
  topology,
  trace,
  comparison,
  normalizedError,
}: {
  input: unknown;
  requestedOptions: Partial<PolygonizerOptions>;
  topology: TopologyFingerprintV1 | null;
  trace: TopologyTraceV1 | null;
  comparison: ProfileComparison | null;
  normalizedError: NormalizedPolygonizeErrorV1 | null;
}): DebuggerEvidenceBundleV1 {
  if ((topology === null) !== (trace === null)) {
    throw new Error('Debugger evidence requires topology and trace together');
  }
  return {
    schema_version: 1,
    kind: 'geo_polygonize_debugger_evidence',
    input,
    requested_options: requestedOptions,
    trace_run: topology && trace ? { topology, trace } : null,
    profile_comparison: comparison,
    normalized_error: normalizedError,
  };
}

export function serializeDebuggerEvidence(bundle: DebuggerEvidenceBundleV1): string {
  return `${JSON.stringify(canonicalize(bundle), null, 2)}\n`;
}

export function createDebuggerFixtureBundle({
  caseId,
  classification,
  segments,
  profileComparison,
  witness,
}: {
  caseId: string;
  classification: CompatibilityClassification;
  segments: ExactInputSegment[];
  profileComparison: ProfileComparison;
  witness: ProfileDifferenceSignature;
}): DebuggerFixtureBundleV1 {
  if (!/^[a-z0-9]+(?:[._-][a-z0-9]+)*$/.test(caseId)) {
    throw new Error('Fixture case ID must be lowercase and filesystem-safe');
  }
  if (!profileComparison.diverged || profileComparison.results.length !== 2 || segments.length === 0) {
    throw new Error('Fixture export requires a minimized two-profile difference');
  }
  const outcome = (result: ProfileComparison['results'][number]): FixtureProfileOutcome => (
    result.status === 'success'
      ? { label: result.label, options: result.options, status: 'success', topology: result.report.topology }
      : { label: result.label, options: result.options, status: 'error', error: result.error }
  );
  return {
    schema_version: 1,
    kind: 'geo_polygonize_compatibility_fixture',
    case_id: caseId,
    classification,
    input: segments,
    baseline: outcome(profileComparison.results[0]),
    comparison: outcome(profileComparison.results[1]),
    witness,
  };
}

export function serializeDebuggerFixture(bundle: DebuggerFixtureBundleV1): string {
  return `${JSON.stringify(canonicalize(bundle), null, 2)}\n`;
}

function downloadJson(filename: string, value: string) {
  const url = URL.createObjectURL(new Blob([value], { type: 'application/json' }));
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}

export function downloadDebuggerEvidence(bundle: DebuggerEvidenceBundleV1) {
  downloadJson(DEBUGGER_EVIDENCE_FILENAME, serializeDebuggerEvidence(bundle));
}

export function downloadDebuggerFixture(bundle: DebuggerFixtureBundleV1) {
  downloadJson(`${bundle.case_id}.json`, serializeDebuggerFixture(bundle));
}
