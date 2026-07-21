use geo_polygonize_core::polygonize;
use geo_polygonize_core::{Coord3D, Line3D};
use geo_polygonize_core::{DeterminismOptions, PolygonizerOptions, ProvenanceOptions};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct GoldenCoord {
    x: f64,
    y: f64,
    z: f64,
}

impl From<Coord3D> for GoldenCoord {
    fn from(c: Coord3D) -> Self {
        Self {
            x: c.x,
            y: c.y,
            z: c.z,
        }
    }
}

impl From<&GoldenCoord> for Coord3D {
    fn from(c: &GoldenCoord) -> Self {
        Self::new(c.x, c.y, c.z)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct GoldenLine {
    start: GoldenCoord,
    end: GoldenCoord,
    id: u32,
}

impl From<Line3D> for GoldenLine {
    fn from(l: Line3D) -> Self {
        Self {
            start: l.start.into(),
            end: l.end.into(),
            id: l.line_id,
        }
    }
}

impl From<&GoldenLine> for Line3D {
    fn from(l: &GoldenLine) -> Self {
        Self::new((&l.start).into(), (&l.end).into(), l.id)
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct ProvenanceFixturePolygon {
    exterior_ids: Vec<u32>,
    interiors_ids: Vec<Vec<u32>>,
    provenance_line_ids: Option<Vec<u64>>,
    provenance_profile_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct ProvenanceFixtureResult {
    polygons: Vec<ProvenanceFixturePolygon>,
}

#[derive(Serialize, Deserialize, Debug)]
struct ProvenanceFixture {
    name: String,
    profile_id: Option<String>,
    inputs: Vec<GoldenLine>,
    expected: Option<ProvenanceFixtureResult>,
}

fn run_provenance_test(path: &Path) {
    let content = fs::read_to_string(path).expect("Failed to read fixture file");
    let fixture: ProvenanceFixture =
        serde_json::from_str(&content).expect("Failed to parse fixture");

    let input_lines: Vec<Line3D> = fixture.inputs.iter().map(|l| l.into()).collect();

    let options = PolygonizerOptions {
        node_input: true,
        determinism: DeterminismOptions {
            canonical_sort: true,
            canonical_ring_rotation: true,
            stable_tie_breaks: true,
        },
        provenance: ProvenanceOptions {
            enabled: true,
            include_boundary_line_ids: true,
        },
        input_profile_id: fixture.profile_id.clone(),
        ..PolygonizerOptions::default()
    };

    let res = polygonize(input_lines.iter().copied(), &options).unwrap();

    let actual_result = ProvenanceFixtureResult {
        polygons: res
            .polygons
            .into_iter()
            .map(|p| ProvenanceFixturePolygon {
                exterior_ids: p.exterior_ids,
                interiors_ids: p.interiors_ids,
                provenance_line_ids: p
                    .provenance
                    .as_ref()
                    .map(|prov| prov.boundary_line_ids.clone()),
                provenance_profile_id: p
                    .provenance
                    .as_ref()
                    .and_then(|prov| prov.input_profile_id.clone()),
            })
            .collect(),
    };

    let expected = fixture
        .expected
        .as_ref()
        .unwrap_or_else(|| panic!("Missing mandatory expected output for {}", fixture.name));
    assert_eq!(
        &actual_result, expected,
        "Mismatch in fixture {}",
        fixture.name
    );
}

#[test]
fn collinear_overlap_preserves_every_boundary_source() {
    let coord = |x, y| Coord3D::new(x, y, 0.0);
    let lines = vec![
        Line3D::new(coord(0.0, 0.0), coord(10.0, 0.0), 100),
        Line3D::new(coord(10.0, 0.0), coord(10.0, 10.0), 101),
        Line3D::new(coord(10.0, 10.0), coord(0.0, 10.0), 102),
        Line3D::new(coord(0.0, 10.0), coord(0.0, 0.0), 103),
        Line3D::new(coord(2.0, 0.0), coord(8.0, 0.0), 200),
    ];
    let options = PolygonizerOptions {
        node_input: true,
        provenance: ProvenanceOptions {
            enabled: true,
            include_boundary_line_ids: true,
        },
        ..PolygonizerOptions::default()
    };

    let result = polygonize(lines, &options).unwrap();
    assert_eq!(result.polygons.len(), 1);
    assert_eq!(
        result.polygons[0]
            .provenance
            .as_ref()
            .unwrap()
            .boundary_line_ids,
        vec![100, 101, 102, 103, 200]
    );
}

#[test]
fn test_all_provenance_fixtures() {
    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("provenance");
    if !base_dir.exists() {
        return;
    }

    let mut count = 0;
    for entry in walkdir::WalkDir::new(base_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry
            .path()
            .extension()
            .map(|s| s == "json")
            .unwrap_or(false)
        {
            run_provenance_test(entry.path());
            count += 1;
        }
    }
    println!("Ran {} provenance fixtures", count);
}
