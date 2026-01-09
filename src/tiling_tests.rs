#[cfg(test)]
mod tests {
    use crate::TiledPolygonizer;
    use geo::{Rect, Coord, LineString, Polygon, Geometry};
    use geo::bounding_rect::BoundingRect;

    #[test]
    fn test_tiled_polygonization_grid() {
        // Create a 2x2 grid of squares
        // 0,0 - 10,0 - 20,0
        //  |     |      |
        // 0,10- 10,10- 20,10
        //  |     |      |
        // 0,20- 10,20- 20,20

        let mut geoms = Vec::new();

        // Horizontals
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 0.0 }])));
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 0.0, y: 10.0 }, Coord { x: 20.0, y: 10.0 }])));
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 0.0, y: 20.0 }, Coord { x: 20.0, y: 20.0 }])));

        // Verticals
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 0.0, y: 20.0 }])));
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 10.0, y: 0.0 }, Coord { x: 10.0, y: 20.0 }])));
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 20.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 }])));

        // BBox covers 0,0 to 20,20
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        // Tile size 10 (exactly matching lines) or 15 (offset)
        // Let's try 15 to ensure polygons span tiles
        // Add buffer of 5.0 to ensure full polygons are captured in each tile
        let mut tiler = TiledPolygonizer::new(bbox, 15.0).with_buffer(5.0);

        for g in geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize();

        // Should find 4 polygons
        assert_eq!(polys.len(), 4);

        // Check areas
        for p in polys {
            use geo::Area;
            assert!((p.unsigned_area() - 100.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_tiled_polygonization_exact_boundary() {
        // Tile size 10, lines on 10.
        // This tests the "ownership" logic at boundaries.

        let mut geoms = Vec::new();
         // Horizontals
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 0.0 }])));
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 0.0, y: 10.0 }, Coord { x: 20.0, y: 10.0 }])));
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 0.0, y: 20.0 }, Coord { x: 20.0, y: 20.0 }])));

        // Verticals
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 0.0, y: 0.0 }, Coord { x: 0.0, y: 20.0 }])));
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 10.0, y: 0.0 }, Coord { x: 10.0, y: 20.0 }])));
        geoms.push(Geometry::LineString(LineString::new(vec![Coord { x: 20.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 }])));

        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        // Tile size 10.
        // Tiles: [0,10]x[0,10], [10,20]x[0,10], etc.
        let mut tiler = TiledPolygonizer::new(bbox, 10.0);

        for g in geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize();

        assert_eq!(polys.len(), 4);
    }
}
