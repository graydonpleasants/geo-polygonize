#[cfg(test)]
#[allow(clippy::module_inception)]
mod tests {
    use crate::{Coord3D, PolygonizeError, PolygonizerOptions, TiledPolygonizer};
    use geo::{Contains, Coord, Geometry, LineString, Rect};

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

        let polys = tiler.polygonize().unwrap();

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

        let polys = tiler.polygonize().unwrap();

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

        let polys = tiler.polygonize().unwrap();
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

        let polys = tiler.polygonize().unwrap();
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

        let polys = tiler.polygonize().unwrap();
        assert_eq!(
            polys.len(),
            1,
            "Should identify polygon based on CanonicalBoundaryHash ownership policy"
        );
    }

    #[test]
    fn representative_ownership_uses_an_interior_point() {
        use crate::options::TileOwnershipPolicy;
        use crate::Polygon3D;

        let polygon = Polygon3D::new(
            vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(4.0, 0.0, 0.0),
                Coord3D::new(4.0, 4.0, 0.0),
                Coord3D::new(3.0, 4.0, 0.0),
                Coord3D::new(3.0, 1.0, 0.0),
                Coord3D::new(1.0, 1.0, 0.0),
                Coord3D::new(1.0, 4.0, 0.0),
                Coord3D::new(0.0, 4.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
            ],
            vec![],
            vec![],
            vec![],
        );
        let polygon_2d = polygon.to_polygon_2d();
        assert!(!polygon_2d.contains(&polygon.centroid_2d().unwrap()));

        let tiler = TiledPolygonizer::new(
            Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 4.0, y: 4.0 }),
            2.0,
        )
        .with_ownership_policy(TileOwnershipPolicy::RepresentativePointInsidePolygon);
        assert!(polygon_2d.contains(&tiler.ownership_point(&polygon).unwrap()));
    }

    #[test]
    fn rejects_invalid_tiling_configuration_and_options() {
        let bbox = Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 1.0, y: 1.0 });
        assert!(matches!(
            TiledPolygonizer::new(bbox, 0.0).polygonize(),
            Err(PolygonizeError::InvalidArgumentType { field, .. }) if field == "tile_size"
        ));
        assert!(matches!(
            TiledPolygonizer::new(bbox, 1.0)
                .with_buffer(f64::NAN)
                .polygonize(),
            Err(PolygonizeError::InvalidArgumentType { field, .. }) if field == "buffer"
        ));
        assert!(matches!(
            TiledPolygonizer::new(
                Rect::new(Coord { x: 1.0, y: 1.0 }, Coord { x: 1.0, y: 1.0 }),
                1.0,
            )
            .polygonize(),
            Err(PolygonizeError::InvalidGeometry { .. })
        ));

        let options = PolygonizerOptions {
            pre_snap_tolerance: 1.0,
            ..Default::default()
        };
        assert!(matches!(
            TiledPolygonizer::new(bbox, 1.0)
                .with_options(options)
                .polygonize(),
            Err(PolygonizeError::UnsupportedOptionCombination { .. })
        ));

        let square = Geometry::LineString(LineString::new(vec![
            Coord { x: 0.0, y: 0.0 },
            Coord { x: 1.0, y: 0.0 },
            Coord { x: 1.0, y: 1.0 },
            Coord { x: 0.0, y: 1.0 },
            Coord { x: 0.0, y: 0.0 },
        ]));
        let mut options = PolygonizerOptions::default();
        options.output_filter.minimum_face_area = Some(2.0);
        let mut tiler = TiledPolygonizer::new(bbox, 1.0).with_options(options);
        tiler.add_geometry(&square);
        assert!(tiler.polygonize().unwrap().is_empty());
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
    let polys_keep = t_keep.polygonize().unwrap();
    assert_eq!(polys_keep.len(), 1);

    let mut t_dedup = TiledPolygonizer::new(
        Rect::new(Coord { x: 0.0, y: 0.0 }, Coord { x: 10.0, y: 10.0 }),
        10.0,
    )
    .with_dedup_policy(DedupPolicy::CanonicalRingHash);
    t_dedup.add_geometry(&geom1);
    t_dedup.add_geometry(&geom2);
    let polys_dedup = t_dedup.polygonize().unwrap();
    assert_eq!(polys_dedup.len(), 1);
}

#[test]
fn canonical_dedup_key_compares_exact_geometry() {
    use super::canonical_polygon_key;
    use crate::{Coord3D, Polygon3D};

    let polygon = |max_x| {
        Polygon3D::new(
            vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(max_x, 0.0, 0.0),
                Coord3D::new(max_x, 1.0, 0.0),
                Coord3D::new(0.0, 1.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
            ],
            vec![],
            vec![],
            vec![],
        )
    };

    let equivalent = Polygon3D::new(
        vec![
            Coord3D::new(1.0, 1.0, 0.0),
            Coord3D::new(1.0, 0.0, 0.0),
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(0.0, 1.0, 0.0),
            Coord3D::new(1.0, 1.0, 0.0),
        ],
        vec![],
        vec![],
        vec![],
    );
    let key = canonical_polygon_key(&polygon(1.0));

    assert_eq!(key, canonical_polygon_key(&equivalent));
    assert_ne!(key, canonical_polygon_key(&polygon(2.0)));
}
