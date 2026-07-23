import init, {
    polygonizeReportWithOptions,
    polygonizeWithOptions,
} from "../pkg-scalar/geo_polygonize.js";
import wasmUrl from "../pkg-scalar/geo_polygonize_bg.wasm";

type Operation = "polygons" | "report";

type Request = {
    id: number;
    operation: Operation;
    geojson: string;
    options: object;
};

let initialized: Promise<void> | undefined;

function initialize() {
    initialized ??= init(wasmUrl).then(() => {});
    return initialized;
}

self.addEventListener("message", async ({ data }: MessageEvent<Request>) => {
    try {
        await initialize();
        const result = data.operation === "polygons"
            ? polygonizeWithOptions(data.geojson, data.options)
            : polygonizeReportWithOptions(data.geojson, data.options);
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
