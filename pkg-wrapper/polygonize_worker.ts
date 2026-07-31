import init, {
    polygonizeReportWithOptions,
    polygonizeTraceWithOptions,
    polygonizeWithOptions,
} from "../pkg-scalar/geo_polygonize.js";
import wasmScalarUrl from "../pkg-scalar/geo_polygonize_bg.wasm";
import wasmSimdUrl from "../pkg-simd/geo_polygonize_bg.wasm";
import { selectRuntime } from "./runtime";

type Operation = "initialize" | "polygons" | "report" | "trace";

type Request = {
    id: number;
    operation: Operation;
    geojson: string;
    options: object;
    traceLevel?: string;
    byteLimit?: number;
};

let initialized: Promise<void> | undefined;
const runtime = selectRuntime(wasmScalarUrl, wasmSimdUrl);

function initialize() {
    initialized ??= init(runtime.module).then(() => {});
    return initialized;
}

self.addEventListener("message", async ({ data }: MessageEvent<Request>) => {
    try {
        await initialize();
        let result: string;
        switch (data.operation) {
            case "initialize":
                result = runtime.variant;
                break;
            case "polygons":
                result = polygonizeWithOptions(data.geojson, data.options);
                break;
            case "report":
                result = polygonizeReportWithOptions(data.geojson, data.options);
                break;
            case "trace":
                result = polygonizeTraceWithOptions(
                    data.geojson,
                    data.options,
                    data.traceLevel!,
                    data.byteLimit!,
                );
                break;
        }
        self.postMessage({ id: data.id, result });
    } catch (error) {
        const exception = error instanceof Error ? error : new Error(String(error));
        self.postMessage({
            id: data.id,
            error: {
                name: exception.name,
                message: exception.message,
                normalized: (exception as Error & { normalized?: unknown }).normalized,
            },
        });
    }
});
