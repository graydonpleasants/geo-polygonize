import type { TopologyFingerprintV1 } from "./bindings/TopologyFingerprintV1";

export type TopologyTraceLevelV1 = "summary" | "noding" | "graph" | "rings" | "full";
export type TopologyTraceStageV1 = "summary" | "noding" | "graph" | "rings" | "output";

export type TopologyTraceEventV1 = {
    sequence: number;
    stage: TopologyTraceStageV1;
    kind: string;
    payload: unknown;
};

export type TopologyTraceV1 = {
    schema_version: 1;
    library_version: string;
    level: TopologyTraceLevelV1;
    byte_limit: number;
    bytes_used: number;
    truncated: boolean;
    options: unknown;
    events: TopologyTraceEventV1[];
};

export type PolygonizeTraceReportV1 = {
    schema_version: 1;
    topology: TopologyFingerprintV1;
    trace: TopologyTraceV1;
};
