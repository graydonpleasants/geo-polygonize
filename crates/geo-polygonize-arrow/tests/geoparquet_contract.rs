#![cfg(feature = "geoparquet")]

use arrow::array::Int32Array;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use geo::LineString;
use geo_polygonize_arrow::geoparquet_api::polygonize_geoparquet_file;
use geo_polygonize_arrow::PolygonizerOptions;
use geo_polygonize_core::PolygonizeErrorKind;
use geo_traits::{CoordTrait, LineStringTrait, PolygonTrait};
use geoarrow::array::{
    GeoArrowArray, GeoArrowArrayAccessor, LineStringBuilder, PolygonArray, PolygonBuilder,
};
use geoarrow::datatypes::{CoordType, Crs, Dimension, LineStringType, Metadata, PolygonType};
use geoparquet::reader::{GeoParquetReaderBuilder, GeoParquetRecordBatchReader};
use geoparquet::writer::{
    GeoParquetRecordBatchEncoder, GeoParquetWriterEncoding, GeoParquetWriterOptionsBuilder,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use parquet::file::metadata::KeyValue;
use std::collections::BTreeSet;
use std::fs::{self, File};
use std::path::Path;
use std::sync::Arc;

const EXISTING_OUTPUT: &[u8] = b"existing output must survive input validation failure";

#[test]
fn polygonizes_edges_split_across_default_reader_batches() {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("split-square.parquet");
    let output_path = directory.path().join("polygons.parquet");

    // Null geometries make the file span real reader batches without introducing other
    // linework. The square's closing edge is deliberately in the final batch.
    let mut lines = vec![None; 4097];
    let edges = square_edges();
    lines[0] = Some(edges[0].clone());
    lines[1] = Some(edges[1].clone());
    lines[2] = Some(edges[2].clone());
    *lines.last_mut().unwrap() = Some(edges[3].clone());
    write_geoparquet(
        &input_path,
        &line_batch(&lines, "geometry", Arc::new(Metadata::default())),
    );

    // Do not override batch size: this must exercise the same default reader used by
    // polygonize_geoparquet_file, rather than just multiple writes to one row group.
    let input = read_geoparquet(&input_path);
    assert!(input.batches.len() > 1, "fixture must span reader batches");
    let first = input.batches.first().unwrap().column(0);
    let last = input.batches.last().unwrap().column(0);
    assert_eq!(first.len() - first.null_count(), 3);
    assert_eq!(last.len() - last.null_count(), 1);

    polygonize_file(&input_path, &output_path, "geometry");
    assert_square_output(&read_geoparquet(&output_path));
}

#[test]
fn rejects_attributes_even_when_input_and_polygon_counts_match() {
    // One closed input LineString produces one polygon. Equal row counts do not supply
    // an attribute attribution policy, so silently copying source_id is still invalid.
    assert_attributes_rejected(&[Some(square_ring())]);
}

#[test]
fn rejects_attributes_when_input_and_polygon_counts_differ() {
    let lines = square_edges().into_iter().map(Some).collect::<Vec<_>>();
    assert_attributes_rejected(&lines);
}

#[test]
fn empty_linework_produces_a_readable_zero_row_polygon_file() {
    assert_empty_polygon_output(&[]);
}

#[test]
fn dangle_only_linework_produces_a_readable_zero_row_polygon_file() {
    assert_empty_polygon_output(&[
        Some(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)])),
        Some(LineString::from(vec![(10.0, 0.0), (10.0, 10.0)])),
    ]);
}

#[test]
fn preserves_custom_geometry_column_and_crs() {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("site-lines.parquet");
    let output_path = directory.path().join("site-polygons.parquet");
    let metadata = Arc::new(Metadata::new(
        Crs::from_projjson(serde_json::json!({
            "type": "EngineeringCRS",
            "name": "Local site grid",
            "datum": { "type": "EngineeringDatum", "name": "Local benchmark" },
            "coordinate_system": {
                "subtype": "Cartesian",
                "axis": [
                    { "name": "Easting", "abbreviation": "X", "direction": "east", "unit": "metre" },
                    { "name": "Northing", "abbreviation": "Y", "direction": "north", "unit": "metre" }
                ]
            }
        })),
        None,
    ));
    for encoding in [
        GeoParquetWriterEncoding::WKB,
        GeoParquetWriterEncoding::GeoArrow,
    ] {
        write_geoparquet_with_encoding(
            &input_path,
            &line_batch(&[Some(square_ring())], "site_linework", metadata.clone()),
            encoding,
        );
        let input = read_geoparquet(&input_path);

        polygonize_file(&input_path, &output_path, "site_linework");

        let output = read_geoparquet(&output_path);
        assert_eq!(output.primary_column, "site_linework");
        assert_eq!(output.schema.fields().len(), 1);
        assert_eq!(output.schema.field(0).name(), "site_linework");
        assert_eq!(
            output.schema.field(0).extension_type_metadata(),
            input.schema.field(0).extension_type_metadata()
        );
        assert_square_output(&output);
    }
}

#[test]
fn missing_geometry_column_is_rejected_even_without_rows() {
    for lines in [vec![], vec![Some(square_ring())]] {
        let batch = line_batch(&lines, "linework", Arc::new(Metadata::default()));
        assert_rejected_without_replacing_output(&batch, "missing_geometry");
    }
}

#[test]
fn polygon_input_is_rejected_even_without_rows() {
    for with_polygon in [false, true] {
        let mut builder = PolygonBuilder::new(PolygonType::new(
            Dimension::XY,
            Arc::new(Metadata::default()),
        ));
        if with_polygon {
            builder
                .push_polygon(Some(&geo::Polygon::new(square_ring(), vec![])))
                .unwrap();
        }
        let polygons = builder.finish();
        let schema = Arc::new(Schema::new(vec![polygons
            .data_type()
            .to_field("geometry", true)]));
        let batch = RecordBatch::try_new(schema, vec![polygons.into_array_ref()]).unwrap();
        assert_rejected_without_replacing_output(&batch, "geometry");
    }
}

#[test]
fn scalar_input_is_rejected_even_without_rows() {
    for values in [vec![], vec![1]] {
        let directory = tempfile::tempdir().unwrap();
        let input_path = directory.path().join("not-geometry.parquet");
        let output_path = directory.path().join("existing.parquet");
        let schema = Arc::new(Schema::new(vec![Field::new(
            "geometry",
            DataType::Int32,
            false,
        )]));
        let batch =
            RecordBatch::try_new(schema.clone(), vec![Arc::new(Int32Array::from(values))]).unwrap();
        let mut writer =
            ArrowWriter::try_new(File::create(&input_path).unwrap(), schema, None).unwrap();
        writer.write(&batch).unwrap();
        writer.close().unwrap();
        fs::write(&output_path, EXISTING_OUTPUT).unwrap();

        assert!(polygonize_geoparquet_file(
            input_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "geometry",
            PolygonizerOptions::default(),
        )
        .is_err());
        assert_eq!(fs::read(&output_path).unwrap(), EXISTING_OUTPUT);
    }
}

#[test]
fn rejects_malformed_native_geometry_types_without_replacing_output() {
    for (geometry_types, expected_error_kind) in [
        (vec![], PolygonizeErrorKind::InvalidGeometry),
        (
            vec!["LineString", "Polygon"],
            PolygonizeErrorKind::InvalidGeometry,
        ),
        (
            vec!["GeometryCollection"],
            PolygonizeErrorKind::InvalidArgumentType,
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let input_path = directory.path().join("invalid-metadata.parquet");
        let output_path = directory.path().join("existing.parquet");
        let batch = line_batch(
            &[Some(square_ring())],
            "geometry",
            Arc::new(Metadata::default()),
        );
        let options = GeoParquetWriterOptionsBuilder::default()
            .set_encoding(GeoParquetWriterEncoding::GeoArrow)
            .build();
        let mut encoder =
            GeoParquetRecordBatchEncoder::try_new(batch.schema_ref(), &options).unwrap();
        let mut writer = ArrowWriter::try_new(
            File::create(&input_path).unwrap(),
            encoder.target_schema(),
            None,
        )
        .unwrap();
        writer
            .write(&encoder.encode_record_batch(&batch).unwrap())
            .unwrap();
        let mut metadata = serde_json::to_value(encoder.into_geoparquet_metadata()).unwrap();
        metadata["columns"]["geometry"]["geometry_types"] = serde_json::json!(geometry_types);
        writer.append_key_value_metadata(KeyValue::new(
            "geo".to_string(),
            Some(serde_json::to_string(&metadata).unwrap()),
        ));
        writer.close().unwrap();
        fs::write(&output_path, EXISTING_OUTPUT).unwrap();

        // The dependency asserts single-type native metadata, and its native parser
        // treats GeometryCollection as unreachable. Neither malformed declaration
        // may allow file input to panic before the adapter validates its field.
        let error = polygonize_geoparquet_file(
            input_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "geometry",
            PolygonizerOptions::default(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), expected_error_kind);
        assert_eq!(fs::read(&output_path).unwrap(), EXISTING_OUTPUT);
    }
}

#[test]
fn failed_output_replacement_preserves_directory_and_cleans_up_temporary_file() {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("lines.parquet");
    let output_path = directory.path().join("destination");
    write_geoparquet(
        &input_path,
        &line_batch(
            &[Some(square_ring())],
            "geometry",
            Arc::new(Metadata::default()),
        ),
    );
    fs::create_dir(&output_path).unwrap();
    let sentinel_path = output_path.join("sentinel");
    fs::write(&sentinel_path, EXISTING_OUTPUT).unwrap();
    let original_entries = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<BTreeSet<_>>();

    // A complete temporary output can be written, but a file cannot replace this
    // existing nonempty directory. Exercise finalization rather than input validation.
    assert!(polygonize_geoparquet_file(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        "geometry",
        PolygonizerOptions::default(),
    )
    .is_err());

    assert!(output_path.is_dir());
    assert_eq!(fs::read(&sentinel_path).unwrap(), EXISTING_OUTPUT);
    assert_eq!(fs::read_dir(&output_path).unwrap().count(), 1);
    let remaining_entries = fs::read_dir(directory.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<BTreeSet<_>>();
    assert_eq!(remaining_entries, original_entries);
}

fn square_ring() -> LineString {
    LineString::from(vec![
        (0.0, 0.0),
        (10.0, 0.0),
        (10.0, 10.0),
        (0.0, 10.0),
        (0.0, 0.0),
    ])
}

fn square_edges() -> Vec<LineString> {
    square_ring()
        .lines()
        .map(|line| LineString::from(vec![line.start, line.end]))
        .collect()
}

fn line_batch(
    lines: &[Option<LineString>],
    geometry_column: &str,
    metadata: Arc<Metadata>,
) -> RecordBatch {
    let mut builder = LineStringBuilder::new(LineStringType::new(Dimension::XY, metadata));
    for line in lines {
        builder.push_line_string(line.as_ref()).unwrap();
    }
    let lines = builder.finish();
    let schema = Arc::new(Schema::new(vec![lines
        .data_type()
        .to_field(geometry_column, true)]));
    RecordBatch::try_new(schema, vec![lines.into_array_ref()]).unwrap()
}

fn write_geoparquet(path: &Path, batch: &RecordBatch) {
    write_geoparquet_with_encoding(path, batch, GeoParquetWriterEncoding::WKB);
}

fn write_geoparquet_with_encoding(
    path: &Path,
    batch: &RecordBatch,
    encoding: GeoParquetWriterEncoding,
) {
    let options = GeoParquetWriterOptionsBuilder::default()
        .set_encoding(encoding)
        .build();
    let mut encoder = GeoParquetRecordBatchEncoder::try_new(batch.schema_ref(), &options).unwrap();
    let mut writer =
        ArrowWriter::try_new(File::create(path).unwrap(), encoder.target_schema(), None).unwrap();
    if batch.num_rows() > 0 {
        writer
            .write(&encoder.encode_record_batch(batch).unwrap())
            .unwrap();
    }
    writer.append_key_value_metadata(encoder.into_keyvalue().unwrap());
    writer.close().unwrap();
}

struct DecodedGeoParquet {
    primary_column: String,
    schema: SchemaRef,
    batches: Vec<RecordBatch>,
}

fn read_geoparquet(path: &Path) -> DecodedGeoParquet {
    let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(path).unwrap()).unwrap();
    let metadata = builder.geoparquet_metadata().unwrap().unwrap();
    // GeoParquet native geometries store separated coordinates. Request the matching
    // layout because this reader version cannot cast native input to interleaved;
    // WKB input can be decoded to either layout.
    let schema = builder
        .geoarrow_schema(&metadata, true, CoordType::Separated)
        .unwrap();
    let reader =
        GeoParquetRecordBatchReader::try_new(builder.build().unwrap(), schema.clone()).unwrap();
    DecodedGeoParquet {
        primary_column: metadata.primary_column.clone(),
        schema,
        batches: reader.collect::<Result<Vec<_>, _>>().unwrap(),
    }
}

fn polygonize_file(input: &Path, output: &Path, geometry_column: &str) {
    polygonize_geoparquet_file(
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        geometry_column,
        PolygonizerOptions::default(),
    )
    .unwrap();
}

fn assert_square_output(output: &DecodedGeoParquet) {
    assert_eq!(
        output.schema.field(0).extension_type_name(),
        Some("geoarrow.polygon")
    );
    assert_eq!(
        output
            .batches
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>(),
        1
    );
    let batch = output
        .batches
        .iter()
        .find(|batch| batch.num_rows() > 0)
        .unwrap();
    let polygons =
        PolygonArray::try_from((batch.column(0).as_ref(), output.schema.field(0))).unwrap();
    let polygon = polygons.get(0).unwrap().unwrap();
    assert_eq!(polygon.num_interiors(), 0);
    let exterior = polygon.exterior().unwrap();
    let mut coordinates = (0..exterior.num_coords())
        .map(|index| {
            let point = exterior.coord(index).unwrap();
            (point.x(), point.y())
        })
        .collect::<Vec<_>>();
    assert_eq!(coordinates.len(), 5);
    assert_eq!(coordinates.first(), coordinates.last());
    coordinates.pop();
    coordinates.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.total_cmp(&right.1))
    });
    assert_eq!(
        coordinates,
        vec![(0.0, 0.0), (0.0, 10.0), (10.0, 0.0), (10.0, 10.0)]
    );
}

fn assert_attributes_rejected(lines: &[Option<LineString>]) {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("attributed-lines.parquet");
    let output_path = directory.path().join("existing.parquet");
    let geometry = line_batch(lines, "geometry", Arc::new(Metadata::default()));
    let schema = Arc::new(Schema::new(vec![
        geometry.schema().field(0).clone(),
        Field::new("source_id", DataType::Int32, false),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            geometry.column(0).clone(),
            Arc::new(Int32Array::from_iter_values(0..lines.len() as i32)),
        ],
    )
    .unwrap();
    write_geoparquet(&input_path, &batch);
    fs::write(&output_path, EXISTING_OUTPUT).unwrap();

    let error = polygonize_geoparquet_file(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        "geometry",
        PolygonizerOptions::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        PolygonizeErrorKind::UnsupportedOptionCombination
    );
    assert_eq!(fs::read(&output_path).unwrap(), EXISTING_OUTPUT);
}

fn assert_empty_polygon_output(lines: &[Option<LineString>]) {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("no-polygons.parquet");
    let output_path = directory.path().join("polygons.parquet");
    write_geoparquet(
        &input_path,
        &line_batch(lines, "geometry", Arc::new(Metadata::default())),
    );
    fs::write(&output_path, EXISTING_OUTPUT).unwrap();

    polygonize_file(&input_path, &output_path, "geometry");

    let output = read_geoparquet(&output_path);
    assert_eq!(output.primary_column, "geometry");
    assert_eq!(output.schema.fields().len(), 1);
    assert_eq!(
        output.schema.field(0).extension_type_name(),
        Some("geoarrow.polygon")
    );
    assert_eq!(
        output
            .batches
            .iter()
            .map(RecordBatch::num_rows)
            .sum::<usize>(),
        0
    );
}

fn assert_rejected_without_replacing_output(batch: &RecordBatch, geometry_column: &str) {
    let directory = tempfile::tempdir().unwrap();
    let input_path = directory.path().join("invalid-input.parquet");
    let output_path = directory.path().join("existing.parquet");
    write_geoparquet(&input_path, batch);
    fs::write(&output_path, EXISTING_OUTPUT).unwrap();

    assert!(polygonize_geoparquet_file(
        input_path.to_str().unwrap(),
        output_path.to_str().unwrap(),
        geometry_column,
        PolygonizerOptions::default(),
    )
    .is_err());
    assert_eq!(fs::read(&output_path).unwrap(), EXISTING_OUTPUT);
}
