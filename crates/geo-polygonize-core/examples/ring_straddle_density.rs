//! Measure how often point-in-ring batches contain a straddling edge.
//!
//! `SimdRing::contains` evaluates four consecutive edges per batch. The
//! benchmark-only `wide_contains_skip_empty` kernel in `hole_sort_bench.rs`
//! skips the intersection arithmetic for batches where no edge straddles the
//! probe's y, so its benefit depends on the fraction of active batches in real
//! queries. This tool polygonizes each input and replays approximations of the
//! two production query families:
//!
//! - hole assignment: probes near each hole vertex against every output shell
//!   whose envelope contains the probe;
//! - interior probes: probes near each ring's own vertices against that ring.
//!
//! Probes sit `1e-9 × bbox diagonal` above and below each vertex, mirroring the
//! offset that `guaranteed_interior_probe_prepared` uses. Only full four-edge
//! batches are counted; the scalar tail is excluded because both kernels treat
//! it identically.
//!
//! Usage:
//!   cargo run --release -p geo-polygonize-core --example ring_straddle_density -- [--node] PATH...
//!
//! Each PATH is a GeoJSON file, a CFB fixture (`*.fixture.json`), or a
//! directory searched recursively for either. Pass `--node` to set
//! `node_input` for GeoJSON inputs, as the floating benchmark lane does for
//! unnoded linework such as the OSM production tiers. CFB fixtures always use
//! their declared options profile.

use geo_polygonize_core::{polygonize, Coord3D, Line3D, PolygonizerOptions};
use geojson::{GeoJson, Value};
use std::f64::consts::PI;
use std::path::{Path, PathBuf};

/// Ring-size buckets, by coordinate count, aligned with the production
/// crossover: rings with 257 or more coordinates take the scalar path on
/// x86-64 and Apple AArch64.
const BUCKETS: [(&str, usize, usize); 4] = [
    ("5-32", 5, 33),
    ("33-128", 33, 129),
    ("129-256", 129, 257),
    ("257+", 257, usize::MAX),
];

#[derive(Default, Clone, Copy)]
struct Tally {
    queries: u64,
    batches: u64,
    active_batches: u64,
    edges: u64,
    straddling_edges: u64,
}

impl Tally {
    fn add(&mut self, other: Tally) {
        self.queries += other.queries;
        self.batches += other.batches;
        self.active_batches += other.active_batches;
        self.edges += other.edges;
        self.straddling_edges += other.straddling_edges;
    }
}

#[derive(Default)]
struct Report {
    hole_assignment: [Tally; BUCKETS.len()],
    interior_probe: [Tally; BUCKETS.len()],
}

struct Ring {
    x: Vec<f64>,
    y: Vec<f64>,
    min: [f64; 2],
    max: [f64; 2],
}

impl Ring {
    fn new(coords: &[Coord3D]) -> Self {
        let x: Vec<f64> = coords.iter().map(|c| c.x).collect();
        let y: Vec<f64> = coords.iter().map(|c| c.y).collect();
        let fold = |values: &[f64], f: fn(f64, f64) -> f64, start: f64| {
            values.iter().copied().fold(start, f)
        };
        Self {
            min: [
                fold(&x, f64::min, f64::INFINITY),
                fold(&y, f64::min, f64::INFINITY),
            ],
            max: [
                fold(&x, f64::max, f64::NEG_INFINITY),
                fold(&y, f64::max, f64::NEG_INFINITY),
            ],
            x,
            y,
        }
    }

    fn len(&self) -> usize {
        self.x.len()
    }

    fn diagonal(&self) -> f64 {
        (self.max[0] - self.min[0]).hypot(self.max[1] - self.min[1])
    }

    fn envelope_contains(&self, x: f64, y: f64) -> bool {
        self.min[0] <= x && x <= self.max[0] && self.min[1] <= y && y <= self.max[1]
    }

    /// Batch statistics for one probe, using the kernel's straddle predicate.
    fn tally(&self, py: f64) -> Tally {
        let segments = self.len() - 1;
        let batches = segments / 4;
        let mut tally = Tally {
            queries: 1,
            batches: batches as u64,
            edges: (batches * 4) as u64,
            ..Tally::default()
        };
        for batch in 0..batches {
            let straddling = (batch * 4..batch * 4 + 4)
                .filter(|&i| (self.y[i] > py) != (self.y[i + 1] > py))
                .count();
            tally.straddling_edges += straddling as u64;
            tally.active_batches += u64::from(straddling > 0);
        }
        tally
    }
}

fn bucket(len: usize) -> Option<usize> {
    BUCKETS
        .iter()
        .position(|&(_, low, high)| low <= len && len < high)
}

fn vertex_probes(ring: &Ring) -> impl Iterator<Item = (f64, f64)> + '_ {
    let eps = ring.diagonal() * 1e-9;
    (0..ring.len() - 1)
        .flat_map(move |i| [(ring.x[i], ring.y[i] + eps), (ring.x[i], ring.y[i] - eps)])
}

fn measure(shells: &[Ring], holes: &[Ring], report: &mut Report) {
    for ring in shells.iter().chain(holes) {
        let Some(b) = bucket(ring.len()) else {
            continue;
        };
        for (_, py) in vertex_probes(ring) {
            report.interior_probe[b].add(ring.tally(py));
        }
    }
    for hole in holes {
        for (px, py) in vertex_probes(hole) {
            for shell in shells {
                if !shell.envelope_contains(px, py) {
                    continue;
                }
                if let Some(b) = bucket(shell.len()) {
                    report.hole_assignment[b].add(shell.tally(py));
                }
            }
        }
    }
}

fn collect_inputs(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .unwrap_or_else(|err| panic!("{}: {err}", path.display()))
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for entry in entries {
            collect_inputs(&entry, out);
        }
    } else if path
        .extension()
        .is_some_and(|ext| ext == "geojson" || ext == "json")
    {
        out.push(path.to_path_buf());
    }
}

fn geojson_lines(value: &Value, id: u32, lines: &mut Vec<Line3D>) {
    let mut push_path = |path: &Vec<Vec<f64>>| {
        for pair in path.windows(2) {
            let coord = |p: &[f64]| Coord3D::new(p[0], p[1], p.get(2).copied().unwrap_or(0.0));
            lines.push(Line3D::new(coord(&pair[0]), coord(&pair[1]), id));
        }
    };
    match value {
        Value::LineString(path) => push_path(path),
        Value::MultiLineString(paths) | Value::Polygon(paths) => paths.iter().for_each(push_path),
        Value::MultiPolygon(polygons) => polygons.iter().flatten().for_each(push_path),
        Value::GeometryCollection(geometries) => {
            for geometry in geometries {
                geojson_lines(&geometry.value, id, lines);
            }
        }
        Value::Point(_) | Value::MultiPoint(_) => {}
    }
}

/// Returns the input segments and the options profile, or `None` when the file
/// is neither a CFB fixture nor a GeoJSON document with line geometry.
fn load(path: &Path, node_geojson: bool) -> Option<(Vec<Line3D>, PolygonizerOptions)> {
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;

    if let Some(raw_lines) = json.get("lines").and_then(|v| v.as_array()) {
        let stride = json.get("stride").and_then(|v| v.as_u64()).unwrap_or(2);
        let options = match json.get("optionsProfile").and_then(|v| v.as_str()) {
            Some("cfb_robust_v1") => PolygonizerOptions::cfb_robust_v1(),
            _ => PolygonizerOptions::default(),
        };
        let mut lines = Vec::new();
        for line in raw_lines {
            let id = line.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let coords: Vec<Vec<f64>> = serde_json::from_value(line.get("coords")?.clone()).ok()?;
            for pair in coords.windows(2) {
                let coord = |p: &[f64]| {
                    let z = if stride == 3 { p[2] } else { 0.0 };
                    Coord3D::new(p[0], p[1], z)
                };
                lines.push(Line3D::new(coord(&pair[0]), coord(&pair[1]), id));
            }
        }
        return Some((lines, options));
    }

    let mut lines = Vec::new();
    match text.parse::<GeoJson>().ok()? {
        GeoJson::FeatureCollection(collection) => {
            for (index, feature) in collection.features.iter().enumerate() {
                if let Some(geometry) = &feature.geometry {
                    geojson_lines(&geometry.value, index as u32 + 1, &mut lines);
                }
            }
        }
        GeoJson::Feature(feature) => {
            if let Some(geometry) = &feature.geometry {
                geojson_lines(&geometry.value, 1, &mut lines);
            }
        }
        GeoJson::Geometry(geometry) => geojson_lines(&geometry.value, 1, &mut lines),
    }
    let options = PolygonizerOptions {
        node_input: node_geojson,
        ..PolygonizerOptions::default()
    };
    (!lines.is_empty()).then_some((lines, options))
}

/// The `circle` and `sawtooth` fixtures from `point_in_ring_empty_batch_skip`,
/// with the benchmark's probe sets, as reference points for the fractions.
fn calibration(edges: usize) -> [(&'static str, Tally); 2] {
    let circle: Vec<Coord3D> = (0..=edges)
        .map(|i| {
            let angle = 2.0 * PI * (i % edges) as f64 / edges as f64;
            Coord3D::new(100.0 * angle.cos(), 100.0 * angle.sin(), 0.0)
        })
        .collect();
    let circle = Ring::new(&circle);
    let mut circle_tally = Tally::default();
    for i in 0..1_024 {
        let angle = 2.0 * PI * i as f64 / 1_024.0;
        let radius = if i % 2 == 0 { 50.0 } else { 150.0 };
        circle_tally.add(circle.tally(radius * angle.sin()));
    }

    let teeth = edges - 3;
    let mut sawtooth = vec![Coord3D::new(0.0, -1.0, 0.0)];
    sawtooth.extend(
        (0..=teeth).map(|i| Coord3D::new(i as f64, if i % 2 == 0 { 0.0 } else { 2.0 }, 0.0)),
    );
    sawtooth.push(Coord3D::new(teeth as f64, -1.0, 0.0));
    sawtooth.push(sawtooth[0]);
    let sawtooth = Ring::new(&sawtooth);
    let mut sawtooth_tally = Tally::default();
    for i in 0..1_024 {
        sawtooth_tally.add(sawtooth.tally(0.25 + 1.5 * (i % 7) as f64 / 6.0));
    }

    [("circle", circle_tally), ("sawtooth", sawtooth_tally)]
}

fn print_row(label: &str, tally: Tally) {
    if tally.batches == 0 {
        println!(
            "{label:<34} {:>10} {:>12} {:>9} {:>11}",
            tally.queries, 0, "-", "-"
        );
        return;
    }
    println!(
        "{label:<34} {:>10} {:>12} {:>8.1}% {:>11.3}",
        tally.queries,
        tally.batches,
        100.0 * tally.active_batches as f64 / tally.batches as f64,
        tally.straddling_edges as f64 / tally.queries as f64,
    );
}

fn main() {
    let mut node_geojson = false;
    let args: Vec<PathBuf> = std::env::args()
        .skip(1)
        .filter(|arg| {
            let is_flag = arg == "--node";
            node_geojson |= is_flag;
            !is_flag
        })
        .map(PathBuf::from)
        .collect();
    if args.is_empty() {
        eprintln!("usage: ring_straddle_density [--node] PATH...");
        std::process::exit(2);
    }
    let mut inputs = Vec::new();
    for arg in &args {
        collect_inputs(arg, &mut inputs);
    }

    let mut total = Report::default();
    let mut used = 0;
    for path in &inputs {
        let Some((lines, options)) = load(path, node_geojson) else {
            continue;
        };
        let result = match polygonize(lines, &options) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("skip {}: {err}", path.display());
                continue;
            }
        };
        let shells: Vec<Ring> = result
            .polygons
            .iter()
            .map(|p| Ring::new(&p.exterior))
            .collect();
        let holes: Vec<Ring> = result
            .polygons
            .iter()
            .flat_map(|p| p.interiors.iter().map(|ring| Ring::new(ring)))
            .collect();
        let mut report = Report::default();
        measure(&shells, &holes, &mut report);
        let rings = shells.len() + holes.len();
        let max_len = shells
            .iter()
            .chain(&holes)
            .map(Ring::len)
            .max()
            .unwrap_or(0);
        println!(
            "{}: {} shells, {} holes, largest ring {} coords",
            path.display(),
            shells.len(),
            holes.len(),
            max_len
        );
        if rings > 0 {
            used += 1;
        }
        for b in 0..BUCKETS.len() {
            total.hole_assignment[b].add(report.hole_assignment[b]);
            total.interior_probe[b].add(report.interior_probe[b]);
        }
    }

    println!("\nInputs with output rings: {used} of {}", inputs.len());
    println!(
        "\n{:<34} {:>10} {:>12} {:>9} {:>11}",
        "family / ring coords", "queries", "batches", "active", "straddles"
    );
    for (family, tallies) in [
        ("hole_assignment", &total.hole_assignment),
        ("interior_probe", &total.interior_probe),
    ] {
        for (b, &(name, _, _)) in BUCKETS.iter().enumerate() {
            print_row(&format!("{family} / {name}"), tallies[b]);
        }
    }
    for edges in [32, 256, 1_024] {
        for (name, tally) in calibration(edges) {
            print_row(&format!("calibration {name} / {edges} edges"), tally);
        }
    }
    println!(
        "\nactive = full four-edge batches with at least one straddling edge.\n\
         straddles = straddling edges per query within full batches."
    );
}
