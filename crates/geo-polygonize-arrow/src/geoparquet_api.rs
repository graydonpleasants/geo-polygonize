use crate::arrow_api::linestring_metadata;
use crate::polygonize_arrow;
use arrow::array::{new_empty_array, ArrayRef};
use arrow::compute::concat;
use arrow::datatypes::Schema;
use arrow::record_batch::RecordBatch;
use geo_polygonize_core::{PolygonizeError, PolygonizerOptions};
use geoarrow::array::GeoArrowArray;
use geoarrow::datatypes::CoordType;
use geoparquet::metadata::GeoParquetColumnEncoding;
use geoparquet::reader::{GeoParquetReaderBuilder, GeoParquetRecordBatchReader};
use geoparquet::writer::{GeoParquetRecordBatchEncoder, GeoParquetWriterOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use std::fmt::Display;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

/// Polygonize one GeoParquet file as a complete topology domain.
///
/// The input must contain exactly the requested geometry column and satisfy the
/// XY LineString and planar-edge constraints of [polygonize_arrow]. Additional
/// columns are rejected because input lines have no positional correspondence to
/// output polygons. Project the geometry column explicitly before calling this
/// function if the source contains attributes or bbox covering columns.
///
/// All input geometry is read into memory before polygonization, so faces may
/// span row groups or record batches. Independent drawings or coordinate systems
/// must be processed separately. The output contains one polygon column with the
/// original name and CRS, including a valid zero-row dataset when no faces exist.
///
/// The output is written to a temporary file in the destination directory and
/// replaces the destination only after writing and closing succeeds.
///
/// # Errors
///
/// Returns an error for unsupported input schemas, invalid geometry or options,
/// and read or write failures. An error leaves an existing destination unchanged.
pub fn polygonize_geoparquet_file(
    input_path: &str,
    output_path: &str,
    geometry_column_name: &str,
    options: PolygonizerOptions,
) -> Result<(), PolygonizeError> {
    let file = File::open(input_path).map_err(geoparquet_error)?;
    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(geoparquet_error)?;

    let geometry_index = builder
        .schema()
        .index_of(geometry_column_name)
        .map_err(geoparquet_error)?;
    if builder.schema().fields().len() != 1 {
        return Err(PolygonizeError::UnsupportedOptionCombination {
            reason: "GeoParquet polygonization requires exactly one geometry column; input \
                attributes have no positional correspondence to output polygons. Project the \
                geometry column explicitly before polygonizing."
                .to_string(),
        });
    }

    let geo_meta = builder
        .geoparquet_metadata()
        .transpose()
        .map_err(geoparquet_error)?;
    let schema = if let Some(meta) = &geo_meta {
        // The dependency asserts this invariant during native schema inference.
        // Reject malformed metadata here instead of allowing an input-driven panic.
        if let Some(column) = meta.columns.get(geometry_column_name) {
            if column.encoding != GeoParquetColumnEncoding::WKB && column.geometry_types.len() != 1
            {
                return Err(PolygonizeError::InvalidGeometry {
                    reason: format!(
                        "Native GeoParquet column {geometry_column_name:?} must declare exactly \
                        one geometry type"
                    ),
                });
            }
        }

        // Native GeoParquet uses separated coordinates. Request that same layout
        // for WKB too: geoparquet 0.7 does not convert native input to interleaved.
        builder
            .geoarrow_schema(meta, true, CoordType::Separated)
            .map_err(geoparquet_error)?
    } else {
        builder.schema().clone()
    };
    let field = schema.field(geometry_index);
    // Validate before decoding: unsupported native targets can panic inside the
    // GeoParquet reader before polygonize_arrow has a chance to reject them.
    linestring_metadata(field)?;
    let reader = GeoParquetRecordBatchReader::try_new(
        builder.build().map_err(geoparquet_error)?,
        schema.clone(),
    )
    .map_err(geoparquet_error)?;

    let geometry = {
        let mut chunks: Vec<ArrayRef> = Vec::new();
        for batch in reader {
            let batch = batch.map_err(geoparquet_error)?;
            if batch.num_rows() > 0 {
                chunks.push(batch.column(geometry_index).clone());
            }
        }

        match chunks.as_slice() {
            [] => new_empty_array(field.data_type()),
            [chunk] => chunk.clone(),
            chunks => {
                let arrays = chunks
                    .iter()
                    .map(|chunk| chunk.as_ref())
                    .collect::<Vec<_>>();
                concat(&arrays).map_err(geoparquet_error)?
            }
        }
    };

    // Polygonize exactly once, including empty input so schema and options are
    // validated even when the reader yields no batches.
    let polygons = polygonize_arrow(geometry.as_ref(), field, options)?;
    drop(geometry);
    let output_schema = Arc::new(Schema::new(vec![polygons
        .data_type()
        .to_field(geometry_column_name, false)]));
    let output_batch = RecordBatch::try_new(output_schema.clone(), vec![polygons.into_array_ref()])
        .map_err(geoparquet_error)?;

    let mut encoder =
        GeoParquetRecordBatchEncoder::try_new(&output_schema, &GeoParquetWriterOptions::default())
            .map_err(geoparquet_error)?;
    // Encoding an empty batch in geoparquet 0.7 adds a nonfinite bbox. The typed
    // schema already supplies Polygon metadata; leaving bbox unset is correct.
    let encoded_batch = if output_batch.num_rows() == 0 {
        None
    } else {
        Some(
            encoder
                .encode_record_batch(&output_batch)
                .map_err(geoparquet_error)?,
        )
    };
    let target_schema = encoder.target_schema();
    let metadata = encoder.into_keyvalue().map_err(geoparquet_error)?;
    drop(output_batch);

    let output_path = Path::new(output_path);
    let output_directory = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut output_file = tempfile::Builder::new()
        .prefix(".geo-polygonize-")
        .suffix(".parquet")
        .tempfile_in(output_directory)
        .map_err(geoparquet_error)?;
    {
        let mut writer = ArrowWriter::try_new(output_file.as_file_mut(), target_schema, None)
            .map_err(geoparquet_error)?;
        if let Some(batch) = encoded_batch {
            writer.write(&batch).map_err(geoparquet_error)?;
        }
        writer.append_key_value_metadata(metadata);
        writer.close().map_err(geoparquet_error)?;
    }
    output_file.persist(output_path).map_err(geoparquet_error)?;
    Ok(())
}

fn geoparquet_error(error: impl Display) -> PolygonizeError {
    PolygonizeError::InvalidGeometry {
        reason: error.to_string(),
    }
}
