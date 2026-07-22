use flatgeobuf::{
    FallibleStreamingIterator, FgbCrs, FgbReader, FgbWriter, FgbWriterOptions, GeometryType,
};
use geo_polygonize_core::PolygonizerOptions;
use geo_polygonize_flatgeobuf::polygonize_flatgeobuf_file;
use geo_traits::to_geo::ToGeoGeometry;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};

#[test]
fn polygonizes_the_shared_conformance_fixture() {
    let input_path =
        std::env::temp_dir().join(format!("geo-polygonize-{}-input.fgb", std::process::id()));
    let output_path =
        std::env::temp_dir().join(format!("geo-polygonize-{}-output.fgb", std::process::id()));
    let fixture = conformance_fixture();
    write_conformance_lines(&input_path, &fixture);

    polygonize_flatgeobuf_file(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        PolygonizerOptions::default(),
    )
    .unwrap();

    let reader = FgbReader::open(BufReader::new(File::open(&output_path).unwrap())).unwrap();
    assert_eq!(reader.header().geometry_type(), GeometryType::Polygon);
    assert_eq!(reader.header().crs().unwrap().code(), 4326);
    let mut features = reader.select_all().unwrap();
    let feature = features.next().unwrap().unwrap();
    let geometry = feature
        .geometry_trait()
        .unwrap()
        .unwrap()
        .try_to_geometry()
        .unwrap();
    let geo::Geometry::Polygon(polygon) = geometry else {
        panic!("expected polygon output");
    };
    assert_conformance_polygon(&fixture, &polygon);
    assert!(features.next().unwrap().is_none());

    fs::remove_file(input_path).unwrap();
    fs::remove_file(output_path).unwrap();
}

fn conformance_fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../geo-polygonize-core/tests/fixtures/conformance/axis_aligned_ring_v1.json"
    ))
    .unwrap()
}

fn canonical_ring(mut ring: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    ring.pop();
    let twice_area: f64 = ring
        .iter()
        .zip(ring.iter().cycle().skip(1))
        .map(|((x1, y1), (x2, y2))| x1 * y2 - x2 * y1)
        .sum();
    if twice_area < 0.0 {
        ring.reverse();
    }
    let first = ring
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
        })
        .unwrap()
        .0;
    ring.rotate_left(first);
    ring.push(ring[0]);
    ring
}

fn assert_conformance_polygon(fixture: &Value, polygon: &geo::Polygon) {
    let expected = fixture["expected_fingerprint"]["polygons"][0]["exterior"]
        .as_array()
        .unwrap()
        .iter()
        .map(|coord| {
            let parse = |name| {
                f64::from_bits(
                    u64::from_str_radix(&coord[name].as_str().unwrap()[2..], 16).unwrap(),
                )
            };
            (parse("x"), parse("y"))
        })
        .collect::<Vec<_>>();
    let actual = polygon
        .exterior()
        .0
        .iter()
        .map(|coord| (coord.x, coord.y))
        .collect();
    assert_eq!(canonical_ring(actual), canonical_ring(expected));
}

fn write_conformance_lines(path: &std::path::Path, fixture: &Value) {
    let options = FgbWriterOptions {
        crs: FgbCrs {
            code: 4326,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut writer =
        FgbWriter::create_with_options("lines", GeometryType::LineString, options).unwrap();
    let points = fixture["coords"]
        .as_array()
        .unwrap()
        .chunks_exact(2)
        .map(|point| {
            geo::coord! { x: point[0].as_f64().unwrap(), y: point[1].as_f64().unwrap() }
        })
        .collect::<Vec<_>>();
    for pair in points.windows(2) {
        writer
            .add_feature_geom(
                geo::Geometry::LineString(geo::LineString::new(pair.to_vec())),
                |_| {},
            )
            .unwrap();
    }
    writer
        .write(BufWriter::new(File::create(path).unwrap()))
        .unwrap();
}
