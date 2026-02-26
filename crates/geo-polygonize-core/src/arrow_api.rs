use crate::types::{Coord3D, Line3D};
use crate::Polygonizer;
use arrow::array::Array;
use arrow::datatypes::Field;
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
    // Convert to LineStringArray
    let line_string_array = LineStringArray::try_from((array, field))
        .map_err(|e| format!("Failed to convert to LineStringArray: {}", e))?;

    let mut polygonizer = Polygonizer::new();
    polygonizer.node_input = options.node_input;
    polygonizer.snap_grid_size = options.snap_grid_size;
    polygonizer.extract_only_polygonal = options.extract_only_polygonal;

    let mut lines = Vec::new();
    for i in 0..line_string_array.len() {
        if let Ok(Some(geom)) = line_string_array.get(i) {
            let ls = geom.to_line_string();
            for line in ls.lines() {
                let p1 = Coord3D::new(line.start.x, line.start.y, 0.0);
                let p2 = Coord3D::new(line.end.x, line.end.y, 0.0);
                lines.push(Line3D::new(p1, p2));
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
                    geo::LineString::from(
                        ring.into_iter()
                            .map(|c| (c.x, c.y))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            geo::Polygon::new(exterior, interiors)
        })
        .collect();

    let mut builder = PolygonBuilder::new(PolygonType::new(Dimension::XY, Arc::new(Default::default())));
    for poly in geo_polygons {
        builder
            .push_polygon(Some(&poly))
            .map_err(|e| format!("Failed to push polygon: {}", e))?;
    }
    Ok(builder.finish())
}
