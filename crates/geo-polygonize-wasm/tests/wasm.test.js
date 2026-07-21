import { describe, it, expect } from 'vitest';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join, resolve } from 'node:path';
import init, { polygonize, cfbRobustOptions } from '../../../dist/standard/es/index.js';

describe('WASM Polygonizer', () => {
    it('should publish declaration paths referenced by wrapper types', () => {
        expect(existsSync(resolve('dist/standard/pkg-scalar/geo_polygonize.d.ts'))).toBe(true);
        expect(existsSync(resolve('dist/slim/pkg-scalar/geo_polygonize.d.ts'))).toBe(true);
        expect(existsSync(resolve('dist/threads/pkg-threads/geo_polygonize.d.ts'))).toBe(true);
    });

    it('should initialize slim with module alias options', async () => {
        const { cfbRobustOptions, initBest } = await import('../../../dist/slim/es/index_slim.js');
        const wasm = await initBest(
            { module: await WebAssembly.compile(readFileSync(resolve('dist/geo_polygonize.wasm'))) },
            { module: await WebAssembly.compile(readFileSync(resolve('dist/geo_polygonize_simd.wasm'))) },
        );

        const input = {
            type: "FeatureCollection",
            features: [
                {
                    type: "Feature",
                    geometry: {
                        type: "LineString",
                        coordinates: [[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]
                    }
                }
            ]
        };

        const result = JSON.parse(wasm.polygonizeWithOptions(JSON.stringify(input), cfbRobustOptions));
        expect(result.features).toHaveLength(1);
    });

    it('should polygonize a simple square', async () => {
        await init();

        const input = {
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]
                    }
                }
            ]
        };

        const resultJson = polygonize(JSON.stringify(input));
        const result = JSON.parse(resultJson);

        expect(result.type).toBe("FeatureCollection");
        expect(result.features).toHaveLength(1);
        expect(result.features[0].geometry.type).toBe("Polygon");

        // Check coordinates exist
        const coords = result.features[0].geometry.coordinates;
        expect(coords.length).toBeGreaterThan(0);
    });

    it('should apply defaults to partial options', async () => {
        const { default: initModule, polygonizeWithOptions } = await import('../../../dist/standard/es/index.js');
        await initModule();
        const input = {
            type: "FeatureCollection",
            features: [{
                type: "Feature",
                geometry: {
                    type: "LineString",
                    coordinates: [[0, 0], [1, 0], [1, 1], [0, 1], [0, 0]],
                },
            }],
        };

        expect(JSON.parse(polygonizeWithOptions(JSON.stringify(input), {})).features).toHaveLength(1);
        expect(() => polygonizeWithOptions(JSON.stringify(input), {
            precision_model: { type: "fixed_grid", grid_size: -1 },
        })).toThrow(/precision_model.grid_size/);

        const crossing = {
            type: "FeatureCollection",
            features: [
                { type: "Feature", geometry: { type: "LineString", coordinates: [[-1, 0], [1, 0]] } },
                { type: "Feature", geometry: { type: "LineString", coordinates: [[0, -1], [0, 1]] } },
            ],
        };
        expect(() => polygonizeWithOptions(JSON.stringify(crossing), {
            noding: { guarantee: "Validate" },
        })).toThrow(/Noding validation failed/);
        expect(JSON.parse(polygonizeWithOptions(JSON.stringify(crossing), {
            node_input: true,
            precision_model: { type: "fixed_grid", grid_size: 1 },
            noding: { guarantee: "CertifiedFixedPrecision" },
        })).features).toHaveLength(0);
    });

    it('should handle empty input', async () => {
        await init();
        const input = {
            "type": "FeatureCollection",
            "features": []
        };
        const resultJson = polygonize(JSON.stringify(input));
        const result = JSON.parse(resultJson);
        expect(result.features).toHaveLength(0);
    });

    it('should throw error on invalid JSON syntax', async () => {
        expect.assertions(2);
        await init();
        const input = "{ invalid json }";
        try {
            polygonize(input);
        } catch (e) {
            expect(e.name).toBe("InvalidArgumentType");
            expect(e.message).toMatch(/Invalid GeoJSON/);
        }
    });

    it('should throw error on empty string input', async () => {
        expect.assertions(2);
        await init();
        const input = "";
        try {
            polygonize(input);
        } catch (e) {
            expect(e.name).toBe("InvalidArgumentType");
            expect(e.message).toMatch(/Invalid GeoJSON/);
        }
    });

    it('should throw error on valid JSON but invalid GeoJSON structure', async () => {
        expect.assertions(4);
        await init();
        // Missing "type" field
        const input = JSON.stringify({ "foo": "bar" });
        try {
            polygonize(input);
        } catch (e) {
            expect(e.name).toBe("InvalidArgumentType");
            expect(e.message).toMatch(/Invalid GeoJSON/);
        }

        // Invalid geometry type
        const input2 = JSON.stringify({
            "type": "Feature",
            "geometry": {
                "type": "InvalidType",
                "coordinates": []
            }
        });
        try {
            polygonize(input2);
        } catch (e) {
            expect(e.name).toBe("InvalidArgumentType");
            expect(e.message).toMatch(/Invalid GeoJSON/);
        }
    });

    it('should behave differently with explicit options (parity with backend)', async () => {
        await init();

        // Cross without a node at [5, 5]
        const input = {
            "type": "FeatureCollection",
            "features": [
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[0, 0], [10, 10]]
                    }
                },
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[0, 10], [10, 0]]
                    }
                },
                // Add a bounding box to close the shape
                {
                    "type": "Feature",
                    "geometry": {
                        "type": "LineString",
                        "coordinates": [[0, 0], [10, 0], [10, 10], [0, 10], [0, 0]]
                    }
                }
            ]
        };

        // Default behavior (node_input = false): The crossing lines are not noded,
        // so no smaller polygons are found, just the outer bounding box and maybe dangles.
        const defaultResultJson = polygonize(JSON.stringify(input));
        const defaultResult = JSON.parse(defaultResultJson);

        // Explicit parity behavior (node_input = true, snap_grid_size = 0.1):
        // The crossing lines are correctly noded at the intersection,
        // yielding 4 separate triangle polygons.
        const parityResultJson = polygonize(JSON.stringify(input), true, 0.1, false);
        const parityResult = JSON.parse(parityResultJson);

        // With node_input=true, we expect to find the 4 quadrants formed by the crossed square
        expect(parityResult.features.length).toBeGreaterThan(defaultResult.features.length);
        expect(parityResult.features.length).toBe(4);
    });

    it('should pass line_ids and return flat_line_ids via buffer API', async () => {
        await init();
        const { polygonizeWithOptionsBuffer } = await import('../../../dist/standard/es/index.js');

        const coords = new Float64Array([0, 0, 10, 0, 10, 10, 0, 10, 0, 0]);
        const offsets = new Uint32Array([0]);
        const stride = 2;
        const line_ids = new Uint32Array([42]);
        const options = { node_input: false, precision_model: { type: 'floating' }, extract_only_polygonal: false, snap_strategy: 'Grid', noding: { backend: 'Snap' }, containment: { touch_policy: 'AllowPointTouchDisallowEdgeShare' }, determinism: { canonical_sort: false, canonical_ring_rotation: false, stable_tie_breaks: false }, diagnostics: { enabled: false, report_mode: false }, provenance: { enabled: false, include_boundary_line_ids: false }, input_profile_id: null };

        const result = polygonizeWithOptionsBuffer(coords, offsets, stride, options, line_ids);

        expect(result).toBeDefined();
        const numIds = result.flat_line_ids_len();
        expect(numIds).toBeGreaterThan(0);

        // We can't easily read from WASM memory directly without the wasm object,
        // but we can at least assert the binding doesn't crash and returns the expected length.
        // It returns 5 elements for the single ring (5 coordinates)
        expect(numIds).toBe(5);
    });

    it('should throw error when line_ids length does not match offsets length', async () => {
        expect.assertions(2);
        await init();
        const { polygonizeWithOptionsBuffer } = await import('../../../dist/standard/es/index.js');

        const coords = new Float64Array([0, 0, 10, 0, 10, 10, 0, 10, 0, 0]);
        const offsets = new Uint32Array([0]); // 1 line
        const stride = 2;
        const line_ids = new Uint32Array([42, 43]); // 2 ids
        const options = { node_input: false, precision_model: { type: 'floating' }, extract_only_polygonal: false, snap_strategy: 'Grid', noding: { backend: 'Snap' }, containment: { touch_policy: 'AllowPointTouchDisallowEdgeShare' }, determinism: { canonical_sort: false, canonical_ring_rotation: false, stable_tie_breaks: false }, diagnostics: { enabled: false, report_mode: false }, provenance: { enabled: false, include_boundary_line_ids: false }, input_profile_id: null };

        try {
            polygonizeWithOptionsBuffer(coords, offsets, stride, options, line_ids);
        } catch (e) {
            expect(e.name).toBe("InvalidBufferShape");
            expect(e.message).toMatch(/line_ids length 2 does not match line count 1/);
        }
    });

    it('should throw error when stride is invalid in polygonize_buffers', async () => {
        expect.assertions(2);
        await init();
        const { polygonize_buffers } = await import('../../../dist/standard/es/index.js');

        const coords = new Float64Array([0, 0, 10, 0, 10, 10, 0, 10, 0, 0]);
        const offsets = new Uint32Array([0]);

        try {
            polygonize_buffers(coords, offsets, 4, false, 1e-10);
        } catch (e) {
            expect(e.name).toBe("InvalidArgumentType");
            expect(e.message).toBe("stride must be 2 or 3");
        }
    });

    it('should throw error when stride is invalid in polygonizeWithOptionsBuffer', async () => {
        expect.assertions(2);
        await init();
        const { polygonizeWithOptionsBuffer } = await import('../../../dist/standard/es/index.js');

        const coords = new Float64Array([0, 0, 10, 0, 10, 10, 0, 10, 0, 0]);
        const offsets = new Uint32Array([0]);
        const options = { node_input: false, precision_model: { type: 'floating' }, extract_only_polygonal: false, snap_strategy: 'Grid', noding: { backend: 'Snap' }, containment: { touch_policy: 'AllowPointTouchDisallowEdgeShare' }, determinism: { canonical_sort: false, canonical_ring_rotation: false, stable_tie_breaks: false }, diagnostics: { enabled: false, report_mode: false }, provenance: { enabled: false, include_boundary_line_ids: false }, input_profile_id: null };

        try {
            polygonizeWithOptionsBuffer(coords, offsets, 1, options);
        } catch (e) {
            expect(e.name).toBe("InvalidArgumentType");
            expect(e.message).toBe("stride must be 2 or 3");
        }
    });

    it('should run CFB fixtures through polygonizeWithOptionsBuffer', async () => {
        await init();
        const { polygonizeWithOptionsBuffer } = await import('../../../dist/standard/es/index.js');
        const fixtureDir = resolve('fixtures/cfb/cases');

        for (const file of readdirSync(fixtureDir).filter((name) => name.endsWith('.json')).sort()) {
            const fixture = JSON.parse(readFileSync(join(fixtureDir, file), 'utf8'));
            if (fixture.expectedStatus === 'xfail') continue;

            const coords = [];
            const offsets = [];
            const lineIds = [];
            let offset = 0;

            for (const line of fixture.lines) {
                offsets.push(offset);
                lineIds.push(line.id);
                for (const point of line.coords) {
                    coords.push(...point.slice(0, fixture.stride));
                    offset += 1;
                }
            }

            const result = polygonizeWithOptionsBuffer(
                new Float64Array(coords),
                new Uint32Array(offsets),
                fixture.stride,
                cfbRobustOptions,
                new Uint32Array(lineIds),
            );

            expect(result.polygon_offsets_len()).toBe(fixture.expected.polygonCount);
            expect(result.diagnostics.dangle_count).toBe(fixture.expected.dangleCount);
            expect(result.diagnostics.cut_edge_count).toBe(fixture.expected.cutEdgeCount);
            expect(result.diagnostics.invalid_ring_count).toBe(fixture.expected.invalidRingCount);
        }
    });

});
