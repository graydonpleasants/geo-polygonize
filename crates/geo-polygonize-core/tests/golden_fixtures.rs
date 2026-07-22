use geo_polygonize_core::{
    polygonize, Coord3D, DedupPolicy, DeterminismOptions, Line3D, PolygonizerOptions,
    ProvenanceOptions, TileOwnershipPolicy, TiledPolygonizer,
};
use geo_types::{Coord, Geometry, LineString, Rect};
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
    options: PolygonizerOptions,
    tiling: Option<GoldenTiling>,
    inputs: Vec<GoldenLine>,
    expected_metrics: GoldenMetrics,
    expected: GoldenResult,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoldenTiling {
    bbox: [f64; 4],
    tile_size: f64,
    buffer: f64,
    ownership_policy: TileOwnershipPolicy,
    dedup_policy: DedupPolicy,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoldenMetrics {
    polygon_count: usize,
    total_area: f64,
    area_tolerance: f64,
    dangle_count: usize,
    cut_edge_count: usize,
    invalid_ring_count: usize,
}

fn run_golden_test(path: &Path) {
    let content = fs::read_to_string(path).expect("Failed to read fixture file");
    let fixture: GoldenFixture = serde_json::from_str(&content).expect("Failed to parse fixture");

    let input_lines: Vec<Line3D> = fixture.inputs.iter().map(|l| l.into()).collect();
    let mut options = fixture.options;
    options.determinism = DeterminismOptions {
        canonical_sort: true,
        canonical_ring_rotation: true,
        stable_tie_breaks: true,
    };
    options.provenance = ProvenanceOptions {
        enabled: true,
        include_boundary_line_ids: true,
    };

    let (polygons, dangles, cut_edges, invalid_rings) = if let Some(tiling) = fixture.tiling {
        let geometries: Vec<_> = input_lines
            .iter()
            .map(|line| {
                Geometry::LineString(LineString::new(vec![
                    line.start.to_coord_2d(),
                    line.end.to_coord_2d(),
                ]))
            })
            .collect();
        let mut polygonizer = TiledPolygonizer::new(
            Rect::new(
                Coord {
                    x: tiling.bbox[0],
                    y: tiling.bbox[1],
                },
                Coord {
                    x: tiling.bbox[2],
                    y: tiling.bbox[3],
                },
            ),
            tiling.tile_size,
        )
        .with_buffer(tiling.buffer)
        .with_options(options)
        .with_ownership_policy(tiling.ownership_policy)
        .with_dedup_policy(tiling.dedup_policy);
        for geometry in &geometries {
            polygonizer.add_geometry(geometry);
        }
        let result = polygonizer.polygonize().unwrap();
        (result.polygons, Vec::new(), Vec::new(), Vec::new())
    } else {
        let result = polygonize(input_lines, &options).unwrap();
        (
            result.polygons,
            result.dangles,
            result.cut_edges,
            result.invalid_rings,
        )
    };
    let actual_area: f64 = polygons
        .iter()
        .map(|polygon| polygon.unsigned_area_2d())
        .sum();
    assert_eq!(
        [
            polygons.len(),
            dangles.len(),
            cut_edges.len(),
            invalid_rings.len(),
        ],
        [
            fixture.expected_metrics.polygon_count,
            fixture.expected_metrics.dangle_count,
            fixture.expected_metrics.cut_edge_count,
            fixture.expected_metrics.invalid_ring_count,
        ],
        "{} counts [polygons, dangles, cut edges, invalid rings]",
        fixture.name
    );
    assert!(
        (actual_area - fixture.expected_metrics.total_area).abs()
            <= fixture.expected_metrics.area_tolerance,
        "{} total area: expected {}, got {}",
        fixture.name,
        fixture.expected_metrics.total_area,
        actual_area
    );
    let actual_result = GoldenResult {
        polygons: polygons
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
        dangles: dangles
            .into_iter()
            .map(|d| d.into_iter().map(|c| c.into()).collect())
            .collect(),
        cut_edges: cut_edges
            .into_iter()
            .map(|edge| edge.into_iter().map(|c| c.into()).collect())
            .collect(),
        invalid_rings: invalid_rings
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
    let mut count = 0;
    for entry in walkdir::WalkDir::new(base_dir) {
        let entry = entry.expect("Failed to discover golden fixtures");
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
    assert!(count > 0, "No golden fixtures found");
    println!("Ran {} golden fixtures", count);
}
