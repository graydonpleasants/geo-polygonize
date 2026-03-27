use crate::arrow_api::polygonize_arrow;
use crate::error::PolygonizeError;
use crate::options::PolygonizerOptions;
use arrow::array::Array;
use arrow::datatypes::{Field, Schema};
use arrow::record_batch::RecordBatch;
use geoarrow::array::GeoArrowArray;
use geoarrow::datatypes::CoordType;
use geoparquet::reader::GeoParquetReaderBuilder;
use geoparquet::writer::{GeoParquetRecordBatchEncoder, GeoParquetWriterOptions};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use parquet::arrow::ArrowWriter;
use std::fs::File;
use std::sync::Arc;

pub fn polygonize_geoparquet_file(
    input_path: &str,
    output_path: &str,
    geometry_column_name: &str,
    options: PolygonizerOptions,
) -> Result<(), PolygonizeError> {
    let file = File::open(input_path).map_err(|e| PolygonizeError::InvalidGeometry {
        reason: e.to_string(),
    })?;

    let builder = ParquetRecordBatchReaderBuilder::try_new(file).map_err(|e| {
        PolygonizeError::InvalidGeometry {
            reason: e.to_string(),
        }
    })?;

    let geo_meta = builder.geoparquet_metadata().transpose().map_err(|e| {
        PolygonizeError::InvalidGeometry {
            reason: e.to_string(),
        }
    })?;

    let _schema = if let Some(meta) = &geo_meta {
        // True = convert geometries to geoarrow from wkb, Interleaved

        builder
            .geoarrow_schema(meta, true, CoordType::Interleaved)
            .map_err(|e| PolygonizeError::InvalidGeometry {
                reason: e.to_string(),
            })?
    } else {
        builder.schema().clone()
    };

    let reader = builder
        .build()
        .map_err(|e| PolygonizeError::InvalidGeometry {
            reason: e.to_string(),
        })?;

    let out_file = File::create(output_path).map_err(|e| PolygonizeError::InvalidGeometry {
        reason: e.to_string(),
    })?;
    let mut parquet_writer: Option<ArrowWriter<File>> = None;
    let mut gpq_encoder: Option<GeoParquetRecordBatchEncoder> = None;

    for batch_res in reader {
        let batch: RecordBatch = batch_res.map_err(|e| PolygonizeError::InvalidGeometry {
            reason: e.to_string(),
        })?;

        let mut new_columns: Vec<Arc<dyn Array>> = Vec::new();
        let mut actual_new_fields = Vec::new();

        let schema_ref = batch.schema();

        for (i, col) in batch.columns().iter().enumerate() {
            let field = schema_ref.field(i);

            if field.name() == geometry_column_name {
                let polygon_array = polygonize_arrow(col.as_ref(), field, options.clone())?;
                let arr_ref = polygon_array.into_array_ref();
                let new_field = Arc::new(Field::new(
                    field.name(),
                    arr_ref.data_type().clone(),
                    field.is_nullable(),
                ));
                actual_new_fields.push(new_field);
                new_columns.push(arr_ref);
            } else {
                actual_new_fields.push(Arc::new(field.clone()));
                new_columns.push(col.clone());
            }
        }

        let new_schema = Arc::new(Schema::new(actual_new_fields));
        let new_batch = RecordBatch::try_new(new_schema.clone(), new_columns).map_err(|e| {
            PolygonizeError::InvalidGeometry {
                reason: e.to_string(),
            }
        })?;

        if parquet_writer.is_none() {
            let writer_options = GeoParquetWriterOptions::default();
            let encoder = GeoParquetRecordBatchEncoder::try_new(&new_schema, &writer_options)
                .map_err(|e| PolygonizeError::InvalidGeometry {
                    reason: e.to_string(),
                })?;
            let writer = ArrowWriter::try_new(
                out_file.try_clone().unwrap(),
                encoder.target_schema().clone(),
                None,
            )
            .map_err(|e| PolygonizeError::InvalidGeometry {
                reason: e.to_string(),
            })?;
            parquet_writer = Some(writer);
            gpq_encoder = Some(encoder);
        }

        let encoded_batch = gpq_encoder
            .as_mut()
            .unwrap()
            .encode_record_batch(&new_batch)
            .map_err(|e| PolygonizeError::InvalidGeometry {
                reason: e.to_string(),
            })?;

        parquet_writer
            .as_mut()
            .unwrap()
            .write(&encoded_batch)
            .map_err(|e| PolygonizeError::InvalidGeometry {
                reason: e.to_string(),
            })?;
    }

    if let (Some(mut writer), Some(encoder)) = (parquet_writer, gpq_encoder) {
        let kv_metadata =
            encoder
                .into_keyvalue()
                .map_err(|e| PolygonizeError::InvalidGeometry {
                    reason: e.to_string(),
                })?;
        writer.append_key_value_metadata(kv_metadata);
        writer
            .close()
            .map_err(|e| PolygonizeError::InvalidGeometry {
                reason: e.to_string(),
            })?;
    }

    Ok(())
}
