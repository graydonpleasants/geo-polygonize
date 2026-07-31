import type {
  NormalizedPolygonizeErrorV1,
  PolygonizerOptions,
  TopologyFingerprintV1,
  TopologyTraceV1,
} from 'geo-polygonize';
import type { ProfileComparison } from './compare';

export const DEBUGGER_EVIDENCE_FILENAME = 'geo-polygonize-debugger-evidence-v1.json';

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

export function downloadDebuggerEvidence(bundle: DebuggerEvidenceBundleV1) {
  const url = URL.createObjectURL(new Blob(
    [serializeDebuggerEvidence(bundle)],
    { type: 'application/json' },
  ));
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = DEBUGGER_EVIDENCE_FILENAME;
  anchor.click();
  URL.revokeObjectURL(url);
}
