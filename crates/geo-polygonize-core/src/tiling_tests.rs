#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::TiledPolygonizer;
    use geo::{Coord, Geometry, LineString, Rect};

    #[test]
    fn test_tiled_polygonization_grid() {
        // Create a 2x2 grid of squares
        // 0,0 - 10,0 - 20,0
        //  |     |      |
        // 0,10- 10,10- 20,10
        //  |     |      |
        // 0,20- 10,20- 20,20

        let geoms = vec![
            // Horizontals
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 20.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 10.0 },
                Coord { x: 20.0, y: 10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 20.0 },
                Coord { x: 20.0, y: 20.0 },
            ])),
            // Verticals
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 0.0, y: 20.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 10.0, y: 0.0 },
                Coord { x: 10.0, y: 20.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 20.0, y: 0.0 },
                Coord { x: 20.0, y: 20.0 },
            ])),
        ];

        // BBox covers 0,0 to 20,20
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        // Tile size 10 (exactly matching lines) or 15 (offset)
        // Let's try 15 to ensure polygons span tiles
        // Add buffer of 5.0 to ensure full polygons are captured in each tile
        let mut tiler = TiledPolygonizer::new(bbox, 15.0).with_buffer(5.0);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize();

        // Should find 4 polygons
        assert_eq!(polys.len(), 4);

        // Check areas
        for p in polys {
            assert!((p.unsigned_area_2d() - 100.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_tiled_polygonization_exact_boundary() {
        // Tile size 10, lines on 10.
        // This tests the "ownership" logic at boundaries.

        let geoms = vec![
            // Horizontals
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 20.0, y: 0.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 10.0 },
                Coord { x: 20.0, y: 10.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 20.0 },
                Coord { x: 20.0, y: 20.0 },
            ])),
            // Verticals
            Geometry::LineString(LineString::new(vec![
                Coord { x: 0.0, y: 0.0 },
                Coord { x: 0.0, y: 20.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 10.0, y: 0.0 },
                Coord { x: 10.0, y: 20.0 },
            ])),
            Geometry::LineString(LineString::new(vec![
                Coord { x: 20.0, y: 0.0 },
                Coord { x: 20.0, y: 20.0 },
            ])),
        ];

        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        // Tile size 10.
        // Tiles: [0,10]x[0,10], [10,20]x[0,10], etc.
        let mut tiler = TiledPolygonizer::new(bbox, 10.0);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize();

        assert_eq!(polys.len(), 4);
    }

    #[test]
    fn test_tiled_polygonization_centroid_on_max_boundary() {
        // A square centered at (20, 5).
        // 19,0 -> 21,0 -> 21,10 -> 19,10 -> 19,0.
        // Centroid is x=20, y=5.
        // BBox passed is 0,0 -> 20,20.
        // This simulates a polygon on the edge of the world.

        let geoms = vec![Geometry::LineString(LineString::new(vec![
            Coord { x: 19.0, y: 0.0 },
            Coord { x: 21.0, y: 0.0 },
            Coord { x: 21.0, y: 10.0 },
            Coord { x: 19.0, y: 10.0 },
            Coord { x: 19.0, y: 0.0 },
        ]))];

        // BBox 0,0 -> 20,20.
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        // Tile size 10.
        // Tiles: [0,10) and [10,20).
        let mut tiler = TiledPolygonizer::new(bbox, 10.0).with_buffer(5.0);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize();
        assert_eq!(
            polys.len(),
            1,
            "Should identify polygon with centroid on the boundary"
        );
    }

    #[test]
    fn test_lexicographic_min_vertex_ownership() {
        use crate::options::TileOwnershipPolicy;

        // A single square crossing the x=10 boundary.
        // Bbox: [8, 0] to [12, 4].
        // Centroid is x=10, y=2.
        // Lexicographic Min Vertex is x=8, y=0.
        let geoms = vec![Geometry::LineString(LineString::new(vec![
            Coord { x: 8.0, y: 0.0 },
            Coord { x: 12.0, y: 0.0 },
            Coord { x: 12.0, y: 4.0 },
            Coord { x: 8.0, y: 4.0 },
            Coord { x: 8.0, y: 0.0 },
        ]))];

        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        let mut tiler = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(5.0)
            .with_ownership_policy(TileOwnershipPolicy::LexicographicMinVertex);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize();
        assert_eq!(
            polys.len(),
            1,
            "Should identify polygon based on LexicographicMinVertex"
        );
    }

    #[test]
    fn test_canonical_boundary_hash_ownership() {
        use crate::options::TileOwnershipPolicy;

        let geoms = vec![Geometry::LineString(LineString::new(vec![
            Coord { x: 8.0, y: 0.0 },
            Coord { x: 12.0, y: 0.0 },
            Coord { x: 12.0, y: 4.0 },
            Coord { x: 8.0, y: 4.0 },
            Coord { x: 8.0, y: 0.0 },
        ]))];

        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 20.0, y: 20.0 });

        let mut tiler = TiledPolygonizer::new(bbox, 10.0)
            .with_buffer(5.0)
            .with_ownership_policy(TileOwnershipPolicy::CanonicalBoundaryHash);

        for g in &geoms {
            tiler.add_geometry(g);
        }

        let polys = tiler.polygonize();
        assert_eq!(
            polys.len(),
            1,
            "Should identify polygon based on CanonicalBoundaryHash ownership policy"
        );
    }
}

#[test]
fn test_dedup_policy_canonical_ring_hash() {
    use crate::options::DedupPolicy;
    use crate::TiledPolygonizer;
    use geo::{Coord, Geometry, LineString, Rect};

    let geom1 = Geometry::LineString(LineString::new(vec![
        Coord { x: 1.0, y: 1.0 },
        Coord { x: 9.0, y: 1.0 },
        Coord { x: 9.0, y: 9.0 },
        Coord { x: 1.0, y: 9.0 },
        Coord { x: 1.0, y: 1.0 },
    ]));

    let geom2 = Geometry::LineString(LineString::new(vec![
        Coord { x: 9.0, y: 1.0 },
        Coord { x: 9.0, y: 9.0 },
        Coord { x: 1.0, y: 9.0 },
        Coord { x: 1.0, y: 1.0 },
        Coord { x: 9.0, y: 1.0 },
    ]));

    let mut t_keep = TiledPolygonizer::new(
        Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 }),
        10.0,
    )
    .with_dedup_policy(DedupPolicy::KeepAll);
    t_keep.add_geometry(&geom1);
    t_keep.add_geometry(&geom2);
    let polys_keep = t_keep.polygonize();
    assert_eq!(polys_keep.len(), 1);

    let mut t_dedup = TiledPolygonizer::new(
        Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 }),
        10.0,
    )
    .with_dedup_policy(DedupPolicy::CanonicalRingHash);
    t_dedup.add_geometry(&geom1);
    t_dedup.add_geometry(&geom2);
    let polys_dedup = t_dedup.polygonize();
    assert_eq!(polys_dedup.len(), 1);
}
