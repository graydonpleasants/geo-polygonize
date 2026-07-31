import wasmInit, * as exports from "../pkg-scalar/geo_polygonize.js";
import wasmScalarUrl from "../pkg-scalar/geo_polygonize_bg.wasm";
import wasmSimdUrl from "../pkg-simd/geo_polygonize_bg.wasm";
import type { PolygonizerOptions } from "./bindings/PolygonizerOptions";
import { selectRuntime } from "./runtime";

// Cache the initialization promise
let initPromise: Promise<typeof exports> | undefined;

// We re-export everything from the scalar package.
// The JS bindings in pkg-scalar/geo_polygonize.js are identical to pkg-simd/geo_polygonize.js
// because the exported API is the same.
// By calling init() with the correct Wasm binary, these exported functions will use that binary.
export * from "../pkg-scalar/geo_polygonize.js";

// Export auto-generated ts-rs bindings
export * from "./bindings/PolygonizerOptions";
export * from "./bindings/ContainmentOptions";
export * from "./bindings/DeterminismOptions";
export * from "./bindings/DiagnosticsOptions";
export * from "./bindings/NodingBackend";
export * from "./bindings/NodingGuarantee";
export * from "./bindings/NodingOptions";
export * from "./bindings/OutputFilterOptions";
export * from "./bindings/PrecisionModel";
export * from "./bindings/ProvenanceOptions";
export * from "./bindings/SnapStrategy";
export * from "./bindings/TileOwnershipPolicy";
export * from "./bindings/TouchPolicy";
export * from "./bindings/ZOptions";
export * from "./bindings/ZPolicy";
export * from "./bindings/TopologyFingerprintV1";
export * from "./bindings/NormalizedPolygonizeErrorV1";
export * from "./cfb";

export type WasmWorkerOptions = {
    signal?: AbortSignal;
};

export type TopologyTraceLevel = "summary" | "noding" | "graph" | "rings" | "full";

type WorkerOperation = "polygons" | "report" | "trace";

type WorkerReply = {
    id: number;
    result?: string;
    error?: { name: string; message: string; normalized?: unknown };
};

function abortError() {
    const error = new Error("Polygonization cancelled");
    error.name = "AbortError";
    return error;
}

function polygonizeInWorker(
    operation: WorkerOperation,
    geojson: string,
    options: Partial<PolygonizerOptions>,
    { signal }: WasmWorkerOptions = {},
    trace?: { level: TopologyTraceLevel; byteLimit: number },
): Promise<string> {
    if (signal?.aborted) return Promise.reject(abortError());
    if (typeof Worker === "undefined") {
        return Promise.reject(new Error("Worker-based polygonization requires a browser Worker"));
    }

    return new Promise((resolve, reject) => {
        const worker = new Worker(new URL("./polygonize_worker.js", import.meta.url), {
            type: "module",
        });
        const cleanup = () => {
            signal?.removeEventListener("abort", abort);
            worker.terminate();
        };
        const abort = () => {
            cleanup();
            reject(abortError());
        };
        worker.onmessage = ({ data }: MessageEvent<WorkerReply>) => {
            cleanup();
            if (data.error) {
                const error = Object.assign(new Error(data.error.message), data.error);
                reject(error);
            } else {
                resolve(data.result!);
            }
        };
        worker.onerror = ({ message }) => {
            cleanup();
            reject(new Error(message));
        };
        signal?.addEventListener("abort", abort, { once: true });
        worker.postMessage({
            id: 0,
            operation,
            geojson,
            options,
            traceLevel: trace?.level,
            byteLimit: trace?.byteLimit,
        });
    });
}

/**
 * Polygonizes GeoJSON in an isolated browser worker.
 *
 * Aborting terminates that worker; direct Wasm exports remain synchronous and
 * cannot be interrupted with an `AbortSignal`.
 */
export function polygonizeWithOptionsAsync(
    geojson: string,
    options: Partial<PolygonizerOptions>,
    workerOptions?: WasmWorkerOptions,
): Promise<string> {
    return polygonizeInWorker("polygons", geojson, options, workerOptions);
}

/** Returns the canonical topology report in an abortable browser worker. */
export function polygonizeReportWithOptionsAsync(
    geojson: string,
    options: Partial<PolygonizerOptions>,
    workerOptions?: WasmWorkerOptions,
): Promise<string> {
    return polygonizeInWorker("report", geojson, options, workerOptions);
}

/** Returns a bounded topology trace in an abortable browser worker. */
export function polygonizeTraceWithOptionsAsync(
    geojson: string,
    options: Partial<PolygonizerOptions>,
    traceLevel: TopologyTraceLevel,
    byteLimit: number,
    workerOptions?: WasmWorkerOptions,
): Promise<string> {
    return polygonizeInWorker(
        "trace",
        geojson,
        options,
        workerOptions,
        { level: traceLevel, byteLimit },
    );
}

// Override the init function
// input is ignored because we are using inlined Wasm
export default function init(_input?: any): Promise<typeof exports> {
    if (initPromise) return initPromise;

    const runtime = selectRuntime(wasmScalarUrl, wasmSimdUrl);

    // Create the promise and cache it
    initPromise = (async () => {
        await wasmInit(runtime.module);
        return exports;
    })();

    return initPromise;
}
