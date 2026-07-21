use geo_polygonize_core::DeterminismOptions;
use geo_polygonize_core::Polygonizer;
use geo_polygonize_core::{Coord3D, Line3D};
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
struct GoldenPolygon {
    exterior: Vec<GoldenCoord>,
    interiors: Vec<Vec<GoldenCoord>>,
    exterior_ids: Vec<u32>,
    interiors_ids: Vec<Vec<u32>>,
    boundary_line_ids: Vec<u64>,
    input_profile_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct GoldenResult {
    polygons: Vec<GoldenPolygon>,
    dangles: Vec<Vec<GoldenCoord>>,
    cut_edges: Vec<Vec<GoldenCoord>>,
    invalid_rings: Vec<Vec<GoldenCoord>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoldenFixture {
    name: String,
    inputs: Vec<GoldenLine>,
    expected: GoldenResult,
}

fn run_golden_test(path: &Path) {
    let content = fs::read_to_string(path).expect("Failed to read fixture file");
    let fixture: GoldenFixture = serde_json::from_str(&content).expect("Failed to parse fixture");

    let input_lines: Vec<Line3D> = fixture.inputs.iter().map(|l| l.into()).collect();

    let mut poly = Polygonizer::new();
    poly.options_mut().node_input = true;
    poly.options_mut().determinism = DeterminismOptions {
        canonical_sort: true,
        canonical_ring_rotation: true,
        stable_tie_breaks: true,
    };
    poly.options_mut().provenance.enabled = true;
    poly.options_mut().provenance.include_boundary_line_ids = true;
    poly.add_lines(input_lines);

    let res = poly.polygonize().unwrap();

    let actual_result = GoldenResult {
        polygons: res
            .polygons
            .into_iter()
            .map(|p| {
                let provenance = p.provenance.unwrap_or_default();
                GoldenPolygon {
                    exterior: p.exterior.into_iter().map(|c| c.into()).collect(),
                    interiors: p
                        .interiors
                        .into_iter()
                        .map(|h| h.into_iter().map(|c| c.into()).collect())
                        .collect(),
                    exterior_ids: p.exterior_ids,
                    interiors_ids: p.interiors_ids,
                    boundary_line_ids: provenance.boundary_line_ids,
                    input_profile_id: provenance.input_profile_id,
                }
            })
            .collect(),
        dangles: res
            .dangles
            .into_iter()
            .map(|d| d.into_iter().map(|c| c.into()).collect())
            .collect(),
        cut_edges: res
            .cut_edges
            .into_iter()
            .map(|edge| edge.into_iter().map(|c| c.into()).collect())
            .collect(),
        invalid_rings: res
            .invalid_rings
            .into_iter()
            .map(|r| r.into_iter().map(|c| c.into()).collect())
            .collect(),
    };

    assert_eq!(
        actual_result, fixture.expected,
        "Mismatch in fixture {}",
        fixture.name
    );
}

#[test]
fn test_all_fixtures() {
    let base_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    if !base_dir.exists() {
        return;
    }

    let mut count = 0;
    for entry in walkdir::WalkDir::new(base_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let is_json = entry
            .path()
            .extension()
            .map(|s| s == "json")
            .unwrap_or(false);
        let is_provenance = entry
            .path()
            .components()
            .any(|c| c.as_os_str() == "provenance");

        if is_json && !is_provenance {
            run_golden_test(entry.path());
            count += 1;
        }
    }
    println!("Ran {} golden fixtures", count);
}
