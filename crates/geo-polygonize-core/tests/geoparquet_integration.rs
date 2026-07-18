#![cfg(feature = "geoparquet")]

use arrow::record_batch::RecordBatch;
use geo_polygonize_core::geoparquet_api::polygonize_geoparquet_file;
use geo_polygonize_core::options::PolygonizerOptions;
use geo_traits::{LineStringTrait, PolygonTrait};
use geoarrow::array::{GeoArrowArray, GeoArrowArrayAccessor, LineStringBuilder, PolygonArray};
use geoarrow::datatypes::{Dimension, LineStringType};
use geoparquet::reader::{GeoParquetReaderBuilder, GeoParquetRecordBatchReader};
use geoparquet::writer::{GeoParquetRecordBatchEncoder, GeoParquetWriterOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use std::fs::{self, File};
use std::sync::Arc;

#[test]
fn polygonizes_a_geoparquet_file() {
    let input_path = std::env::temp_dir().join(format!(
        "geo-polygonize-{}-input.parquet",
        std::process::id()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "geo-polygonize-{}-output.parquet",
        std::process::id()
    ));

    write_square(&input_path);
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
    assert_eq!(polygons.len(), 1);
    let polygon = polygons.get(0).unwrap().unwrap();
    assert_eq!(polygon.exterior().unwrap().num_coords(), 5);

    fs::remove_file(input_path).unwrap();
    fs::remove_file(output_path).unwrap();
}

fn write_square(path: &std::path::Path) {
    let points = [
        geo::coord! { x: 0.0, y: 0.0 },
        geo::coord! { x: 10.0, y: 0.0 },
        geo::coord! { x: 10.0, y: 10.0 },
        geo::coord! { x: 0.0, y: 10.0 },
        geo::coord! { x: 0.0, y: 0.0 },
    ];
    let typ = LineStringType::new(Dimension::XY, Arc::new(Default::default()));
    let mut lines = LineStringBuilder::new(typ);
    for pair in points.windows(2) {
        lines
            .push_line_string(Some(&geo::LineString::new(pair.to_vec())))
            .unwrap();
    }
    let lines = lines.finish();
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
