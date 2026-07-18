use crate::error::PolygonizeError;
use crate::options::PolygonizerOptions;
use crate::polygonizer::polygonize_with_options;
use crate::types::{Coord3D, Line3D, Polygon3D};
use flatgeobuf::{
    FallibleStreamingIterator, FgbCrs, FgbReader, FgbWriter, FgbWriterOptions, GeometryType,
};
use geo_traits::to_geo::ToGeoGeometry;
use std::fs::File;
use std::io::{BufReader, BufWriter};

pub fn polygonize_flatgeobuf_file(
    input_path: &str,
    output_path: &str,
    options: PolygonizerOptions,
) -> Result<(), PolygonizeError> {
    let reader = FgbReader::open(BufReader::new(
        File::open(input_path).map_err(invalid_geometry)?,
    ))
    .map_err(invalid_geometry)?;
    let header = reader.header();
    if header.has_z() || header.has_m() || header.has_t() || header.has_tm() {
        return Err(PolygonizeError::UnsupportedOptionCombination {
            reason: "FlatGeobuf polygonization currently supports XY geometry only".to_string(),
        });
    }

    let crs = header.crs();
    let crs_org = crs.and_then(|value| value.org()).map(str::to_owned);
    let crs_name = crs.and_then(|value| value.name()).map(str::to_owned);
    let crs_description = crs.and_then(|value| value.description()).map(str::to_owned);
    let crs_wkt = crs.and_then(|value| value.wkt()).map(str::to_owned);
    let crs_code_string = crs.and_then(|value| value.code_string()).map(str::to_owned);
    let crs_code = crs.map(|value| value.code()).unwrap_or_default();

    let mut features = reader.select_all().map_err(invalid_geometry)?;
    let mut lines = Vec::new();
    let mut source_id = 0_u32;
    while let Some(feature) = features.next().map_err(invalid_geometry)? {
        let id = source_id;
        source_id = source_id
            .checked_add(1)
            .ok_or_else(|| PolygonizeError::InvalidGeometry {
                reason: "FlatGeobuf contains more than u32::MAX features".to_string(),
            })?;
        let Some(geometry) = feature.geometry_trait().map_err(invalid_geometry)? else {
            continue;
        };
        let Some(geometry) = geometry.try_to_geometry() else {
            continue;
        };
        match geometry {
            geo::Geometry::LineString(line) => add_line_string(&line, id, &mut lines)?,
            geo::Geometry::MultiLineString(multi) => {
                for line in &multi.0 {
                    add_line_string(line, id, &mut lines)?;
                }
            }
            other => {
                return Err(PolygonizeError::InvalidGeometry {
                    reason: format!(
                        "FlatGeobuf input must contain LineString or MultiLineString geometry, got {other:?}"
                    ),
                });
            }
        }
    }

    let result = polygonize_with_options(&lines, &options)?;
    let writer_options = FgbWriterOptions {
        crs: FgbCrs {
            org: crs_org.as_deref(),
            code: crs_code,
            name: crs_name.as_deref(),
            description: crs_description.as_deref(),
            wkt: crs_wkt.as_deref(),
            code_string: crs_code_string.as_deref(),
        },
        ..Default::default()
    };
    let mut writer =
        FgbWriter::create_with_options("polygons", GeometryType::Polygon, writer_options)
            .map_err(invalid_geometry)?;
    for polygon in result.polygons {
        writer
            .add_feature_geom(geo::Geometry::Polygon(to_geo_polygon(polygon)), |_| {})
            .map_err(invalid_geometry)?;
    }
    writer
        .write(BufWriter::new(
            File::create(output_path).map_err(invalid_geometry)?,
        ))
        .map_err(invalid_geometry)
}

fn add_line_string(
    line: &geo::LineString,
    source_id: u32,
    lines: &mut Vec<Line3D>,
) -> Result<(), PolygonizeError> {
    for segment in line.lines() {
        if !segment.start.x.is_finite()
            || !segment.start.y.is_finite()
            || !segment.end.x.is_finite()
            || !segment.end.y.is_finite()
        {
            return Err(PolygonizeError::InvalidGeometry {
                reason: "FlatGeobuf contains NaN or infinite coordinates".to_string(),
            });
        }
        lines.push(Line3D::new(
            Coord3D::new(segment.start.x, segment.start.y, 0.0),
            Coord3D::new(segment.end.x, segment.end.y, 0.0),
            source_id,
        ));
    }
    Ok(())
}

fn to_geo_polygon(polygon: Polygon3D) -> geo::Polygon {
    geo::Polygon::new(
        to_geo_ring(polygon.exterior),
        polygon.interiors.into_iter().map(to_geo_ring).collect(),
    )
}

fn to_geo_ring(ring: Vec<Coord3D>) -> geo::LineString {
    ring.into_iter().map(|coord| (coord.x, coord.y)).collect()
}

fn invalid_geometry(error: impl ToString) -> PolygonizeError {
    PolygonizeError::InvalidGeometry {
        reason: error.to_string(),
    }
}
