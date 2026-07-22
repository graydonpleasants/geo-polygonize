use geo_polygonize_core::{polygonize, Coord3D, Line3D, PolygonizerOptions};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct CompatibilityCase {
    case_id: String,
    fixture: PathBuf,
    classification: Classification,
    shapely_reference: ShapelyReference,
    rust_profiles: Vec<RustProfile>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Classification {
    ExpectedParity,
    ExpectedDivergence,
    InvalidAmbiguous,
}

#[derive(Debug, Deserialize)]
struct ShapelyReference {
    polygon_count: usize,
    total_area: f64,
    area_tolerance: f64,
}

#[derive(Debug, Deserialize)]
struct RustProfile {
    name: String,
    options: PolygonizerOptions,
}

#[derive(Debug, Deserialize)]
struct GoldenFixture {
    inputs: Vec<GoldenLine>,
    expected_metrics: ExpectedMetrics,
}

#[derive(Debug, Deserialize)]
struct GoldenLine {
    start: GoldenCoord,
    end: GoldenCoord,
    id: u32,
}

#[derive(Debug, Deserialize)]
struct GoldenCoord {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Debug, Deserialize)]
struct ExpectedMetrics {
    polygon_count: usize,
    total_area: f64,
    area_tolerance: f64,
    dangle_count: usize,
    cut_edge_count: usize,
    invalid_ring_count: usize,
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn compatibility_corpus_matches_recorded_contracts() {
    let root = repository_root();
    let mut seen_classifications = [false; 3];
    let mut manifests: Vec<_> = fs::read_dir(root.join("fixtures/compat"))
        .expect("compatibility fixture directory should exist")
        .map(|entry| {
            entry
                .expect("compatibility fixture should be readable")
                .path()
        })
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    manifests.sort();
    assert!(!manifests.is_empty(), "expected compatibility fixtures");

    for manifest in manifests {
        let case: CompatibilityCase = serde_json::from_str(
            &fs::read_to_string(&manifest).expect("compatibility fixture should be readable"),
        )
        .expect("compatibility fixture should parse");
        seen_classifications[match case.classification {
            Classification::ExpectedParity => 0,
            Classification::ExpectedDivergence => 1,
            Classification::InvalidAmbiguous => 2,
        }] = true;
        let golden: GoldenFixture = serde_json::from_str(
            &fs::read_to_string(root.join(&case.fixture))
                .expect("golden fixture should be readable"),
        )
        .expect("golden fixture should parse");
        let lines: Vec<_> = golden
            .inputs
            .into_iter()
            .map(|line| {
                Line3D::new(
                    Coord3D::new(line.start.x, line.start.y, line.start.z),
                    Coord3D::new(line.end.x, line.end.y, line.end.z),
                    line.id,
                )
            })
            .collect();

        if matches!(case.classification, Classification::ExpectedParity) {
            assert_eq!(
                case.shapely_reference.polygon_count, golden.expected_metrics.polygon_count,
                "{} Shapely polygon count",
                case.case_id
            );
            assert!(
                (case.shapely_reference.total_area - golden.expected_metrics.total_area).abs()
                    <= case.shapely_reference.area_tolerance,
                "{} Shapely total area",
                case.case_id
            );
        }

        for profile in case.rust_profiles {
            let result = polygonize(lines.iter().copied(), &profile.options)
                .unwrap_or_else(|error| panic!("{} / {}: {error}", case.case_id, profile.name));
            let total_area: f64 = result
                .polygons
                .iter()
                .map(|polygon| polygon.unsigned_area_2d())
                .sum();
            assert_eq!(
                [
                    result.polygons.len(),
                    result.dangles.len(),
                    result.cut_edges.len(),
                    result.invalid_rings.len(),
                ],
                [
                    golden.expected_metrics.polygon_count,
                    golden.expected_metrics.dangle_count,
                    golden.expected_metrics.cut_edge_count,
                    golden.expected_metrics.invalid_ring_count,
                ],
                "{} / {} topology counts",
                case.case_id,
                profile.name
            );
            assert!(
                (total_area - golden.expected_metrics.total_area).abs()
                    <= golden.expected_metrics.area_tolerance,
                "{} / {} total area",
                case.case_id,
                profile.name
            );
        }
    }

    assert_eq!(
        seen_classifications, [true; 3],
        "expected parity, divergence, and invalid/ambiguous cases"
    );
}
