use crate::error::PolygonizeError;
use crate::polygonize;
use crate::types::{Coord3D, Line3D};
use arrow::array::{Array, AsArray, GenericListArray};
use arrow::datatypes::{DataType, Field, Float64Type};
use geo_traits::to_geo::ToGeoLineString;
use geoarrow::array::{GeoArrowArray, GeoArrowArrayAccessor, LineStringArray, PolygonBuilder};
use geoarrow::datatypes::{Dimension, GeoArrowType, Metadata, PolygonType};
use std::convert::TryFrom;
use std::sync::Arc;

pub use crate::options::PolygonizerOptions;

pub fn polygonize_arrow(
    array: &dyn Array,
    field: &Field,
    options: PolygonizerOptions,
) -> Result<geoarrow::array::PolygonArray, PolygonizeError> {
    let metadata = match GeoArrowType::from_extension_field(field)
        .map_err(|e| PolygonizeError::ArrowError(e.to_string()))?
    {
        Some(GeoArrowType::LineString(typ)) => {
            if typ.dimension() != Dimension::XY {
                return Err(PolygonizeError::UnsupportedOptionCombination {
                    reason: format!(
                        "GeoArrow polygonization currently supports XY coordinates only, got {:?}",
                        typ.dimension()
                    ),
                });
            }
            typ.metadata().clone()
        }
        Some(other) => {
            return Err(PolygonizeError::InvalidArgumentType {
                field: field.name().to_string(),
                expected: "geoarrow.linestring".to_string(),
                actual: format!("{other:?}"),
            });
        }
        None => Arc::new(
            Metadata::try_from(field).map_err(|e| PolygonizeError::ArrowError(e.to_string()))?,
        ),
    };

    if let Some(edges) = metadata.edges() {
        return Err(PolygonizeError::UnsupportedOptionCombination {
            reason: format!("GeoArrow polygonization supports planar edges only, got {edges:?}"),
        });
    }

    let mut lines = Vec::new();

    match LineStringArray::try_from((array, field)) {
        Ok(arr) => process_linestring_array(&arr, &mut lines)?,
        Err(error)
            if field
                .extension_type_name()
                .is_some_and(|name| name != "ogc.geoarrow.linestring") =>
        {
            return Err(PolygonizeError::ArrowError(error.to_string()));
        }
        Err(_) => match array.data_type() {
            DataType::List(_) => process_list_array(array.as_list::<i32>(), &mut lines)?,
            DataType::LargeList(_) => process_list_array(array.as_list::<i64>(), &mut lines)?,
            _ => {
                return Err(PolygonizeError::ArrowError(format!(
                    "Failed to convert input array to LineStringArray and fallback failed. DataType: {:?}, Field: {:?}.",
                    array.data_type(),
                    field
                )));
            }
        },
    }

    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| polygonize(lines, &options)))
            .unwrap_or_else(|_| {
                Err(PolygonizeError::Panic(
                    "Panic occurred in Rust core".to_string(),
                ))
            })?;

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
                .map(|ring: Vec<Coord3D>| {
                    geo::LineString::from(ring.into_iter().map(|c| (c.x, c.y)).collect::<Vec<_>>())
                })
                .collect();
            geo::Polygon::new(exterior, interiors)
        })
        .collect();

    let mut builder = PolygonBuilder::new(PolygonType::new(Dimension::XY, metadata));
    for poly in geo_polygons {
        builder
            .push_polygon(Some(&poly))
            .map_err(|e| PolygonizeError::ArrowError(format!("Failed to push polygon: {}", e)))?;
    }
    Ok(builder.finish())
}

fn process_linestring_array(
    arr: &LineStringArray,
    lines: &mut Vec<Line3D>,
) -> Result<(), PolygonizeError> {
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
    Ok(())
}

// Manual fallback for GenericListArray<Offset>
fn process_list_array<O: arrow::array::OffsetSizeTrait>(
    list_arr: &GenericListArray<O>,
    lines: &mut Vec<Line3D>,
) -> Result<(), PolygonizeError> {
    // Values should be StructArray with x, y
    let values = list_arr.values();
    let struct_arr = values
        .as_struct_opt()
        .ok_or_else(|| PolygonizeError::InvalidBufferShape {
            reason: "List values must be Struct".to_string(),
        })?;

    if struct_arr.num_columns() != 2 {
        return Err(PolygonizeError::UnsupportedOptionCombination {
            reason: format!(
                "GeoArrow polygonization currently supports XY coordinates only, got {} coordinate columns",
                struct_arr.num_columns()
            ),
        });
    }

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
        .ok_or_else(|| PolygonizeError::InvalidBufferShape {
            reason: "Struct missing 'x' column".to_string(),
        })?;

    let y_arr = struct_arr
        .column_by_name("y")
        .or_else(|| {
            if struct_arr.num_columns() > 1 {
                Some(struct_arr.column(1))
            } else {
                None
            }
        })
        .ok_or_else(|| PolygonizeError::InvalidBufferShape {
            reason: "Struct missing 'y' column".to_string(),
        })?;

    let x_vals = x_arr.as_primitive_opt::<Float64Type>().ok_or_else(|| {
        PolygonizeError::InvalidArgumentType {
            field: "x".to_string(),
            expected: "Float64".to_string(),
            actual: format!("{:?}", x_arr.data_type()),
        }
    })?;
    let y_vals = y_arr.as_primitive_opt::<Float64Type>().ok_or_else(|| {
        PolygonizeError::InvalidArgumentType {
            field: "y".to_string(),
            expected: "Float64".to_string(),
            actual: format!("{:?}", y_arr.data_type()),
        }
    })?;

    for i in 0..list_arr.len() {
        if list_arr.is_null(i) {
            continue;
        }
        let start = list_arr.value_offsets()[i].to_usize().ok_or_else(|| {
            PolygonizeError::InvalidBufferShape {
                reason: "Invalid start offset: cannot convert to usize".to_string(),
            }
        })?;
        let end = list_arr.value_offsets()[i + 1].to_usize().ok_or_else(|| {
            PolygonizeError::InvalidBufferShape {
                reason: "Invalid end offset: cannot convert to usize".to_string(),
            }
        })?;

        if end <= start {
            continue;
        }

        if end > x_vals.len() || end > y_vals.len() {
            return Err(PolygonizeError::InvalidBufferShape {
                reason: "Offset out of bounds for x/y coordinate arrays".to_string(),
            });
        }

        // Iterate points in the linestring
        for j in start..end - 1 {
            let x1 = x_vals.value(j);
            let y1 = y_vals.value(j);
            let x2 = x_vals.value(j + 1);
            let y2 = y_vals.value(j + 1);

            if !x1.is_finite() || !y1.is_finite() || !x2.is_finite() || !y2.is_finite() {
                return Err(PolygonizeError::InvalidGeometry {
                    reason: "NaN or Inf coordinates detected in list array".to_string(),
                });
            }

            let p1 = Coord3D::new(x1, y1, 0.0);
            let p2 = Coord3D::new(x2, y2, 0.0);
            lines.push(Line3D::new(p1, p2, 0));
        }
    }

    Ok(())
}
