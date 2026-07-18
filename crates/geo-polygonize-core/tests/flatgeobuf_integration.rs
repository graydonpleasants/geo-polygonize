#![cfg(feature = "flatgeobuf")]

use flatgeobuf::{
    FallibleStreamingIterator, FgbCrs, FgbReader, FgbWriter, FgbWriterOptions, GeometryType,
};
use geo::Area;
use geo_polygonize_core::flatgeobuf_api::polygonize_flatgeobuf_file;
use geo_polygonize_core::options::PolygonizerOptions;
use geo_traits::to_geo::ToGeoGeometry;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};

#[test]
fn polygonizes_a_flatgeobuf_file() {
    let input_path =
        std::env::temp_dir().join(format!("geo-polygonize-{}-input.fgb", std::process::id()));
    let output_path =
        std::env::temp_dir().join(format!("geo-polygonize-{}-output.fgb", std::process::id()));
    write_square(&input_path);

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
    assert_eq!(polygon.unsigned_area(), 100.0);
    assert!(features.next().unwrap().is_none());

    fs::remove_file(input_path).unwrap();
    fs::remove_file(output_path).unwrap();
}

fn write_square(path: &std::path::Path) {
    let options = FgbWriterOptions {
        crs: FgbCrs {
            code: 4326,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut writer =
        FgbWriter::create_with_options("lines", GeometryType::LineString, options).unwrap();
    let points = [
        geo::coord! { x: 0.0, y: 0.0 },
        geo::coord! { x: 10.0, y: 0.0 },
        geo::coord! { x: 10.0, y: 10.0 },
        geo::coord! { x: 0.0, y: 10.0 },
        geo::coord! { x: 0.0, y: 0.0 },
    ];
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
