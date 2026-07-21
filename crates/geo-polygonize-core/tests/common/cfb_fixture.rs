use geo_polygonize_core::polygonize;
use geo_polygonize_core::PolygonizerOptions;
use geo_polygonize_core::{Coord3D, Line3D, Polygon3D};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfbFixture {
    pub case_id: String,
    pub options_profile: String,
    pub stride: u8,
    #[serde(default = "pass_status")]
    pub expected_status: String,
    pub lines: Vec<CfbLine>,
    pub expected: CfbExpected,
}

#[derive(Debug, Deserialize)]
pub struct CfbLine {
    pub id: u32,
    pub coords: Vec<Vec<f64>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CfbExpected {
    pub polygon_count: usize,
    pub total_area: f64,
    #[serde(default = "default_area_tolerance")]
    pub area_tolerance: f64,
    #[serde(default)]
    pub canonical_ring_hashes: Vec<String>,
    pub dangle_count: usize,
    pub cut_edge_count: usize,
    pub invalid_ring_count: usize,
    #[serde(default)]
    pub boundary_line_ids: Vec<u64>,
}

fn pass_status() -> String {
    "pass".to_string()
}

fn default_area_tolerance() -> f64 {
    1e-6
}

pub fn cfb_fixture_paths() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures")
        .join("cfb")
        .join("cases");

    let mut paths: Vec<_> = fs::read_dir(root)
        .expect("CFB fixture directory should exist")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    paths
}

pub fn read_cfb_fixture(path: &Path) -> CfbFixture {
    let content = fs::read_to_string(path).expect("failed to read CFB fixture");
    serde_json::from_str(&content).expect("failed to parse CFB fixture")
}

pub fn run_cfb_fixture(fixture: &CfbFixture) {
    let options = match fixture.options_profile.as_str() {
        "cfb_robust_v1" => PolygonizerOptions::cfb_robust_v1(),
        other => panic!("unsupported CFB options profile {other}"),
    };
    let lines = fixture_lines(fixture);

    let result = polygonize(lines.iter().copied(), &options)
        .unwrap_or_else(|err| panic!("{} failed: {err}", fixture.case_id));

    let check = || {
        assert_eq!(
            result.polygons.len(),
            fixture.expected.polygon_count,
            "{} polygon count",
            fixture.case_id
        );

        let total_area: f64 = result
            .polygons
            .iter()
            .map(Polygon3D::unsigned_area_2d)
            .sum();
        assert!(
            (total_area - fixture.expected.total_area).abs() <= fixture.expected.area_tolerance,
            "{} total area: expected {}, got {}",
            fixture.case_id,
            fixture.expected.total_area,
            total_area
        );

        assert_eq!(
            result.dangles.len(),
            fixture.expected.dangle_count,
            "{} dangle count",
            fixture.case_id
        );
        assert_eq!(
            result.cut_edges.len(),
            fixture.expected.cut_edge_count,
            "{} cut-edge count",
            fixture.case_id
        );
        assert_eq!(
            result.invalid_rings.len(),
            fixture.expected.invalid_ring_count,
            "{} invalid-ring count",
            fixture.case_id
        );
        assert_eq!(
            result.diagnostics.as_ref().unwrap().invalid_ring_count,
            fixture.expected.invalid_ring_count,
            "{} diagnostic invalid-ring count",
            fixture.case_id
        );

        if !fixture.expected.boundary_line_ids.is_empty() {
            let mut actual: Vec<u64> = result
                .polygons
                .iter()
                .filter_map(|poly| poly.provenance.as_ref())
                .flat_map(|prov| prov.boundary_line_ids.iter().copied())
                .collect();
            actual.sort_unstable();
            actual.dedup();

            assert_eq!(
                actual, fixture.expected.boundary_line_ids,
                "{} boundary line IDs",
                fixture.case_id
            );
        }

        if !fixture.expected.canonical_ring_hashes.is_empty() {
            let actual: Vec<String> = result
                .polygons
                .iter()
                .map(|poly| canonical_ring_hash(&poly.exterior))
                .collect();
            assert_eq!(
                actual, fixture.expected.canonical_ring_hashes,
                "{} canonical ring hashes",
                fixture.case_id
            );
        }
    };

    if fixture.expected_status == "xfail" {
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(check)).is_err();
        assert!(failed, "{} unexpectedly passed", fixture.case_id);
    } else {
        check();
    }
}

pub fn fixture_lines(fixture: &CfbFixture) -> Vec<Line3D> {
    fixture
        .lines
        .iter()
        .flat_map(|line| {
            line.coords.windows(2).map(|pair| {
                Line3D::new(
                    coord(&pair[0], fixture.stride),
                    coord(&pair[1], fixture.stride),
                    line.id,
                )
            })
        })
        .collect()
}

fn coord(raw: &[f64], stride: u8) -> Coord3D {
    let z = if stride == 3 { raw[2] } else { 0.0 };
    Coord3D::new(raw[0], raw[1], z)
}

fn canonical_ring_hash(ring: &[Coord3D]) -> String {
    ring.iter()
        .map(|c| format!("{:.6},{:.6},{:.6}", c.x, c.y, c.z))
        .collect::<Vec<_>>()
        .join("|")
}
