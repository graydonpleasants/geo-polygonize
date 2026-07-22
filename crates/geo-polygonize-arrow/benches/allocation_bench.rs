#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use arrow::array::make_array;
use arrow::datatypes::Field;
use arrow::ffi::{from_ffi_and_data_type, FFI_ArrowArray, FFI_ArrowSchema};
use geo_polygonize_arrow::ffi::{polygonize_ffi, PolygonizerOptions as FfiOptions};
use geo_polygonize_arrow::polygonize_arrow;
use geo_polygonize_core::Polygonizer;
use geo_polygonize_core::PolygonizerOptions;
use geo_types::{Coord, LineString};
use geoarrow::array::{GeoArrowArray, PolygonArray};
use geoarrow::datatypes::Metadata;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::convert::TryFrom;
use std::hint::black_box;
use std::sync::Arc;

#[allow(dead_code)]
mod geoarrow_reference {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/geoarrow/reference.rs"
    ));
}

fn measure(name: &str, runs: u64, warm_up: bool, mut run: impl FnMut() -> u64) {
    if warm_up {
        black_box(run());
    }
    let before = dhat::HeapStats::get();
    let minimum_boundary_bytes: u64 = (0..runs).map(|_| run()).sum();
    let after = dhat::HeapStats::get();
    println!(
        "{name},{},{},{minimum_boundary_bytes}",
        (after.total_blocks - before.total_blocks) / runs,
        (after.total_bytes - before.total_bytes) / runs,
        minimum_boundary_bytes = minimum_boundary_bytes / runs,
    );
}

fn main() {
    let profile_path =
        std::env::temp_dir().join(format!("geo-polygonize-dhat-{}.json", std::process::id()));
    let _profiler = dhat::Profiler::builder().file_name(profile_path).build();
    // This is a lower bound: IPC/file paths must transfer their payload bytes.
    // A zero here means no serialization boundary, not an allocation-free call.
    println!("boundary,allocations,allocated_bytes,minimum_boundary_bytes");

    let input = geoarrow_reference::square(Arc::new(Metadata::default()));
    let field = input.data_type().to_field("geometry", true);
    let array = input.into_array_ref();
    measure("rust_arrow", 10, true, || {
        let polygons = polygonize_arrow(
            black_box(array.as_ref()),
            black_box(&field),
            PolygonizerOptions::default(),
        )
        .unwrap();
        geoarrow_reference::assert_square(&polygons);
        0
    });

    let input = geoarrow_reference::square(Arc::new(Metadata::default()));
    let input_field = input.data_type().to_field("geometry", true);
    let input_array = input.into_array_ref();
    measure("c_data_interface", 10, true, || {
        let mut ffi_input_array = FFI_ArrowArray::new(&input_array.to_data());
        let mut ffi_input_schema = FFI_ArrowSchema::try_from(&input_field).unwrap();
        let mut output_array = FFI_ArrowArray::empty();
        let mut output_schema = FFI_ArrowSchema::empty();
        let options = FfiOptions {
            node_input: 0,
            snap_grid_size: 1e-10,
            extract_only_polygonal: 0,
            report_mode: 0,
        };
        let status = unsafe {
            polygonize_ffi(
                &mut ffi_input_array,
                &mut ffi_input_schema,
                &mut output_array,
                &mut output_schema,
                &options,
            )
        };
        assert_eq!(status, 0);
        let field = Field::try_from(&output_schema).unwrap();
        let data =
            unsafe { from_ffi_and_data_type(output_array, field.data_type().clone()).unwrap() };
        let array = make_array(data);
        let polygons = PolygonArray::try_from((array.as_ref(), &field)).unwrap();
        geoarrow_reference::assert_square(&polygons);
        0
    });

    let ipc = geoarrow_reference::square_ipc(Arc::new(Metadata::default()));
    measure("wasm_arrow_ipc", 10, true, || {
        let (array, field) = geoarrow_reference::read_geometry_ipc(&ipc);
        let polygons =
            polygonize_arrow(array.as_ref(), &field, PolygonizerOptions::default()).unwrap();
        let schema = Arc::new(arrow::datatypes::Schema::new(vec![polygons
            .data_type()
            .to_field("geometry", true)]));
        let batch = arrow::record_batch::RecordBatch::try_new(
            schema.clone(),
            vec![polygons.into_array_ref()],
        )
        .unwrap();
        let mut output = Vec::new();
        let mut writer = arrow::ipc::writer::StreamWriter::try_new(&mut output, &schema).unwrap();
        writer.write(&batch).unwrap();
        writer.finish().unwrap();
        drop(writer);
        black_box(&output);
        (ipc.len() + output.len()) as u64
    });

    let official_ipc = geoarrow_reference::official_separated_ipc();
    measure("official_arrow_ipc", 10, true, || {
        let (array, field) = geoarrow_reference::read_geometry_ipc(&official_ipc);
        let polygons =
            polygonize_arrow(array.as_ref(), &field, PolygonizerOptions::default()).unwrap();
        assert_eq!(polygons.len(), 0);
        official_ipc.len() as u64
    });

    #[cfg(feature = "geoparquet")]
    measure_geoparquet();

    let mut rng = StdRng::seed_from_u64(42);
    let lines: Vec<_> = (0..100)
        .map(|_| {
            let coords = (0..rng.gen_range(5..20))
                .map(|_| Coord {
                    x: rng.gen_range(0.0..100.0),
                    y: rng.gen_range(0.0..100.0),
                })
                .collect();
            LineString::new(coords)
        })
        .collect();
    measure("core_random_100", 1, false, || {
        let mut polygonizer = Polygonizer::with_options(PolygonizerOptions {
            node_input: true,
            ..Default::default()
        });
        for line in &lines {
            polygonizer.add_geometry(line.clone().into());
        }
        black_box(polygonizer.polygonize().unwrap());
        0
    });
}

#[cfg(feature = "geoparquet")]
fn measure_geoparquet() {
    use arrow::record_batch::RecordBatch;
    use geo_polygonize_arrow::geoparquet_api::polygonize_geoparquet_file;
    use geoparquet::writer::{GeoParquetRecordBatchEncoder, GeoParquetWriterOptions};
    use parquet::arrow::ArrowWriter;
    use std::fs::{self, File};

    let input_path = std::env::temp_dir().join(format!(
        "geo-polygonize-allocation-{}-input.parquet",
        std::process::id()
    ));
    let output_path = std::env::temp_dir().join(format!(
        "geo-polygonize-allocation-{}-output.parquet",
        std::process::id()
    ));
    let lines = geoarrow_reference::square(Arc::new(Metadata::default()));
    let schema = Arc::new(arrow::datatypes::Schema::new(vec![lines
        .data_type()
        .to_field("geometry", false)]));
    let batch = RecordBatch::try_new(schema.clone(), vec![lines.into_array_ref()]).unwrap();
    let mut encoder =
        GeoParquetRecordBatchEncoder::try_new(&schema, &GeoParquetWriterOptions::default())
            .unwrap();
    let mut writer = ArrowWriter::try_new(
        File::create(&input_path).unwrap(),
        encoder.target_schema(),
        None,
    )
    .unwrap();
    writer
        .write(&encoder.encode_record_batch(&batch).unwrap())
        .unwrap();
    writer.append_key_value_metadata(encoder.into_keyvalue().unwrap());
    writer.close().unwrap();

    let input_bytes = fs::metadata(&input_path).unwrap().len();
    measure("geoparquet_file", 10, true, || {
        polygonize_geoparquet_file(
            input_path.to_str().unwrap(),
            output_path.to_str().unwrap(),
            "geometry",
            PolygonizerOptions::default(),
        )
        .unwrap();
        input_bytes + fs::metadata(&output_path).unwrap().len()
    });

    fs::remove_file(input_path).unwrap();
    fs::remove_file(output_path).unwrap();
}
