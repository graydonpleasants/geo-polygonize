import { describe, it, expect } from 'vitest';
import init, { polygonize } from '../../../dist/standard/es/index.js';

describe('WASM Polygonizer', () => {
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
        await init();
        const input = "{ invalid json }";
        expect(() => polygonize(input)).toThrow(/Invalid GeoJSON/);
    });

    it('should throw error on empty string input', async () => {
        await init();
        const input = "";
        expect(() => polygonize(input)).toThrow(/Invalid GeoJSON/);
        expect(() => polygonize(input)).toThrow(/EOF while parsing a value/);
    });

    it('should throw error on valid JSON but invalid GeoJSON structure', async () => {
        await init();
        // Missing "type" field
        const input = JSON.stringify({ "foo": "bar" });
        expect(() => polygonize(input)).toThrow(/Invalid GeoJSON/);

        // Invalid geometry type
        const input2 = JSON.stringify({
            "type": "Feature",
            "geometry": {
                "type": "InvalidType",
                "coordinates": []
            }
        });
        expect(() => polygonize(input2)).toThrow(/Invalid GeoJSON/);
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

        // Explicit parity behavior (node_input = true, snap_grid_size = 0.5):
        // The crossing lines are correctly noded at the intersection,
        // yielding 4 separate triangle polygons.
        const parityResultJson = polygonize(JSON.stringify(input), true, 0.5, false);
        const parityResult = JSON.parse(parityResultJson);

        // With node_input=true, we expect to find the 4 quadrants formed by the crossed square
        expect(parityResult.features.length).toBeGreaterThan(defaultResult.features.length);
        expect(parityResult.features.length).toBe(4);
    });
});
