use arrow::array::ArrayRef;
use arrow::datatypes::Field;
use arrow::ipc::reader::StreamReader;
use arrow::ipc::writer::StreamWriter;
use arrow::record_batch::RecordBatch;
use geo_traits::{CoordTrait, LineStringTrait, PolygonTrait};
use geoarrow::array::{
    GeoArrowArray, GeoArrowArrayAccessor, LineStringArray, LineStringBuilder, PolygonArray,
};
use geoarrow::datatypes::{CoordType, Dimension, LineStringType, Metadata};
use std::io::Cursor;
use std::sync::Arc;

pub fn official_separated_ipc() -> Vec<u8> {
    write_ipc(official_array(
        Dimension::XY,
        CoordType::Separated,
        geoarrow_test::raw::linestring::xy::geoms(),
    ))
}

pub fn official_interleaved_ipc() -> Vec<u8> {
    write_ipc(official_array(
        Dimension::XY,
        CoordType::Interleaved,
        geoarrow_test::raw::linestring::xy::geoms(),
    ))
}

pub fn official_non_xy_ipc() -> Vec<Vec<u8>> {
    vec![
        write_ipc(official_array(
            Dimension::XYZ,
            CoordType::Separated,
            geoarrow_test::raw::linestring::xyz::geoms(),
        )),
        write_ipc(official_array(
            Dimension::XYM,
            CoordType::Separated,
            geoarrow_test::raw::linestring::xym::geoms(),
        )),
        write_ipc(official_array(
            Dimension::XYZM,
            CoordType::Separated,
            geoarrow_test::raw::linestring::xyzm::geoms(),
        )),
    ]
}

fn official_array<G: LineStringTrait<T = f64>>(
    dimension: Dimension,
    coord_type: CoordType,
    geometries: Vec<Option<G>>,
) -> LineStringArray {
    let typ = LineStringType::new(dimension, Arc::new(Metadata::default()))
        .with_coord_type(coord_type);
    let mut builder = LineStringBuilder::new(typ);
    for geometry in geometries {
        builder.push_line_string(geometry.as_ref()).unwrap();
    }
    builder.finish()
}

pub fn read_geometry_ipc(bytes: &[u8]) -> (ArrayRef, Field) {
    let mut reader = StreamReader::try_new(Cursor::new(bytes), None).unwrap();
    let schema = reader.schema();
    let geometry_index = schema.index_of("geometry").unwrap();
    let batch = reader.next().unwrap().unwrap();
    assert!(reader.next().is_none());
    (
        batch.column(geometry_index).clone(),
        schema.field(geometry_index).clone(),
    )
}

pub fn square(metadata: Arc<Metadata>) -> LineStringArray {
    let mut builder = LineStringBuilder::new(LineStringType::new(Dimension::XY, metadata));
    builder
        .push_line_string(Some(&geo::LineString::from(vec![
            (0., 0.),
            (10., 0.),
            (10., 10.),
            (0., 10.),
            (0., 0.),
        ])))
        .unwrap();
    builder.finish()
}

pub fn square_ipc(metadata: Arc<Metadata>) -> Vec<u8> {
    write_ipc(square(metadata))
}

fn write_ipc(array: LineStringArray) -> Vec<u8> {
    let schema = Arc::new(arrow::datatypes::Schema::new(vec![array
        .data_type()
        .to_field("geometry", true)]));
    let batch = RecordBatch::try_new(schema.clone(), vec![array.into_array_ref()]).unwrap();
    let mut bytes = Vec::new();
    let mut writer = StreamWriter::try_new(&mut bytes, &schema).unwrap();
    writer.write(&batch).unwrap();
    writer.finish().unwrap();
    drop(writer);
    bytes
}

pub fn assert_square(array: &PolygonArray) {
    assert_eq!(array.len(), 1);
    let polygon = array.get(0).unwrap().unwrap();
    let exterior = polygon.exterior().unwrap();
    assert_eq!(exterior.num_coords(), 5);

    let mut twice_area = 0.0;
    for index in 0..exterior.num_coords() - 1 {
        let start = exterior.coord(index).unwrap();
        let end = exterior.coord(index + 1).unwrap();
        twice_area += start.x() * end.y() - end.x() * start.y();
    }
    assert_eq!(twice_area.abs() / 2.0, 100.0);
}
