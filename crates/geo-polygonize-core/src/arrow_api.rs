use crate::types::{Coord3D, Line3D};
use crate::Polygonizer;
use arrow::array::{Array, AsArray, GenericListArray};
use arrow::datatypes::{DataType, Field, Float64Type};
use geo_traits::to_geo::ToGeoLineString;
use geoarrow::array::{GeoArrowArray, GeoArrowArrayAccessor, LineStringArray, PolygonBuilder};
use geoarrow::datatypes::{Dimension, PolygonType};
use std::convert::TryFrom;
use std::sync::Arc;

pub struct PolygonizerOptions {
    pub node_input: bool,
    pub snap_grid_size: f64,
    pub extract_only_polygonal: bool,
}

pub fn polygonize_arrow(
    array: &dyn Array,
    field: &Field,
    options: PolygonizerOptions,
) -> Result<geoarrow::array::PolygonArray, String> {
    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = options.node_input;
    polygonizer.snap_grid_size = options.snap_grid_size;
    polygonizer.extract_only_polygonal = options.extract_only_polygonal;

    let mut lines = Vec::new();

    // Attempt 1: Standard try_from
    if let Ok(arr) = LineStringArray::try_from((array, field)) {
        process_linestring_array(&arr, &mut lines);
    } else {
        // Fallback 2: Patch metadata
        let mut new_metadata = field.metadata().clone();
        new_metadata.insert(
            "ARROW:extension:name".to_string(),
            "ogc.geoarrow.linestring".to_string(),
        );
        let new_field = field.clone().with_metadata(new_metadata);

        if let Ok(arr) = LineStringArray::try_from((array, &new_field)) {
            process_linestring_array(&arr, &mut lines);
        } else {
            // Fallback 3: Construct field from array DataType
            match array.data_type() {
                DataType::List(_) => {
                    // as_list returns &GenericListArray<O>, NOT Option.
                    // And it panics if type mismatch in older versions, but `AsArray::as_list` in recent arrow (52+)
                    // assumes the caller checked the type or it panics?
                    // Docs: "Downcast this to a GenericListArray. Panics if the array is not a GenericListArray."
                    // So we must be careful. We checked DataType::List above.
                    let list_arr = array.as_list::<i32>();
                    process_list_array(list_arr, &mut lines)?;
                }
                DataType::LargeList(_) => {
                    let list_arr = array.as_list::<i64>();
                    process_list_array(list_arr, &mut lines)?;
                }
                _ => {
                    let array_type = array.data_type();
                    let new_field_exact = Field::new("geometry", array_type.clone(), true)
                        .with_metadata(
                            [(
                                "ARROW:extension:name".to_string(),
                                "ogc.geoarrow.linestring".to_string(),
                            )]
                            .into(),
                        );

                    if let Ok(arr) = LineStringArray::try_from((array, &new_field_exact)) {
                        process_linestring_array(&arr, &mut lines);
                    } else {
                        return Err(format!(
                             "Failed to convert input array to LineStringArray and fallback failed. DataType: {:?}, Field: {:?}.",
                             array.data_type(),
                             field
                         ));
                    }
                }
            }
        }
    }

    polygonizer.add_lines(lines);

    let result = polygonizer
        .polygonize()
        .map_err(|e| format!("Polygonization error: {}", e))?;

    let geo_polygons: Vec<geo::Polygon> = result
        .polygons
        .into_iter()
        .map(|p| {
            let exterior = geo::LineString::from(
                p.exterior
                    .into_iter()
                    .map(|c| (c.x, c.y))
                    .collect::<Vec<_>>(),
            );
            let interiors = p
                .interiors
                .into_iter()
                .map(|ring| {
                    geo::LineString::from(ring.into_iter().map(|c| (c.x, c.y)).collect::<Vec<_>>())
                })
                .collect();
            geo::Polygon::new(exterior, interiors)
        })
        .collect();

    let mut builder = PolygonBuilder::new(PolygonType::new(
        Dimension::XY,
        Arc::new(Default::default()),
    ));
    for poly in geo_polygons {
        builder
            .push_polygon(Some(&poly))
            .map_err(|e| format!("Failed to push polygon: {}", e))?;
    }
    Ok(builder.finish())
}

fn process_linestring_array(arr: &LineStringArray, lines: &mut Vec<Line3D>) {
    for i in 0..arr.len() {
        if let Ok(Some(geom)) = arr.get(i) {
            let ls = geom.to_line_string();
            for line in ls.lines() {
                let p1 = Coord3D::new(line.start.x, line.start.y, 0.0);
                let p2 = Coord3D::new(line.end.x, line.end.y, 0.0);
                lines.push(Line3D::new(p1, p2, 0));
            }
        }
    }
}

// Manual fallback for GenericListArray<Offset>
fn process_list_array<O: arrow::array::OffsetSizeTrait>(
    list_arr: &GenericListArray<O>,
    lines: &mut Vec<Line3D>,
) -> Result<(), String> {
    // Values should be StructArray with x, y
    let values = list_arr.values();
    let struct_arr = values.as_struct_opt().ok_or("List values must be Struct")?;

    // Get x and y columns
    // We assume field names "x" and "y" or indices 0 and 1
    // GeoArrow spec uses "x", "y".
    let x_arr = struct_arr
        .column_by_name("x")
        .or_else(|| {
            if struct_arr.num_columns() > 0 {
                Some(struct_arr.column(0))
            } else {
                None
            }
        })
        .ok_or("Struct missing 'x' column")?;

    let y_arr = struct_arr
        .column_by_name("y")
        .or_else(|| {
            if struct_arr.num_columns() > 1 {
                Some(struct_arr.column(1))
            } else {
                None
            }
        })
        .ok_or("Struct missing 'y' column")?;

    let x_vals = x_arr
        .as_primitive_opt::<Float64Type>()
        .ok_or("'x' column must be Float64")?;
    let y_vals = y_arr
        .as_primitive_opt::<Float64Type>()
        .ok_or("'y' column must be Float64")?;

    for i in 0..list_arr.len() {
        if list_arr.is_null(i) {
            continue;
        }
        let start = list_arr.value_offsets()[i].to_usize().unwrap();
        let end = list_arr.value_offsets()[i + 1].to_usize().unwrap();

        if end <= start {
            continue;
        }

        // Iterate points in the linestring
        for j in start..end - 1 {
            let x1 = x_vals.value(j);
            let y1 = y_vals.value(j);
            let x2 = x_vals.value(j + 1);
            let y2 = y_vals.value(j + 1);

            let p1 = Coord3D::new(x1, y1, 0.0);
            let p2 = Coord3D::new(x2, y2, 0.0);
            lines.push(Line3D::new(p1, p2, 0));
        }
    }

    Ok(())
}
