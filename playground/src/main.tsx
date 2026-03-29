import React, { useState, useEffect, useMemo } from 'react';
import ReactDOM from 'react-dom/client';
import init, { polygonize } from 'geo-polygonize';
import {
  Container,
  Typography,
  Select,
  MenuItem,
  FormControl,
  InputLabel,
  Switch,
  FormControlLabel,
  TextField,
  Box,
  Paper,
  Grid,
  Alert
} from '@mui/material';

// --- Types ---
interface ManifestEntry {
  slug: string;
  title: string;
  description: string;
  fixture: string;
  defaultOptions: {
    node_input: boolean;
    snap_grid_size: number;
  };
}

// --- Utils ---
function computeBoundingBox(geojson: any) {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
  const processCoord = (coord: number[]) => {
    minX = Math.min(minX, coord[0]);
    minY = Math.min(minY, coord[1]);
    maxX = Math.max(maxX, coord[0]);
    maxY = Math.max(maxY, coord[1]);
  };

  const processGeom = (geom: any) => {
    if (geom.type === 'LineString') {
      geom.coordinates.forEach(processCoord);
    } else if (geom.type === 'Polygon') {
      geom.coordinates.forEach((ring: any) => ring.forEach(processCoord));
    }
  };

  if (geojson && geojson.features) {
    geojson.features.forEach((f: any) => processGeom(f.geometry));
  }

  // Padding
  const padX = (maxX - minX) * 0.1;
  const padY = (maxY - minY) * 0.1;
  return {
    minX: minX - padX,
    minY: minY - padY,
    maxX: maxX + padX,
    maxY: maxY + padY,
    width: maxX - minX + 2 * padX,
    height: maxY - minY + 2 * padY
  };
}

// --- Main App Component ---
function App() {
  const [manifest, setManifest] = useState<ManifestEntry[]>([]);
  const [selectedSlug, setSelectedSlug] = useState<string>('');
  const [wasmReady, setWasmReady] = useState(false);
  const [inputGeojson, setInputGeojson] = useState<any>(null);
  const [outputGeojson, setOutputGeojson] = useState<any>(null);
  const [error, setError] = useState<string | null>(null);

  // Options
  const [nodeInput, setNodeInput] = useState(false);
  const [snapGridSize, setSnapGridSize] = useState(0.0);

  // Initialize WASM and fetch manifest
  useEffect(() => {
    async function load() {
      try {
        await init();
        setWasmReady(true);

        const manifestUrls = [
          '/geo-polygonize/playground/examples/manifest.json', // Production path
          '/geo-polygonize/examples/manifest.json', // Dev path
          '/examples/manifest.json' // Dev server fallback
        ];

        let manifestData = null;
        for (const url of manifestUrls) {
           try {
              const res = await fetch(url);
              if (res.ok) {
                 const text = await res.text();
                 if (text && text.trim().startsWith('[')) {
                    manifestData = JSON.parse(text);
                    break;
                 }
              }
           } catch (e) {
              // ignore
           }
        }

        if (!manifestData) throw new Error("Could not find manifest.json");
        setManifest(manifestData);
      } catch (e: any) {
        setError("Failed to initialize: " + e.toString());
      }
    }
    load();
  }, []);

  // Parse URL for scenario
  useEffect(() => {
    if (manifest.length > 0 && !selectedSlug) {
      const params = new URLSearchParams(window.location.search);
      const scenario = params.get('scenario');
      if (scenario && manifest.find(m => m.slug === scenario)) {
        setSelectedSlug(scenario);
      } else {
        setSelectedSlug(manifest[0].slug);
      }
    }
  }, [manifest, selectedSlug]);

  // Load selected fixture
  useEffect(() => {
    if (!selectedSlug) return;
    const entry = manifest.find(m => m.slug === selectedSlug);
    if (!entry) return;

    setNodeInput(entry.defaultOptions.node_input);
    setSnapGridSize(entry.defaultOptions.snap_grid_size);
    setInputGeojson(null);
    setOutputGeojson(null);
    setError(null);

    async function loadFixture() {
      try {
        const fixtureUrls = [
           `/geo-polygonize/playground/examples/${entry?.fixture}`,
           `/geo-polygonize/examples/${entry?.fixture}`,
           `/examples/${entry?.fixture}`
        ];
        let data = null;
        for (const url of fixtureUrls) {
           try {
              const res = await fetch(url);
              if (res.ok) {
                 const text = await res.text();
                 if (text && text.trim().startsWith('{')) {
                    data = JSON.parse(text);
                    break;
                 }
              }
           } catch(err) {}
        }
        if (!data) throw new Error("Could not load fixture");
        setInputGeojson(data);
      } catch (e: any) {
        setError("Failed to load fixture: " + e.toString());
      }
    }
    loadFixture();
  }, [selectedSlug, manifest]);

  // Run Polygonizer
  useEffect(() => {
    if (!wasmReady || !inputGeojson) return;
    try {
      const resultStr = polygonize(JSON.stringify(inputGeojson), nodeInput, snapGridSize);
      setOutputGeojson(JSON.parse(resultStr));
      setError(null);
    } catch (e: any) {
      setOutputGeojson(null);
      setError("Polygonize Error: " + e.toString());
    }
  }, [wasmReady, inputGeojson, nodeInput, snapGridSize]);


  // SVG Viewport calculation
  const bbox = useMemo(() => {
    if (!inputGeojson) return null;
    return computeBoundingBox(inputGeojson);
  }, [inputGeojson]);

  return (
    <Container maxWidth="lg" sx={{ mt: 4, mb: 4 }}>
      <Typography variant="h3" gutterBottom>
        geo-polygonize Playground
      </Typography>

      {!wasmReady && <Alert severity="info">Loading WebAssembly...</Alert>}
      {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}

      <Grid container spacing={3}>
        {/* Controls */}
        <Grid item xs={12} md={4}>
          <Paper sx={{ p: 2 }}>
            <FormControl fullWidth sx={{ mb: 3 }}>
              <InputLabel id="scenario-label">Scenario</InputLabel>
              <Select
                labelId="scenario-label"
                value={selectedSlug}
                label="Scenario"
                onChange={(e) => {
                  setSelectedSlug(e.target.value as string);
                  // Update URL
                  const url = new URL(window.location.href);
                  url.searchParams.set('scenario', e.target.value as string);
                  window.history.replaceState({}, '', url.toString());
                }}
              >
                {manifest.map(m => (
                  <MenuItem key={m.slug} value={m.slug}>{m.title}</MenuItem>
                ))}
              </Select>
            </FormControl>

            <Typography variant="h6" gutterBottom>Options</Typography>
            <FormControlLabel
              control={<Switch checked={nodeInput} onChange={(e) => setNodeInput(e.target.checked)} />}
              label="Node Input (Iterated Snap Rounding)"
            />
            <TextField
              fullWidth
              label="Snap Grid Size"
              type="number"
              inputProps={{ step: "0.000001" }}
              value={snapGridSize}
              onChange={(e) => setSnapGridSize(parseFloat(e.target.value) || 0)}
              sx={{ mt: 2 }}
              disabled={!nodeInput}
            />
            <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
              Enable Node Input for dirty geometries. Snap Grid Size controls the snapping precision.
            </Typography>
          </Paper>

          {outputGeojson && (
            <Paper sx={{ p: 2, mt: 3 }}>
               <Typography variant="h6">Results</Typography>
               <Typography>Polygons found: {outputGeojson.features.length}</Typography>
            </Paper>
          )}
        </Grid>

        {/* Visualizer */}
        <Grid item xs={12} md={8}>
          <Paper sx={{ p: 2, height: '600px', display: 'flex', flexDirection: 'column' }}>
            <Typography variant="h6" gutterBottom>Geometry View</Typography>
            {bbox && (
               <Box sx={{ flexGrow: 1, border: '1px solid #ccc', position: 'relative', overflow: 'hidden' }}>
                  <svg
                    width="100%"
                    height="100%"
                    viewBox={`${bbox.minX} ${bbox.minY} ${bbox.width} ${bbox.height}`}
                    preserveAspectRatio="xMidYMid meet"
                    style={{ transform: 'scaleY(-1)' }} // Invert Y axis for standard Cartesian coords
                  >
                     {/* Draw Output Polygons (filled) */}
                     {outputGeojson?.features.map((f: any, i: number) => {
                        if (f.geometry.type === 'Polygon') {
                          // SVG paths
                          let d = "";
                          f.geometry.coordinates.forEach((ring: any[]) => {
                             d += "M " + ring.map(c => `${c[0]},${c[1]}`).join(" L ") + " Z ";
                          });
                          // Alternate colors to distinguish
                          const colors = ["rgba(0,150,255,0.4)", "rgba(255,100,0,0.4)", "rgba(0,200,100,0.4)", "rgba(150,0,200,0.4)", "rgba(255,200,0,0.4)"];
                          return <path key={`poly-${i}`} d={d} fill={colors[i % colors.length]} stroke="#0055aa" strokeWidth={bbox.width * 0.002} />;
                        }
                        return null;
                     })}

                     {/* Draw Input Lines (dashed) */}
                     {inputGeojson?.features.map((f: any, i: number) => {
                        if (f.geometry.type === 'LineString') {
                           const pts = f.geometry.coordinates.map((c: any) => `${c[0]},${c[1]}`).join(" ");
                           return <polyline key={`line-${i}`} points={pts} fill="none" stroke="#ff0000" strokeWidth={bbox.width * 0.003} strokeDasharray={`${bbox.width * 0.01},${bbox.width * 0.01}`} />;
                        }
                        return null;
                     })}
                  </svg>
               </Box>
            )}
          </Paper>
        </Grid>
      </Grid>
    </Container>
  );
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)