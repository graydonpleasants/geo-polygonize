import L from 'leaflet';
import 'leaflet-draw';

// Import the WASM module
// Note: In Webpack with asyncWebAssembly, we import() the pkg folder
// But we need to build it first. We'll assume it's in ./pkg
const wasmPromise = import('./pkg/geo_polygonize.js');

let map;
let drawnItems;
let resultLayer;

async function init() {
    // Initialize Map
    map = L.map('map').setView([51.505, -0.09], 13);
    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
        attribution: '© OpenStreetMap contributors'
    }).addTo(map);

    // Feature Group for drawn items
    drawnItems = new L.FeatureGroup();
    map.addLayer(drawnItems);

    resultLayer = new L.FeatureGroup();
    map.addLayer(resultLayer);

    // Initialize Draw Controls
    const drawControl = new L.Control.Draw({
        draw: {
            polygon: false,
            marker: false,
            circle: false,
            rectangle: false,
            circlemarker: false,
            polyline: true // Only allow drawing lines
        },
        edit: {
            featureGroup: drawnItems
        }
    });
    map.addControl(drawControl);

    // Event handlers for drawing
    map.on(L.Draw.Event.CREATED, function (e) {
        drawnItems.addLayer(e.layer);
        clearResults();
    });

    map.on(L.Draw.Event.EDITED, clearResults);
    map.on(L.Draw.Event.DELETED, clearResults);

    // DOM Elements
    const fileInput = document.getElementById('file-input');
    const polygonizeBtn = document.getElementById('polygonize-btn');
    const clearBtn = document.getElementById('clear-btn');
    const statusDiv = document.getElementById('status');
    const statsDiv = document.getElementById('stats');

    // Initialize Panic Hook
    try {
        const wasm = await wasmPromise;
        if (wasm.init_panic_hook) {
            wasm.init_panic_hook();
            console.log("Panic hook initialized");
        }
    } catch (e) {
        console.warn("Failed to init wasm panic hook", e);
    }

    // File Upload Handler
    fileInput.addEventListener('change', (e) => {
        const file = e.target.files[0];
        if (!file) return;

        const reader = new FileReader();
        reader.onload = (event) => {
            try {
                const geojson = JSON.parse(event.target.result);
                drawnItems.clearLayers();
                L.geoJSON(geojson, {
                    onEachFeature: function (feature, layer) {
                        drawnItems.addLayer(layer);
                    }
                });

                // Zoom to fit
                const bounds = drawnItems.getBounds();
                if (bounds.isValid()) {
                    map.fitBounds(bounds);
                }
                clearResults();
                statusDiv.innerText = `Loaded ${file.name}`;
            } catch (err) {
                console.error(err);
                statusDiv.innerText = "Error parsing JSON";
            }
        };
        reader.readAsText(file);
    });

    // Clear Handler
    clearBtn.addEventListener('click', () => {
        drawnItems.clearLayers();
        clearResults();
        fileInput.value = '';
        statusDiv.innerText = "Cleared";
        statsDiv.innerText = "";
    });

    // Polygonize Handler
    polygonizeBtn.addEventListener('click', async () => {
        statusDiv.innerText = "Processing...";

        try {
             // 1. Get GeoJSON from drawn items
            const geojson = drawnItems.toGeoJSON();
            const geojsonStr = JSON.stringify(geojson);

            // 2. Call WASM
            const wasm = await wasmPromise;
            // console.log("WASM Module:", wasm);

            const start = performance.now();

            // Direct call to exported function
            const resultStr = wasm.polygonize(geojsonStr);

            const end = performance.now();
            const duration = (end - start).toFixed(2);

            // 3. Display Results
            const resultGeoJSON = JSON.parse(resultStr);

            resultLayer.clearLayers();
            L.geoJSON(resultGeoJSON, {
                style: {
                    color: '#ff7800',
                    weight: 2,
                    opacity: 0.65,
                    fillOpacity: 0.2
                }
            }).addTo(resultLayer);

            statusDiv.innerText = "Done!";
            statsDiv.innerText = `Polygons: ${resultGeoJSON.features.length}\nTime: ${duration}ms`;

        } catch (e) {
            console.error(e);
            statusDiv.innerText = "Error: " + e;
        }
    });

    function clearResults() {
        resultLayer.clearLayers();
        statusDiv.innerText = "Ready";
    }
}

init();
