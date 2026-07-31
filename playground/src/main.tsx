import React, { useState, useEffect, useMemo, useRef } from 'react';
import ReactDOM from 'react-dom/client';
import init, {
  polygonizeTraceWithOptionsAsync,
  type NodingGuarantee,
  type PolygonizerOptions,
  type TopologyFingerprintV1,
  type TopologyTraceV1,
} from 'geo-polygonize';
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
  Button,
  Box,
  Paper,
  Grid,
  Alert
} from '@mui/material';
import { appendLineString, parseGeojsonInput } from './input';
import { buildPlaygroundOptions } from './options';
import {
  extractTraceLayers,
  parsePlaygroundTraceReport,
  PLAYGROUND_TRACE_BYTE_LIMIT,
} from './trace';

// --- Types ---
interface ManifestEntry {
  slug: string;
  title: string;
  description: string;
  fixture: string;
  defaultOptions: {
    node_input: boolean;
    precision_model:
      | { type: 'floating' }
      | { type: 'fixed_grid'; grid_size: number };
  };
}

const layerOptions = [
  ['rawLines', 'Raw lines'],
  ['snappedLines', 'Snapped lines'],
  ['hotPixels', 'Hot pixels'],
  ['splitPoints', 'Split points'],
  ['graphEdges', 'Graph edges'],
  ['dangles', 'Dangles'],
  ['cutEdges', 'Cut edges'],
  ['invalidRings', 'Invalid rings'],
  ['shells', 'Shells'],
  ['holes', 'Holes'],
  ['finalFaces', 'Final faces'],
] as const;
type LayerKey = typeof layerOptions[number][0];

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
    if (!geom) return;
    if (geom.type === 'LineString') {
      geom.coordinates.forEach(processCoord);
    } else if (geom.type === 'Polygon') {
      geom.coordinates.forEach((ring: any) => ring.forEach(processCoord));
    }
  };

  if (geojson && geojson.features) {
    geojson.features.forEach((f: any) => processGeom(f.geometry));
  }

  if (![minX, minY, maxX, maxY].every(Number.isFinite)) return null;

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

const fingerprintView = new DataView(new ArrayBuffer(8));
function fingerprintCoordinate(value: { x: string; y: string }) {
  const number = (bits: string) => {
    fingerprintView.setBigUint64(0, BigInt(bits));
    return fingerprintView.getFloat64(0);
  };
  return [number(value.x), number(value.y)];
}

function reportToGeojson(report: TopologyFingerprintV1) {
  return {
    type: 'FeatureCollection',
    features: report.polygons.map((polygon) => ({
      type: 'Feature',
      properties: null,
      geometry: {
        type: 'Polygon',
        coordinates: [
          polygon.exterior.map(fingerprintCoordinate),
          ...polygon.interiors.map((ring) => ring.coordinates.map(fingerprintCoordinate)),
        ],
      },
    })),
  };
}

// --- Main App Component ---
function App() {
  const [manifest, setManifest] = useState<ManifestEntry[]>([]);
  const [selectedSlug, setSelectedSlug] = useState<string>('');
  const [wasmReady, setWasmReady] = useState(false);
  const [inputGeojson, setInputGeojson] = useState<any>(null);
  const [inputText, setInputText] = useState('');
  const [outputGeojson, setOutputGeojson] = useState<any>(null);
  const [report, setReport] = useState<TopologyFingerprintV1 | null>(null);
  const [trace, setTrace] = useState<TopologyTraceV1 | null>(null);
  const [traceBusy, setTraceBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [drawEnabled, setDrawEnabled] = useState(false);
  const [drawnPoints, setDrawnPoints] = useState<number[][]>([]);
  const drawnPointsRef = useRef<number[][]>([]);
  const svgRef = useRef<SVGSVGElement>(null);

  // Options
  const [nodeInput, setNodeInput] = useState(false);
  const [snapGridSize, setSnapGridSize] = useState(0.0);
  const [nodingGuarantee, setNodingGuarantee] = useState<NodingGuarantee>('Unchecked');
  const [layers, setLayers] = useState<Record<LayerKey, boolean>>({
    rawLines: true,
    snappedLines: true,
    hotPixels: true,
    splitPoints: true,
    graphEdges: true,
    dangles: true,
    cutEdges: true,
    invalidRings: true,
    shells: true,
    holes: true,
    finalFaces: true,
  });

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
    setNodingGuarantee('Unchecked');
    setSnapGridSize(
      entry.defaultOptions.precision_model.type === 'fixed_grid'
        ? entry.defaultOptions.precision_model.grid_size
        : 0.0
    );
    setInputGeojson(null);
    setOutputGeojson(null);
    setReport(null);
    setTrace(null);
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
        setInputText(JSON.stringify(data, null, 2));
      } catch (e: any) {
        setError("Failed to load fixture: " + e.toString());
      }
    }
    loadFixture();
  }, [selectedSlug, manifest]);

  const setCustomInput = (data: Record<string, unknown>) => {
    setInputGeojson(data);
    setInputText(JSON.stringify(data, null, 2));
    setSelectedSlug('');
    const url = new URL(window.location.href);
    url.searchParams.delete('scenario');
    window.history.replaceState({}, '', url.toString());
    setError(null);
  };

  const applyInput = (text: string) => {
    try {
      setCustomInput(parseGeojsonInput(text));
    } catch (e) {
      setError(`Input Error: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const loadFile = async (file?: File) => {
    if (!file) return;
    try {
      applyInput(await file.text());
    } catch (e) {
      setError(`Input Error: ${e instanceof Error ? e.message : String(e)}`);
    }
  };

  const pointerCoordinate = (event: React.PointerEvent<SVGSVGElement>) => {
    const svg = svgRef.current;
    const matrix = svg?.getScreenCTM();
    if (!svg || !matrix) return null;
    const point = svg.createSVGPoint();
    point.x = event.clientX;
    point.y = event.clientY;
    const local = point.matrixTransform(matrix.inverse());
    return [local.x, local.y];
  };

  const updateDrawnPoints = (points: number[][]) => {
    drawnPointsRef.current = points;
    setDrawnPoints(points);
  };

  const finishDrawing = (event: React.PointerEvent<SVGSVGElement>) => {
    if (!drawEnabled) return;
    const point = pointerCoordinate(event);
    const previous = drawnPointsRef.current.at(-1);
    const points = point && previous
      && Math.hypot(point[0] - previous[0], point[1] - previous[1])
        >= (bbox?.width ?? 1) * 0.002
      ? [...drawnPointsRef.current, point]
      : drawnPointsRef.current;
    if (inputGeojson && points.length >= 2) {
      setCustomInput(appendLineString(inputGeojson, points));
    }
    updateDrawnPoints([]);
  };

  // Run Polygonizer
  useEffect(() => {
    if (!wasmReady || !inputGeojson) {
      setTraceBusy(false);
      return;
    }
    const controller = new AbortController();
    const options: Partial<PolygonizerOptions> = buildPlaygroundOptions(
      nodeInput,
      snapGridSize,
      nodingGuarantee,
    );
    setTraceBusy(true);
    setTrace(null);
    void polygonizeTraceWithOptionsAsync(
      JSON.stringify(inputGeojson),
      options,
      'full',
      PLAYGROUND_TRACE_BYTE_LIMIT,
      { signal: controller.signal },
    ).then((text) => {
      const next = parsePlaygroundTraceReport(text);
      setReport(next.topology);
      setTrace(next.trace);
      setOutputGeojson(reportToGeojson(next.topology));
      setError(null);
    }).catch((e: Error) => {
      if (e.name === 'AbortError') return;
      setOutputGeojson(null);
      setReport(null);
      setTrace(null);
      setError("Polygonize Error: " + e.toString());
    }).finally(() => {
      if (!controller.signal.aborted) setTraceBusy(false);
    });
    return () => controller.abort();
  }, [wasmReady, inputGeojson, nodeInput, snapGridSize, nodingGuarantee]);


  // SVG Viewport calculation
  const bbox = useMemo(() => {
    if (!inputGeojson) return null;
    return computeBoundingBox(inputGeojson);
  }, [inputGeojson]);
  const traceLayers = useMemo(() => trace ? extractTraceLayers(trace) : null, [trace]);

  return (
    <Container maxWidth="lg" sx={{ mt: 4, mb: 4 }}>
      <Typography variant="h3" gutterBottom>
        geo-polygonize Playground
      </Typography>

      {!wasmReady && <Alert severity="info">Loading WebAssembly...</Alert>}
      {traceBusy && <Alert severity="info">Tracing topology in a worker...</Alert>}
      {error && <Alert severity="error" sx={{ mb: 2 }}>{error}</Alert>}

      <Grid container spacing={3}>
        {/* Controls */}
        <Grid size={{ xs: 12, md: 4 }}>
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
                <MenuItem value="" disabled>Custom input</MenuItem>
                {manifest.map(m => (
                  <MenuItem key={m.slug} value={m.slug}>{m.title}</MenuItem>
                ))}
              </Select>
            </FormControl>

            <Typography variant="h6" gutterBottom>Input GeoJSON</Typography>
            <Box
              onDragOver={(event) => {
                event.preventDefault();
                event.dataTransfer.dropEffect = 'copy';
              }}
              onDrop={(event) => {
                event.preventDefault();
                void loadFile(event.dataTransfer.files[0]);
              }}
              sx={{ mb: 3, p: 1.5, border: '1px dashed', borderColor: 'divider' }}
            >
              <TextField
                fullWidth
                multiline
                minRows={6}
                label="Paste GeoJSON"
                value={inputText}
                onChange={(event) => setInputText(event.target.value)}
              />
              <Box sx={{ display: 'flex', gap: 1, mt: 1 }}>
                <Button variant="contained" onClick={() => applyInput(inputText)}>
                  Apply
                </Button>
                <Button component="label" variant="outlined">
                  Upload
                  <input
                    hidden
                    type="file"
                    accept=".geojson,.json,application/geo+json,application/json"
                    onChange={(event) => void loadFile(event.target.files?.[0])}
                  />
                </Button>
                <Button
                  variant={drawEnabled ? 'contained' : 'outlined'}
                  disabled={!bbox}
                  onClick={() => {
                    setDrawEnabled((enabled) => !enabled);
                    updateDrawnPoints([]);
                  }}
                >
                  {drawEnabled ? 'Stop drawing' : 'Draw line'}
                </Button>
              </Box>
              <Typography variant="caption" color="text.secondary">
                Paste, upload, or drop GeoJSON, or drag across the geometry view to draw a line.
              </Typography>
            </Box>

            <Typography variant="h6" gutterBottom>Options</Typography>
            <FormControlLabel
              control={(
                <Switch
                  checked={nodeInput}
                  disabled={nodingGuarantee === 'CertifiedFixedPrecision'}
                  onChange={(e) => setNodeInput(e.target.checked)}
                />
              )}
              label="Node input"
            />
            <FormControl fullWidth sx={{ mt: 2 }}>
              <InputLabel id="noding-guarantee-label">Noding guarantee</InputLabel>
              <Select
                labelId="noding-guarantee-label"
                label="Noding guarantee"
                value={nodingGuarantee}
                onChange={(event) => {
                  const guarantee = event.target.value as NodingGuarantee;
                  setNodingGuarantee(guarantee);
                  if (guarantee === 'CertifiedFixedPrecision') {
                    setNodeInput(true);
                    if (snapGridSize <= 0) setSnapGridSize(0.001);
                  }
                }}
              >
                <MenuItem value="Unchecked">Unchecked</MenuItem>
                <MenuItem value="Validate">Validate</MenuItem>
                <MenuItem value="CertifiedFixedPrecision">Certified fixed precision</MenuItem>
              </Select>
            </FormControl>
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
              Enable noding for dirty geometries, then choose unchecked, independently validated,
              or certified fixed-precision output.
            </Typography>

            <Typography variant="h6" sx={{ mt: 3 }}>Layers</Typography>
            <Box sx={{ display: 'grid', gridTemplateColumns: '1fr 1fr' }}>
              {layerOptions.map(([key, label]) => (
                <FormControlLabel
                  key={key}
                  control={(
                    <Switch
                      size="small"
                      checked={layers[key]}
                      onChange={() => setLayers((current) => ({
                        ...current,
                        [key]: !current[key],
                      }))}
                    />
                  )}
                  label={label}
                />
              ))}
            </Box>
          </Paper>

          {report && (
            <Paper sx={{ p: 2, mt: 3 }}>
               <Typography variant="h6">Results</Typography>
               <Typography>Polygons found: {report.polygons.length}</Typography>
               <Typography>Dangles: {report.dangles.length}</Typography>
               <Typography>Cut edges: {report.cut_edges.length}</Typography>
               <Typography>Invalid rings: {report.invalid_rings.length}</Typography>
               {trace && (
                 <>
                   <Typography>Trace events: {trace.events.length}</Typography>
                   <Typography>
                     Trace bytes: {trace.bytes_used.toLocaleString()} / {trace.byte_limit.toLocaleString()}
                   </Typography>
                   {trace.truncated && <Alert severity="warning">Trace reached its byte budget.</Alert>}
                 </>
               )}
               <TextField
                 fullWidth
                 multiline
                 minRows={6}
                 label="Resolved canonical options"
                 value={JSON.stringify(report.options, null, 2)}
                 InputProps={{ readOnly: true }}
                 sx={{ mt: 2 }}
               />
            </Paper>
          )}
        </Grid>

        {/* Visualizer */}
        <Grid size={{ xs: 12, md: 8 }}>
          <Paper sx={{ p: 2, height: '600px', display: 'flex', flexDirection: 'column' }}>
            <Typography variant="h6" gutterBottom>Geometry View</Typography>
            {bbox && (
               <Box sx={{ flexGrow: 1, border: '1px solid #ccc', position: 'relative', overflow: 'hidden' }}>
                  <svg
                    ref={svgRef}
                    width="100%"
                    height="100%"
                    viewBox={`${bbox.minX} ${bbox.minY} ${bbox.width} ${bbox.height}`}
                    preserveAspectRatio="xMidYMid meet"
                    onPointerDown={(event) => {
                      if (!drawEnabled) return;
                      event.preventDefault();
                      const point = pointerCoordinate(event);
                      if (!point) return;
                      event.currentTarget.setPointerCapture(event.pointerId);
                      updateDrawnPoints([point]);
                    }}
                    onPointerMove={(event) => {
                      if (!drawEnabled || drawnPointsRef.current.length === 0) return;
                      const point = pointerCoordinate(event);
                      if (!point) return;
                      const previous = drawnPointsRef.current.at(-1)!;
                      const minimumDistance = bbox.width * 0.002;
                      if (Math.hypot(point[0] - previous[0], point[1] - previous[1]) >= minimumDistance) {
                        updateDrawnPoints([...drawnPointsRef.current, point]);
                      }
                    }}
                    onPointerUp={finishDrawing}
                    onPointerCancel={() => updateDrawnPoints([])}
                    style={{
                      transform: 'scaleY(-1)',
                      cursor: drawEnabled ? 'crosshair' : 'default',
                      touchAction: drawEnabled ? 'none' : 'auto',
                    }} // Invert Y axis for standard Cartesian coords
                  >
                     {/* Draw Output Polygons (filled) */}
                     {layers.finalFaces && outputGeojson?.features.map((f: any, i: number) => {
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
                   {layers.rawLines && inputGeojson?.features.map((f: any, i: number) => {
                        if (f.geometry?.type === 'LineString') {
                           const pts = f.geometry.coordinates.map((c: any) => `${c[0]},${c[1]}`).join(" ");
                           return <polyline key={`line-${i}`} points={pts} fill="none" stroke="#ff0000" strokeWidth={bbox.width * 0.003} strokeDasharray={`${bbox.width * 0.01},${bbox.width * 0.01}`} />;
                        }
                        return null;
                     })}

                     {layers.snappedLines && traceLayers?.snappedLines.map((line) => (
                       <line
                         key={`snapped-${line.sequence}`}
                         x1={line.start[0]}
                         y1={line.start[1]}
                         x2={line.end[0]}
                         y2={line.end[1]}
                         stroke="#3949ab"
                         strokeWidth={bbox.width * 0.004}
                       />
                     ))}

                     {layers.graphEdges && traceLayers?.graphEdges.map((line) => (
                       <line
                         key={`graph-${line.sequence}`}
                         x1={line.start[0]}
                         y1={line.start[1]}
                         x2={line.end[0]}
                         y2={line.end[1]}
                         stroke="#263238"
                         strokeWidth={bbox.width * 0.002}
                       />
                     ))}

                     {layers.hotPixels && traceLayers?.hotPixels.map((point) => (
                       <rect
                         key={`hot-${point.sequence}`}
                         x={point.coordinate[0] - bbox.width * 0.004}
                         y={point.coordinate[1] - bbox.width * 0.004}
                         width={bbox.width * 0.008}
                         height={bbox.width * 0.008}
                         fill="#d81b60"
                       />
                     ))}

                     {layers.splitPoints && traceLayers?.splitPoints.map((point) => (
                       <circle
                         key={`split-${point.sequence}`}
                         cx={point.coordinate[0]}
                         cy={point.coordinate[1]}
                         r={bbox.width * 0.005}
                         fill="#8e24aa"
                       />
                     ))}

                     {layers.shells && report?.polygons.map((polygon, index) => (
                       <polyline
                         key={`shell-${index}`}
                         points={polygon.exterior.map(fingerprintCoordinate).map((point) => point.join(',')).join(' ')}
                         fill="none"
                         stroke="#0055aa"
                         strokeWidth={bbox.width * 0.004}
                       />
                     ))}

                     {layers.holes && report?.polygons.flatMap((polygon, polygonIndex) => (
                       polygon.interiors.map((ring, ringIndex) => (
                         <polyline
                           key={`hole-${polygonIndex}-${ringIndex}`}
                           points={ring.coordinates.map(fingerprintCoordinate).map((point) => point.join(',')).join(' ')}
                           fill="none"
                           stroke="#00897b"
                           strokeWidth={bbox.width * 0.004}
                         />
                       ))
                     ))}

                     {layers.dangles && report?.dangles.map((line, index) => (
                       <polyline
                         key={`dangle-${index}`}
                         points={line.map(fingerprintCoordinate).map((point) => point.join(',')).join(' ')}
                         fill="none"
                         stroke="#f9a825"
                         strokeWidth={bbox.width * 0.005}
                       />
                     ))}

                     {layers.cutEdges && report?.cut_edges.map((line, index) => (
                       <polyline
                         key={`cut-${index}`}
                         points={line.map(fingerprintCoordinate).map((point) => point.join(',')).join(' ')}
                         fill="none"
                         stroke="#ef6c00"
                         strokeWidth={bbox.width * 0.005}
                       />
                     ))}

                     {layers.invalidRings && report?.invalid_rings.map((line, index) => (
                       <polyline
                         key={`invalid-${index}`}
                         points={line.map(fingerprintCoordinate).map((point) => point.join(',')).join(' ')}
                         fill="none"
                         stroke="#c62828"
                         strokeWidth={bbox.width * 0.006}
                       />
                     ))}

                     {drawnPoints.length > 1 && (
                       <polyline
                         points={drawnPoints.map((point) => point.join(',')).join(' ')}
                         fill="none"
                         stroke="#7b1fa2"
                         strokeWidth={bbox.width * 0.004}
                       />
                     )}
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
