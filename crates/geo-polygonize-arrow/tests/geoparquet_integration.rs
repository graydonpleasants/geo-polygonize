#![cfg(feature = "geoparquet")]

use arrow::record_batch::RecordBatch;
use geo_polygonize_arrow::geoparquet_api::polygonize_geoparquet_file;
use geo_polygonize_arrow::PolygonizerOptions;
use geo_traits::{CoordTrait, LineStringTrait, PolygonTrait};
use geoarrow::array::{
    GeoArrowArray, GeoArrowArrayAccessor, LineStringArray, LineStringBuilder, PolygonArray,
};
use geoarrow::datatypes::{Dimension, LineStringType};
use geoparquet::reader::{GeoParquetReaderBuilder, GeoParquetRecordBatchReader};
use geoparquet::writer::{GeoParquetRecordBatchEncoder, GeoParquetWriterOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use serde_json::Value;
use std::fs::{self, File};
use std::sync::Arc;

#[test]
fn polygonizes_the_shared_conformance_fixture() {
    let input_path = std::env::temp_dir().join(format!(
        "geo-polygonize-{}-input.parquet",
        std::process::id()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "geo-polygonize-{}-output.parquet",
        std::process::id()
    ));

    let fixture = conformance_fixture();
    write_conformance_lines(&input_path, &fixture);
    polygonize_geoparquet_file(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        "geometry",
        PolygonizerOptions::default(),
    )
    .unwrap();

    let builder =
        ParquetRecordBatchReaderBuilder::try_new(File::open(&output_path).unwrap()).unwrap();
    let metadata = builder.geoparquet_metadata().unwrap().unwrap();
    assert_eq!(metadata.primary_column, "geometry");
    let schema = builder
        .geoarrow_schema(&metadata, true, geoarrow::datatypes::CoordType::Interleaved)
        .unwrap();
    let reader = builder.build().unwrap();
    let mut reader = GeoParquetRecordBatchReader::try_new(reader, schema.clone()).unwrap();
    let batch = reader.next().unwrap().unwrap();
    assert!(reader.next().is_none());

    let polygons = PolygonArray::try_from((batch.column(0).as_ref(), schema.field(0))).unwrap();
    assert_conformance_polygon(&fixture, &polygons);

    fs::remove_file(input_path).unwrap();
    fs::remove_file(output_path).unwrap();
}

fn conformance_fixture() -> Value {
    serde_json::from_str(include_str!(
        "../../geo-polygonize-core/tests/fixtures/conformance/axis_aligned_ring_v1.json"
    ))
    .unwrap()
}

fn conformance_input(fixture: &Value) -> LineStringArray {
    let coords = fixture["coords"].as_array().unwrap();
    let line = geo::LineString::from(
        coords
            .chunks_exact(2)
            .map(|point| (point[0].as_f64().unwrap(), point[1].as_f64().unwrap()))
            .collect::<Vec<_>>(),
    );
    let mut builder = LineStringBuilder::new(LineStringType::new(
        Dimension::XY,
        Arc::new(Default::default()),
    ));
    builder.push_line_string(Some(&line)).unwrap();
    builder.finish()
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

fn assert_conformance_polygon(fixture: &Value, polygons: &PolygonArray) {
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
    let polygon = polygons.get(0).unwrap().unwrap();
    let exterior = polygon.exterior().unwrap();
    let actual = (0..exterior.num_coords())
        .map(|index| {
            let coord = exterior.coord(index).unwrap();
            (coord.x(), coord.y())
        })
        .collect::<Vec<_>>();
    assert_eq!(canonical_ring(actual), canonical_ring(expected));
}

fn write_conformance_lines(path: &std::path::Path, fixture: &Value) {
    let lines = conformance_input(fixture);
    let schema = Arc::new(arrow::datatypes::Schema::new(vec![lines
        .data_type()
        .to_field("geometry", false)]));
    let batch = RecordBatch::try_new(schema.clone(), vec![lines.into_array_ref()]).unwrap();

    let mut encoder =
        GeoParquetRecordBatchEncoder::try_new(&schema, &GeoParquetWriterOptions::default())
            .unwrap();
    let mut writer =
        ArrowWriter::try_new(File::create(path).unwrap(), encoder.target_schema(), None).unwrap();
    writer
        .write(&encoder.encode_record_batch(&batch).unwrap())
        .unwrap();
    writer.append_key_value_metadata(encoder.into_keyvalue().unwrap());
    writer.close().unwrap();
}
