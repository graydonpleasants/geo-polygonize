#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use clap::{Parser, ValueEnum};
use geo_polygonize_core::{
    normalize_polygonize_error, polygonize, Coord3D, CoordinateFingerprintV1, Line3D,
    NodingGuarantee, NormalizedPolygonizeErrorV1, PolygonizerOptions, PolygonizerResult,
    PrecisionModel, TopologyFingerprintV1,
};
use geojson::{GeoJson, Value as GeoJsonValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Parser)]
#[command(about = "Emit one correctness-gated benchmark record")]
struct Args {
    #[arg(long, value_enum)]
    lane: Lane,
    #[arg(long)]
    workload: String,
    #[arg(long, default_value_t = 30)]
    samples: usize,
    #[arg(long, default_value_t = 5)]
    warmup_iterations: usize,
    #[arg(long)]
    repetition: Option<usize>,
    #[arg(long)]
    peak_rss_bytes: Option<u64>,
    #[arg(long)]
    reference_result: PathBuf,
    #[arg(long)]
    manifest: Option<PathBuf>,
    #[arg(long)]
    check_only: bool,
    #[arg(long)]
    check_only_output: Option<PathBuf>,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long)]
    mismatch_candidate: Option<PathBuf>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Lane {
    AlreadyNoded,
    Floating,
    CertifiedFixed,
}

impl Lane {
    fn profile(self) -> &'static str {
        match self {
            Self::AlreadyNoded => "already-noded",
            Self::Floating => "floating",
            Self::CertifiedFixed => "certified-fixed",
        }
    }

    fn record_name(self) -> &'static str {
        match self {
            Self::AlreadyNoded => "already-noded-polygonization",
            Self::Floating => "floating-noding-plus-polygonization",
            Self::CertifiedFixed => "certified-fixed-precision-noding-plus-polygonization",
        }
    }

    fn accepts(self, options: &PolygonizerOptions) -> bool {
        match self {
            Self::AlreadyNoded => !options.node_input,
            Self::Floating => {
                options.node_input && matches!(options.precision_model, PrecisionModel::Floating)
            }
            Self::CertifiedFixed => {
                options.node_input
                    && matches!(options.precision_model, PrecisionModel::FixedGrid { .. })
                    && matches!(
                        options.noding.guarantee,
                        NodingGuarantee::CertifiedFixedPrecision
                    )
            }
        }
    }

    fn validation_guarantee(self) -> NodingGuarantee {
        match self {
            Self::CertifiedFixed => NodingGuarantee::CertifiedFixedPrecision,
            Self::AlreadyNoded | Self::Floating => NodingGuarantee::Validate,
        }
    }
}

#[derive(Deserialize)]
struct Manifest {
    workloads: Vec<Workload>,
}

#[derive(Deserialize)]
struct Workload {
    id: String,
    compatibility_class: String,
    permitted_profiles: Vec<String>,
    artifact: Artifact,
    options: Vec<PolygonizerOptions>,
    size: WorkloadSize,
}

#[derive(Deserialize)]
struct Artifact {
    clip_path: String,
    sha256: String,
}

#[derive(Deserialize)]
struct WorkloadSize {
    line_strings: usize,
    segments: usize,
    coordinates: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceResult {
    schema_version: u32,
    workload_id: String,
    lane: String,
    implementation: ReferenceImplementation,
    fingerprint_sha256: String,
    topology: BenchmarkTopologyFingerprintV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReferenceImplementation {
    name: String,
    version: String,
    dependencies: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkTopologyFingerprintV1 {
    polygons: Vec<BenchmarkPolygonV1>,
    dangles: Vec<Vec<BenchmarkCoordinateV1>>,
    cut_edges: Vec<Vec<BenchmarkCoordinateV1>>,
    invalid_rings: Vec<Vec<BenchmarkCoordinateV1>>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkCoordinateV1 {
    x: String,
    y: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
struct BenchmarkPolygonV1 {
    exterior: Vec<BenchmarkCoordinateV1>,
    interiors: Vec<Vec<BenchmarkCoordinateV1>>,
}

/// The benchmark parity contract intentionally compares reduced XY topology.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "status", content = "value", rename_all = "snake_case")]
enum BenchmarkReducedOutcomeV1 {
    Success(BenchmarkTopologyFingerprintV1),
    Error(Box<BenchmarkFailureV1>),
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct BenchmarkFailureV1 {
    stage: String,
    error: NormalizedPolygonizeErrorV1,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct BenchmarkRunV1 {
    implementation: String,
    outcome: BenchmarkReducedOutcomeV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct BenchmarkInputLineV1 {
    start: CoordinateFingerprintV1,
    end: CoordinateFingerprintV1,
    line_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct BenchmarkMismatchCandidateV1 {
    schema_version: u32,
    producer: String,
    workload_id: String,
    lane: String,
    input: Vec<BenchmarkInputLineV1>,
    options: serde_json::Value,
    versions: BTreeMap<String, String>,
    baseline: BenchmarkRunV1,
    comparison: BenchmarkRunV1,
}

impl BenchmarkMismatchCandidateV1 {
    fn new(
        workload_id: &str,
        lane: Lane,
        input: &[Line3D],
        options: &PolygonizerOptions,
        versions: &BTreeMap<String, String>,
        baseline: BenchmarkRunV1,
        comparison: BenchmarkRunV1,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if baseline.implementation == comparison.implementation
            || baseline.outcome == comparison.outcome
        {
            return Err("benchmark mismatch candidates require distinct runs and outcomes".into());
        }
        Ok(Self {
            schema_version: 1,
            producer: "benchmark_record".to_string(),
            workload_id: workload_id.to_string(),
            lane: lane.record_name().to_string(),
            input: input
                .iter()
                .map(|line| BenchmarkInputLineV1 {
                    start: exact_coordinate(line.start),
                    end: exact_coordinate(line.end),
                    line_id: format!("0x{:08x}", line.line_id),
                })
                .collect(),
            options: serde_json::to_value(options)?,
            versions: versions.clone(),
            baseline,
            comparison,
        })
    }
}

#[derive(Default)]
struct Samples {
    elapsed: Vec<Duration>,
    ingest_and_node: Vec<Duration>,
    graph_build: Vec<Duration>,
    ring_extraction: Vec<Duration>,
    containment: Vec<Duration>,
    output_flatten: Vec<Duration>,
    allocations: u64,
    allocated_bytes: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if !args.check_only && args.samples == 0 {
        return Err("samples must be greater than zero".into());
    }
    if args.repetition == Some(0) {
        return Err("repetition must be greater than zero".into());
    }
    if !args.check_only && args.peak_rss_bytes.is_none() {
        return Err("peak RSS is required when recording timings".into());
    }
    if args.check_only_output.is_some() && !args.check_only {
        return Err("check-only output requires --check-only".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest_path = args
        .manifest
        .unwrap_or_else(|| root.join("tests/workloads/manifest-v1.json"));
    let manifest_dir = manifest_path
        .parent()
        .ok_or("manifest path has no parent directory")?
        .to_path_buf();
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(&manifest_path)?)?;
    let workload = manifest
        .workloads
        .into_iter()
        .find(|workload| workload.id == args.workload)
        .ok_or_else(|| format!("unknown workload {}", args.workload))?;
    if workload.compatibility_class != "parity"
        || !workload
            .permitted_profiles
            .iter()
            .any(|profile| profile == args.lane.profile())
    {
        return Err(format!(
            "{} is not a parity-class {} workload",
            workload.id,
            args.lane.profile()
        )
        .into());
    }

    let clip_path = manifest_dir.join(&workload.artifact.clip_path);
    verify_artifact_sha256(&clip_path, &workload.artifact.sha256)?;
    let lines = load_lines(&clip_path)?;
    if lines.len() != workload.size.segments {
        return Err(format!(
            "{} declares {} segments but contains {}",
            workload.id,
            workload.size.segments,
            lines.len()
        )
        .into());
    }
    let mut options = workload
        .options
        .iter()
        .find(|options| args.lane.accepts(options))
        .cloned()
        .ok_or_else(|| format!("workload has no {} options", args.lane.profile()))?;
    options.provenance.enabled = true;
    options.provenance.include_boundary_line_ids = true;

    let reference: ReferenceResult =
        serde_json::from_slice(&std::fs::read(&args.reference_result)?)?;
    validate_reference(&reference, &workload.id, args.lane)?;
    let reference_outcome = reduced_reference_outcome(&reference);
    let expected = parse_sha256(&reference.fingerprint_sha256)?;
    let reference_hash = benchmark_fingerprint_sha256(&reference.topology);
    if reference_hash != expected {
        return Err("reference result fingerprint does not match its topology payload".into());
    }
    let versions = dependencies(&reference)?;
    let write_candidate = |candidate_options: &PolygonizerOptions,
                           comparison: &BenchmarkReducedOutcomeV1|
     -> Result<(), Box<dyn std::error::Error>> {
        let candidate = BenchmarkMismatchCandidateV1::new(
            &workload.id,
            args.lane,
            &lines,
            candidate_options,
            &versions,
            BenchmarkRunV1 {
                implementation: reference.implementation.name.clone(),
                outcome: reference_outcome.clone(),
            },
            BenchmarkRunV1 {
                implementation: "geo-polygonize-core".to_string(),
                outcome: comparison.clone(),
            },
        )?;
        write_mismatch_candidate(args.mismatch_candidate.as_deref(), &candidate)
    };

    let mut validation_options = options.clone();
    validation_options.noding.guarantee = args.lane.validation_guarantee();
    let (validation_outcome, _) = reduced_rust_outcome(
        polygonize(lines.clone(), &validation_options),
        &validation_options,
        "validation",
    );
    if matches!(validation_outcome, BenchmarkReducedOutcomeV1::Error(_)) {
        write_candidate(&validation_options, &validation_outcome)?;
        return Err(format!(
            "benchmark validation failed: {}",
            serde_json::to_string(&validation_outcome)?
        )
        .into());
    }

    let mut correctness_options = options.clone();
    correctness_options.diagnostics.enabled = true;
    let (actual_outcome, correctness) = reduced_rust_outcome(
        polygonize(lines.clone(), &correctness_options),
        &correctness_options,
        "correctness",
    );
    let Some(correctness) = correctness else {
        write_candidate(&correctness_options, &actual_outcome)?;
        return Err(format!(
            "correctness gate failed: {}",
            serde_json::to_string(&actual_outcome)?
        )
        .into());
    };
    let BenchmarkReducedOutcomeV1::Success(actual_topology) = &actual_outcome else {
        unreachable!("successful correctness result has a success outcome");
    };
    let actual = benchmark_fingerprint_sha256(actual_topology);
    let BenchmarkReducedOutcomeV1::Success(reference_topology) = &reference_outcome else {
        unreachable!("validated reference result has a success outcome");
    };
    if actual_topology != reference_topology {
        write_candidate(&correctness_options, &actual_outcome)?;
        return Err(format!(
            "correctness gate failed: expected {}, observed {}",
            hex(&expected),
            hex(&actual),
        )
        .into());
    }
    if args.check_only {
        if let Some(path) = args.check_only_output.as_deref() {
            write_check_only_output(
                path,
                &workload,
                args.lane,
                &lines,
                &correctness,
                actual_topology,
            )?;
        }
        return Ok(());
    }

    let mut timed_options = options.clone();
    timed_options.diagnostics.timings = true;
    for _ in 0..args.warmup_iterations {
        polygonize(lines.clone(), &timed_options)?;
    }
    let profile_path = std::env::temp_dir().join(format!(
        "geo-polygonize-benchmark-{}.json",
        std::process::id()
    ));
    let _profiler = dhat::Profiler::builder().file_name(profile_path).build();
    let mut samples = Samples::default();
    for _ in 0..args.samples {
        let before = dhat::HeapStats::get();
        let started = Instant::now();
        let result = polygonize(lines.clone(), &timed_options)?;
        samples.elapsed.push(started.elapsed());
        let after = dhat::HeapStats::get();
        samples.allocations += after.total_blocks - before.total_blocks;
        samples.allocated_bytes += after.total_bytes - before.total_bytes;
        if benchmark_fingerprint_sha256(&benchmark_topology(&result, &options)?) != expected {
            return Err("timed sample fingerprint diverged after correctness gate".into());
        }
        let phase = &result
            .diagnostics
            .as_ref()
            .ok_or("timed sample omitted phase diagnostics")?
            .phase_times;
        samples.ingest_and_node.push(phase.ingest_and_node);
        samples.graph_build.push(phase.graph_build);
        samples.ring_extraction.push(phase.ring_extraction);
        samples.containment.push(phase.containment);
        samples.output_flatten.push(phase.output_flatten);
    }

    let diagnostics = correctness
        .diagnostics
        .as_ref()
        .ok_or("correctness run omitted diagnostics")?;
    let p50 = percentile(&samples.elapsed, 50);
    let p95 = percentile(&samples.elapsed, 95);
    let commit = command("git", &["rev-parse", "HEAD"])?;
    let output_coordinates = output_coordinates(&correctness);
    let record_id = format!("{}-{}-{}", workload.id, &commit[..12], args.lane.profile());
    let record = json!({
        "schema_version": 1,
        "record_id": args.repetition.map_or(record_id.clone(), |repetition| format!("{record_id}-r{repetition}")),
        "workload_id": workload.id,
        "artifact_sha256": workload.artifact.sha256,
        "lane": args.lane.record_name(),
        "implementation": {
            "name": "geo-polygonize-core",
            "version": env!("CARGO_PKG_VERSION"),
            "features": if cfg!(feature = "parallel") { vec!["parallel"] } else { Vec::<&str>::new() },
        },
        "correctness_gate": {
            "status": "passed",
            "validation": {"promised": true, "result": "passed"},
            "compatibility": {"expected": "parity", "observed": "equal"},
            "fingerprint": {
                "outcome": "equal",
                "actual_sha256": hex(&actual),
                "reference_sha256": hex(&expected),
            },
        },
        "topology": {
            "polygons": correctness.polygons.len(),
            "rings": correctness.polygons.iter().map(|polygon| 1 + polygon.interiors.len()).sum::<usize>(),
            "dangles": correctness.dangles.len(),
            "cut_edges": correctness.cut_edges.len(),
            "invalid_rings": correctness.invalid_rings.len(),
            "provenance_sources": provenance_sources(&correctness),
        },
        "measurement": {
            "p50_ms": milliseconds(p50),
            "p95_ms": milliseconds(p95),
            "throughput": {
                "value": if p50.is_zero() { 0.0 } else { lines.len() as f64 / p50.as_secs_f64() },
                "unit": "input-segments/second",
            },
            "samples": args.samples,
            "phase_times_ms": {
                "ingest_and_node": milliseconds(percentile(&samples.ingest_and_node, 50)),
                "graph_build": milliseconds(percentile(&samples.graph_build, 50)),
                "ring_extraction": milliseconds(percentile(&samples.ring_extraction, 50)),
                "containment": milliseconds(percentile(&samples.containment, 50)),
                "output_flatten": milliseconds(percentile(&samples.output_flatten, 50)),
            },
            "allocations": {
                "count": samples.allocations / args.samples as u64,
                "bytes": samples.allocated_bytes / args.samples as u64,
            },
            "peak_rss_bytes": args.peak_rss_bytes.expect("required before timing"),
        },
        "work": {
            "input_line_strings": workload.size.line_strings,
            "input_segments": lines.len(),
            "input_coordinates": workload.size.coordinates,
            "output_polygons": correctness.polygons.len(),
            "output_coordinates": output_coordinates,
            "candidate_pairs": diagnostics.noding_work_stats.candidate_pairs,
            "exact_predicate_calls": diagnostics.noding_work_stats.exact_intersection_calls,
            "split_events": diagnostics.noding_work_stats.split_events,
            "segment_expansion": {
                "input_segments": diagnostics.input_segment_count,
                "noded_segments": diagnostics.noded_segment_count,
                "ratio": diagnostics.noded_segment_count as f64 / diagnostics.input_segment_count.max(1) as f64,
            },
        },
        "environment": {
            "architecture": std::env::consts::ARCH,
            "os": {
                "name": command("uname", &["-s"])?,
                "version": command("uname", &["-r"])?,
            },
            "compiler": {
                "name": "rustc",
                "version": command("rustc", &["--version"])?,
            },
            "dependencies": versions,
            "commit_sha": commit,
        },
    });
    let bytes = serde_json::to_vec_pretty(&record)?;
    if let Some(path) = args.output {
        std::fs::write(path, bytes)?;
    } else {
        println!("{}", String::from_utf8(bytes)?);
    }
    Ok(())
}

fn load_lines(path: &Path) -> Result<Vec<Line3D>, Box<dyn std::error::Error>> {
    let geojson: GeoJson = std::fs::read_to_string(path)?.parse()?;
    let features = match geojson {
        GeoJson::FeatureCollection(collection) => collection.features,
        _ => return Err("workload clip must be a FeatureCollection".into()),
    };
    let mut line_strings = Vec::new();
    for feature in features {
        let geometry = feature.geometry.ok_or("workload feature has no geometry")?;
        match geometry.value {
            GeoJsonValue::LineString(line) => line_strings.push(line),
            GeoJsonValue::MultiLineString(lines) => line_strings.extend(lines),
            _ => return Err("workload geometry must contain line strings".into()),
        }
    }
    let mut segments = Vec::new();
    for (index, line) in line_strings.into_iter().enumerate() {
        let line_id = u32::try_from(index + 1)?;
        for pair in line.windows(2) {
            segments.push(Line3D::new(
                coordinate(&pair[0])?,
                coordinate(&pair[1])?,
                line_id,
            ));
        }
    }
    Ok(segments)
}

fn verify_artifact_sha256(path: &Path, expected: &str) -> Result<(), Box<dyn std::error::Error>> {
    let expected = parse_sha256(expected)?;
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let observed: [u8; 32] = digest.finalize().into();
    if observed != expected {
        return Err(format!(
            "workload artifact checksum mismatch for {}: expected {}, observed {}",
            path.display(),
            hex(&expected),
            hex(&observed),
        )
        .into());
    }
    Ok(())
}

fn coordinate(position: &[f64]) -> Result<Coord3D, Box<dyn std::error::Error>> {
    if position.len() < 2 {
        return Err("GeoJSON position must contain x and y".into());
    }
    Ok(Coord3D::new(
        position[0],
        position[1],
        position.get(2).copied().unwrap_or_default(),
    ))
}

fn benchmark_topology(
    result: &PolygonizerResult,
    options: &PolygonizerOptions,
) -> geo_polygonize_core::Result<BenchmarkTopologyFingerprintV1> {
    let fingerprint = TopologyFingerprintV1::try_from_result(result, options)?;
    let mut polygons: Vec<_> = fingerprint
        .polygons
        .into_iter()
        .map(|polygon| {
            let mut interiors: Vec<_> = polygon
                .interiors
                .into_iter()
                .map(|ring| canonical_benchmark_ring(xy(ring.coordinates)))
                .collect();
            interiors.sort();
            BenchmarkPolygonV1 {
                exterior: canonical_benchmark_ring(xy(polygon.exterior)),
                interiors,
            }
        })
        .collect();
    polygons.sort();
    Ok(BenchmarkTopologyFingerprintV1 {
        polygons,
        dangles: fingerprint.dangles.into_iter().map(xy).collect(),
        cut_edges: fingerprint.cut_edges.into_iter().map(xy).collect(),
        invalid_rings: fingerprint.invalid_rings.into_iter().map(xy).collect(),
    })
}

fn reduced_rust_outcome(
    result: geo_polygonize_core::Result<PolygonizerResult>,
    options: &PolygonizerOptions,
    stage: &str,
) -> (BenchmarkReducedOutcomeV1, Option<PolygonizerResult>) {
    match result {
        Ok(result) => match benchmark_topology(&result, options) {
            Ok(topology) => (BenchmarkReducedOutcomeV1::Success(topology), Some(result)),
            Err(error) => (
                BenchmarkReducedOutcomeV1::Error(Box::new(BenchmarkFailureV1 {
                    stage: format!("{stage}_fingerprint"),
                    error: normalize_polygonize_error(&error),
                })),
                None,
            ),
        },
        Err(error) => (
            BenchmarkReducedOutcomeV1::Error(Box::new(BenchmarkFailureV1 {
                stage: stage.to_string(),
                error: normalize_polygonize_error(&error),
            })),
            None,
        ),
    }
}

fn reduced_reference_outcome(reference: &ReferenceResult) -> BenchmarkReducedOutcomeV1 {
    BenchmarkReducedOutcomeV1::Success(reference.topology.clone())
}

fn exact_coordinate(coordinate: Coord3D) -> CoordinateFingerprintV1 {
    let bits = |value: f64| format!("0x{:016x}", value.to_bits());
    CoordinateFingerprintV1 {
        x: bits(coordinate.x),
        y: bits(coordinate.y),
        z: bits(coordinate.z),
    }
}

fn write_mismatch_candidate(
    path: Option<&Path>,
    candidate: &BenchmarkMismatchCandidateV1,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = path else {
        return Ok(());
    };
    let mut output = OpenOptions::new().write(true).create_new(true).open(path)?;
    serde_json::to_writer_pretty(&mut output, candidate)?;
    writeln!(output)?;
    Ok(())
}

fn xy(
    coordinates: Vec<geo_polygonize_core::CoordinateFingerprintV1>,
) -> Vec<BenchmarkCoordinateV1> {
    coordinates
        .into_iter()
        .map(|coordinate| BenchmarkCoordinateV1 {
            x: coordinate.x,
            y: coordinate.y,
        })
        .collect()
}

fn canonical_benchmark_ring(mut ring: Vec<BenchmarkCoordinateV1>) -> Vec<BenchmarkCoordinateV1> {
    if ring.len() > 1 && ring.first() == ring.last() {
        ring.pop();
    }
    if ring.is_empty() {
        return ring;
    }
    let forward = rotate_benchmark_ring(&ring);
    ring.reverse();
    let backward = rotate_benchmark_ring(&ring);
    let mut result = forward.min(backward);
    result.push(result[0].clone());
    result
}

fn rotate_benchmark_ring(ring: &[BenchmarkCoordinateV1]) -> Vec<BenchmarkCoordinateV1> {
    let size = ring.len();
    let (mut left, mut right, mut offset) = (0, 1, 0);
    while left < size && right < size && offset < size {
        match ring[(left + offset) % size].cmp(&ring[(right + offset) % size]) {
            std::cmp::Ordering::Equal => offset += 1,
            std::cmp::Ordering::Less => {
                right += offset + 1;
                if right == left {
                    right += 1;
                }
                offset = 0;
            }
            std::cmp::Ordering::Greater => {
                left += offset + 1;
                if left == right {
                    left += 1;
                }
                offset = 0;
            }
        }
    }
    let start = left.min(right);
    ring[start..]
        .iter()
        .chain(&ring[..start])
        .cloned()
        .collect()
}

fn benchmark_fingerprint_sha256(topology: &BenchmarkTopologyFingerprintV1) -> [u8; 32] {
    Sha256::digest(serde_json::to_vec(topology).expect("benchmark topology serializes")).into()
}

fn parse_sha256(value: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    if value.len() != 64 {
        return Err("SHA-256 must contain 64 lowercase hex digits".into());
    }
    let mut result = [0; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
    }
    if hex(&result) != value {
        return Err("SHA-256 must use lowercase hex".into());
    }
    Ok(result)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn percentile(samples: &[Duration], percentile: usize) -> Duration {
    let mut samples = samples.to_vec();
    samples.sort_unstable();
    samples[(samples.len() * percentile).div_ceil(100).saturating_sub(1)]
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn provenance_sources(result: &PolygonizerResult) -> usize {
    result
        .polygons
        .iter()
        .filter_map(|polygon| polygon.provenance.as_ref())
        .flat_map(|provenance| &provenance.boundary_line_ids)
        .copied()
        .collect::<BTreeSet<_>>()
        .len()
}

fn output_coordinates(result: &PolygonizerResult) -> usize {
    let polygon_coordinates = result.polygons.iter().map(|polygon| {
        polygon.exterior.len() + polygon.interiors.iter().map(Vec::len).sum::<usize>()
    });
    polygon_coordinates
        .chain(result.dangles.iter().map(Vec::len))
        .chain(result.cut_edges.iter().map(Vec::len))
        .chain(result.invalid_rings.iter().map(Vec::len))
        .sum()
}

fn write_check_only_output(
    path: &Path,
    workload: &Workload,
    lane: Lane,
    lines: &[Line3D],
    result: &PolygonizerResult,
    topology: &BenchmarkTopologyFingerprintV1,
) -> Result<(), Box<dyn std::error::Error>> {
    let diagnostics = result
        .diagnostics
        .as_ref()
        .ok_or("check-only output requires diagnostics")?;
    let input_segments = lines.len();
    let output = json!({
        "schema_version": 1,
        "workload_id": workload.id,
        "lane": lane.record_name(),
        "status": "passed",
        "fingerprint_sha256": hex(&benchmark_fingerprint_sha256(topology)),
        "topology": {
            "polygons": result.polygons.len(),
            "rings": result.polygons.iter().map(|polygon| 1 + polygon.interiors.len()).sum::<usize>(),
            "dangles": result.dangles.len(),
            "cut_edges": result.cut_edges.len(),
            "invalid_rings": result.invalid_rings.len(),
            "provenance_sources": provenance_sources(result),
        },
        "work": {
            "input_line_strings": workload.size.line_strings,
            "input_segments": input_segments,
            "input_coordinates": workload.size.coordinates,
            "output_coordinates": output_coordinates(result),
            "candidate_pairs": diagnostics.noding_work_stats.candidate_pairs,
            "exact_predicate_calls": diagnostics.noding_work_stats.exact_intersection_calls,
            "split_events": diagnostics.noding_work_stats.split_events,
            "segment_expansion": {
                "input_segments": diagnostics.input_segment_count,
                "noded_segments": diagnostics.noded_segment_count,
                "ratio": diagnostics.noded_segment_count as f64 / diagnostics.input_segment_count.max(1) as f64,
            },
        },
    });
    std::fs::write(path, serde_json::to_vec_pretty(&output)?)?;
    Ok(())
}

fn dependencies(
    reference: &ReferenceResult,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut dependencies = BTreeMap::from([(
        "geo-polygonize-core".to_string(),
        env!("CARGO_PKG_VERSION").to_string(),
    )]);
    if dependencies
        .insert(
            reference.implementation.name.clone(),
            reference.implementation.version.clone(),
        )
        .is_some()
    {
        return Err("reference implementation duplicates a dependency name".into());
    }
    if reference.implementation.dependencies.is_empty() {
        return Err("reference dependencies are required".into());
    }
    for (name, version) in &reference.implementation.dependencies {
        if name.is_empty()
            || version.is_empty()
            || dependencies.insert(name.clone(), version.clone()).is_some()
        {
            return Err(format!("invalid or duplicate reference dependency {name}").into());
        }
    }
    Ok(dependencies)
}

fn validate_reference(
    reference: &ReferenceResult,
    workload_id: &str,
    lane: Lane,
) -> Result<(), Box<dyn std::error::Error>> {
    if reference.schema_version != 1 {
        return Err(format!(
            "unsupported reference result schema {}",
            reference.schema_version
        )
        .into());
    }
    if reference.workload_id != workload_id {
        return Err(format!(
            "reference workload {} does not match {workload_id}",
            reference.workload_id
        )
        .into());
    }
    if reference.lane != lane.record_name() {
        return Err(format!(
            "reference lane {} does not match {}",
            reference.lane,
            lane.record_name()
        )
        .into());
    }
    if reference.implementation.name.is_empty() || reference.implementation.version.is_empty() {
        return Err("reference implementation name and version are required".into());
    }
    Ok(())
}

fn command(program: &str, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(format!("{program} exited with {}", output.status).into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_uses_nearest_rank() {
        let values = [
            Duration::from_millis(4),
            Duration::from_millis(1),
            Duration::from_millis(3),
            Duration::from_millis(2),
        ];
        assert_eq!(percentile(&values, 50), Duration::from_millis(2));
        assert_eq!(percentile(&values, 95), Duration::from_millis(4));
    }

    #[test]
    fn sha256_parser_is_strict() {
        let hash = "00".repeat(32);
        assert_eq!(parse_sha256(&hash).unwrap(), [0; 32]);
        assert!(parse_sha256(&"AA".repeat(32)).is_err());
    }

    #[test]
    fn workload_artifact_sha256_is_verified() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let clip = root.join("tests/workloads/clips/network-linework.geojson");
        let expected = "93d10a61d937c2fd2ff4cdc2a584589050de946b4f53a29b136a8a18ff9515fb";
        assert!(verify_artifact_sha256(&clip, expected).is_ok());

        let error = verify_artifact_sha256(&clip, &"00".repeat(32)).unwrap_err();
        assert!(error
            .to_string()
            .contains("workload artifact checksum mismatch"));
    }

    #[test]
    fn benchmark_fingerprint_omits_rust_only_edge_identity() {
        let mut options = PolygonizerOptions::default();
        options.provenance.enabled = true;
        options.provenance.include_boundary_line_ids = true;
        let first = polygonize(
            [
                Line3D::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(1.0, 0.0, 0.0), 1),
                Line3D::new(Coord3D::new(1.0, 0.0, 0.0), Coord3D::new(0.0, 1.0, 0.0), 2),
                Line3D::new(Coord3D::new(0.0, 1.0, 0.0), Coord3D::new(0.0, 0.0, 0.0), 3),
            ],
            &options,
        )
        .unwrap();
        let second = polygonize(
            [
                Line3D::new(Coord3D::new(0.0, 0.0, 9.0), Coord3D::new(1.0, 0.0, 9.0), 10),
                Line3D::new(Coord3D::new(1.0, 0.0, 9.0), Coord3D::new(0.0, 1.0, 9.0), 20),
                Line3D::new(Coord3D::new(0.0, 1.0, 9.0), Coord3D::new(0.0, 0.0, 9.0), 30),
            ],
            &options,
        )
        .unwrap();
        assert_eq!(
            benchmark_topology(&first, &options).unwrap(),
            benchmark_topology(&second, &options).unwrap()
        );
    }

    #[test]
    fn reduced_outcomes_keep_xy_success_and_structured_rust_errors_separate() {
        let options = PolygonizerOptions::default();
        let (success, result) = reduced_rust_outcome(
            polygonize(Vec::<Line3D>::new(), &options),
            &options,
            "correctness",
        );
        let failure = reduced_rust_outcome(
            Err(geo_polygonize_core::PolygonizeError::InvalidArgumentType {
                field: "example".to_string(),
                expected: "valid".to_string(),
                actual: "invalid".to_string(),
            }),
            &options,
            "validation",
        )
        .0;
        let success = serde_json::to_value(success).unwrap();
        let failure = serde_json::to_value(failure).unwrap();

        assert!(result.is_some());
        assert_eq!(success["status"], "success");
        assert!(success["value"]["polygons"].is_array());
        assert!(success["value"].get("options").is_none());
        assert_eq!(failure["status"], "error");
        assert_eq!(failure["value"]["stage"], "validation");
        assert_eq!(failure["value"]["error"]["family"], "invalid_argument");
    }

    #[test]
    fn mismatch_candidates_keep_exact_input_and_reduced_outcomes() {
        let mut options = PolygonizerOptions::default();
        options.diagnostics.enabled = true;
        let lines = [Line3D::new(
            Coord3D::new(-0.0, 1.0, 10.0),
            Coord3D::new(2.0, 3.0, 11.0),
            7,
        )];
        let baseline = BenchmarkRunV1 {
            implementation: "shapely".to_string(),
            outcome: reduced_rust_outcome(
                polygonize(Vec::<Line3D>::new(), &options),
                &options,
                "reference",
            )
            .0,
        };
        let comparison = BenchmarkRunV1 {
            implementation: "geo-polygonize-core".to_string(),
            outcome: reduced_rust_outcome(
                Err(geo_polygonize_core::PolygonizeError::InvalidArgumentType {
                    field: "example".to_string(),
                    expected: "valid".to_string(),
                    actual: "invalid".to_string(),
                }),
                &options,
                "correctness",
            )
            .0,
        };
        let versions = BTreeMap::from([
            (
                "geo-polygonize-core".to_string(),
                env!("CARGO_PKG_VERSION").to_string(),
            ),
            ("shapely".to_string(), "2.1.2".to_string()),
        ]);
        let candidate = BenchmarkMismatchCandidateV1::new(
            "example-workload",
            Lane::Floating,
            &lines,
            &options,
            &versions,
            baseline.clone(),
            comparison,
        )
        .unwrap();
        let json = serde_json::to_value(candidate).unwrap();

        assert_eq!(json["producer"], "benchmark_record");
        assert_eq!(json["input"][0]["start"]["x"], "0x8000000000000000");
        assert_eq!(json["input"][0]["start"]["z"], "0x4024000000000000");
        assert_eq!(json["input"][0]["line_id"], "0x00000007");
        assert_eq!(json["baseline"]["outcome"]["status"], "success");
        assert!(json["baseline"]["outcome"]["value"]
            .get("schema_version")
            .is_none());
        assert_eq!(json["comparison"]["outcome"]["status"], "error");
        assert!(json.get("case_id").is_none());
        assert!(json.get("classification").is_none());
        assert!(BenchmarkMismatchCandidateV1::new(
            "example-workload",
            Lane::Floating,
            &lines,
            &options,
            &versions,
            baseline.clone(),
            baseline,
        )
        .is_err());
    }

    #[test]
    fn lanes_select_only_equivalent_options() {
        let mut options = PolygonizerOptions::default();
        assert!(Lane::AlreadyNoded.accepts(&options));
        assert!(!Lane::Floating.accepts(&options));
        options.node_input = true;
        assert!(!Lane::AlreadyNoded.accepts(&options));
        assert!(Lane::Floating.accepts(&options));
        options.precision_model = PrecisionModel::FixedGrid { grid_size: 1.0 };
        assert!(!Lane::Floating.accepts(&options));
        assert!(!Lane::CertifiedFixed.accepts(&options));
        options.noding.guarantee = NodingGuarantee::CertifiedFixedPrecision;
        assert!(Lane::CertifiedFixed.accepts(&options));
        assert!(matches!(
            Lane::CertifiedFixed.validation_guarantee(),
            NodingGuarantee::CertifiedFixedPrecision
        ));
    }
}
