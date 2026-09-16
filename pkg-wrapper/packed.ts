import type { PolygonizerOptions } from "./bindings/PolygonizerOptions";

export type PackedProjection = "full" | "geometry";

export type PackedBoundaryMetrics = {
    schema_version: 1;
    report_materialize_ms: number;
    output_flatten_ms: number;
    packed_output_bytes: number;
};

export type BorrowedPackedBuffers = Readonly<{
    coordinates: Float64Array;
    ringOffsets: Uint32Array;
    polygonOffsets: Uint32Array;
    representativeLineIds: Uint32Array;
    stride: 2 | 3;
    projection: PackedProjection;
}>;

export type PackedPolygonSnapshot = BorrowedPackedBuffers & Readonly<{
    layout: "geo-polygonize-packed-v1";
}>;

type RawPolygonResult = {
    free(): void;
    coords_ptr(): number;
    coords_len(): number;
    ring_offsets_ptr(): number;
    ring_offsets_len(): number;
    polygon_offsets_ptr(): number;
    polygon_offsets_len(): number;
    flat_line_ids_ptr(): number;
    flat_line_ids_len(): number;
    stride(): number;
    readonly projection: string;
    readonly provenance: unknown;
    readonly dangles: unknown;
    readonly cut_edges: unknown;
    readonly invalid_rings: unknown;
    readonly diagnostics: unknown;
    readonly topology_fingerprint: unknown;
    readonly boundary_metrics: PackedBoundaryMetrics;
};

type PackedWasmExports = {
    polygonizeWithOptionsBuffer(
        coordinates: Float64Array,
        offsets: Uint32Array,
        stride: number,
        options: Partial<PolygonizerOptions>,
        lineIds?: Uint32Array | null,
    ): RawPolygonResult;
    polygonizeGeometryWithOptionsBuffer(
        coordinates: Float64Array,
        offsets: Uint32Array,
        stride: number,
        options: Partial<PolygonizerOptions>,
        lineIds?: Uint32Array | null,
    ): RawPolygonResult;
};

type PackedDescriptors = Readonly<{
    coordinatesPointer: number;
    coordinatesLength: number;
    ringOffsetsPointer: number;
    ringOffsetsLength: number;
    polygonOffsetsPointer: number;
    polygonOffsetsLength: number;
    representativeLineIdsPointer: number;
    representativeLineIdsLength: number;
}>;

export type PackedFullReport = Readonly<{
    topologyFingerprint: unknown;
    provenance: unknown;
    dangles: unknown;
    cutEdges: unknown;
    invalidRings: unknown;
    diagnostics: unknown;
}>;

export class PackedPolygonResult {
    readonly stride: 2 | 3;
    readonly projection: PackedProjection;
    readonly boundaryMetrics: PackedBoundaryMetrics;

    readonly #memory: WebAssembly.Memory;
    readonly #descriptors: PackedDescriptors;
    readonly #fullReport?: PackedFullReport;
    #raw?: RawPolygonResult;
    #borrowing = false;

    constructor(raw: RawPolygonResult, memory: WebAssembly.Memory) {
        const stride = raw.stride();
        if (stride !== 2 && stride !== 3) {
            throw new Error(`Unexpected packed coordinate stride: ${stride}`);
        }
        if (raw.projection !== "full" && raw.projection !== "geometry") {
            throw new Error(`Unexpected packed result projection: ${raw.projection}`);
        }

        this.stride = stride;
        this.projection = raw.projection;
        this.boundaryMetrics = raw.boundary_metrics;
        this.#memory = memory;
        this.#raw = raw;
        this.#descriptors = {
            coordinatesPointer: raw.coords_ptr(),
            coordinatesLength: raw.coords_len(),
            ringOffsetsPointer: raw.ring_offsets_ptr(),
            ringOffsetsLength: raw.ring_offsets_len(),
            polygonOffsetsPointer: raw.polygon_offsets_ptr(),
            polygonOffsetsLength: raw.polygon_offsets_len(),
            representativeLineIdsPointer: raw.flat_line_ids_ptr(),
            representativeLineIdsLength: raw.flat_line_ids_len(),
        };
        if (this.projection === "full") {
            this.#fullReport = {
                topologyFingerprint: raw.topology_fingerprint,
                provenance: raw.provenance,
                dangles: raw.dangles,
                cutEdges: raw.cut_edges,
                invalidRings: raw.invalid_rings,
                diagnostics: raw.diagnostics,
            };
        }
    }

    get disposed(): boolean {
        return this.#raw === undefined;
    }

    get fullReport(): PackedFullReport | undefined {
        this.#assertUsable();
        return this.#fullReport;
    }

    withBorrowed<T>(consume: (buffers: BorrowedPackedBuffers) => T): T {
        this.#assertUsable();
        if (this.#borrowing) throw new Error("Packed result is already borrowed");
        this.#borrowing = true;
        try {
            const value = consume(this.#borrow());
            if (value !== null && typeof value === "object" && "then" in value) {
                throw new TypeError("Packed buffer borrows must be consumed synchronously");
            }
            return value;
        } finally {
            this.#borrowing = false;
        }
    }

    snapshot(): PackedPolygonSnapshot {
        return this.withBorrowed((buffers) => ({
            layout: "geo-polygonize-packed-v1",
            coordinates: new Float64Array(buffers.coordinates),
            ringOffsets: new Uint32Array(buffers.ringOffsets),
            polygonOffsets: new Uint32Array(buffers.polygonOffsets),
            representativeLineIds: new Uint32Array(buffers.representativeLineIds),
            stride: buffers.stride,
            projection: buffers.projection,
        }));
    }

    toGeoJSON(): GeoJsonFeatureCollection {
        return this.withBorrowed((buffers) => packedBuffersToGeoJSON(buffers));
    }

    free(): void {
        this.dispose();
    }

    dispose(): void {
        if (this.#borrowing) throw new Error("Cannot dispose a borrowed packed result");
        this.#raw?.free();
        this.#raw = undefined;
    }

    [Symbol.dispose](): void {
        this.dispose();
    }

    #assertUsable(): void {
        if (this.disposed) throw new Error("Packed result has been disposed");
        if (this.#borrowing) throw new Error("Cannot re-enter a borrowed packed result");
    }

    #borrow(): BorrowedPackedBuffers {
        const buffer = this.#memory.buffer;
        const descriptors = this.#descriptors;
        return {
            coordinates: new Float64Array(
                buffer,
                descriptors.coordinatesPointer,
                descriptors.coordinatesLength,
            ),
            ringOffsets: new Uint32Array(
                buffer,
                descriptors.ringOffsetsPointer,
                descriptors.ringOffsetsLength,
            ),
            polygonOffsets: new Uint32Array(
                buffer,
                descriptors.polygonOffsetsPointer,
                descriptors.polygonOffsetsLength,
            ),
            representativeLineIds: new Uint32Array(
                buffer,
                descriptors.representativeLineIdsPointer,
                descriptors.representativeLineIdsLength,
            ),
            stride: this.stride,
            projection: this.projection,
        };
    }
}

type GeoJsonPosition = Array<number>;
type GeoJsonPolygon = {
    type: "Polygon";
    coordinates: Array<Array<GeoJsonPosition>>;
};
type GeoJsonFeature = {
    type: "Feature";
    properties: null;
    geometry: GeoJsonPolygon;
};
export type GeoJsonFeatureCollection = {
    type: "FeatureCollection";
    features: Array<GeoJsonFeature>;
};

export function packedBuffersToGeoJSON(buffers: BorrowedPackedBuffers): GeoJsonFeatureCollection {
    const features: Array<GeoJsonFeature> = [];
    for (let polygonIndex = 0; polygonIndex < buffers.polygonOffsets.length; polygonIndex += 1) {
        const ringStart = buffers.polygonOffsets[polygonIndex];
        const ringEnd = buffers.polygonOffsets[polygonIndex + 1] ?? buffers.ringOffsets.length;
        const rings: Array<Array<GeoJsonPosition>> = [];
        for (let ringIndex = ringStart; ringIndex < ringEnd; ringIndex += 1) {
            const coordinateStart = buffers.ringOffsets[ringIndex];
            const coordinateEnd = buffers.ringOffsets[ringIndex + 1]
                ?? buffers.coordinates.length / buffers.stride;
            const ring: Array<GeoJsonPosition> = [];
            for (let coordinateIndex = coordinateStart; coordinateIndex < coordinateEnd; coordinateIndex += 1) {
                const start = coordinateIndex * buffers.stride;
                const position = Array.from(
                    buffers.coordinates.subarray(start, start + buffers.stride),
                );
                if (buffers.stride === 2) position.push(0);
                ring.push(position);
            }
            rings.push(ring);
        }
        features.push({
            type: "Feature",
            properties: null,
            geometry: { type: "Polygon", coordinates: rings },
        });
    }
    return { type: "FeatureCollection", features };
}

export function packedSnapshotTransferList(snapshot: PackedPolygonSnapshot): Array<ArrayBuffer> {
    return [
        snapshot.coordinates.buffer,
        snapshot.ringOffsets.buffer,
        snapshot.polygonOffsets.buffer,
        snapshot.representativeLineIds.buffer,
    ] as Array<ArrayBuffer>;
}

export function createPackedPolygonizer(exports: PackedWasmExports, memory: WebAssembly.Memory) {
    function create(
        projection: PackedProjection,
        coordinates: Float64Array,
        offsets: Uint32Array,
        stride: number,
        options: Partial<PolygonizerOptions>,
        lineIds?: Uint32Array | null,
    ): PackedPolygonResult {
        const raw = projection === "full"
            ? exports.polygonizeWithOptionsBuffer(coordinates, offsets, stride, options, lineIds)
            : exports.polygonizeGeometryWithOptionsBuffer(
                coordinates,
                offsets,
                stride,
                options,
                lineIds,
            );
        try {
            return new PackedPolygonResult(raw, memory);
        } catch (error) {
            raw.free();
            throw error;
        }
    }

    return {
        polygonizePackedWithOptions(
            coordinates: Float64Array,
            offsets: Uint32Array,
            stride: number,
            options: Partial<PolygonizerOptions>,
            lineIds?: Uint32Array | null,
        ): PackedPolygonResult {
            return create("full", coordinates, offsets, stride, options, lineIds);
        },
        polygonizePackedGeometryWithOptions(
            coordinates: Float64Array,
            offsets: Uint32Array,
            stride: number,
            options: Partial<PolygonizerOptions>,
            lineIds?: Uint32Array | null,
        ): PackedPolygonResult {
            return create("geometry", coordinates, offsets, stride, options, lineIds);
        },
    };
}
