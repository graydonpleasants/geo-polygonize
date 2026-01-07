#[cfg(test)]
mod tests {
    use crate::Polygonizer;
    use geo_types::LineString;
    use geo::Area;

    #[test]
    fn test_polygonize_simple_triangle() {
        let mut poly = Polygonizer::new();
        // Triangle
        poly.add_geometry(LineString::from(vec![(0.0, 0.0), (10.0, 0.0)]).into());
        poly.add_geometry(LineString::from(vec![(10.0, 0.0), (0.0, 10.0)]).into());
        poly.add_geometry(LineString::from(vec![(0.0, 10.0), (0.0, 0.0)]).into());

        let polygons = poly.polygonize().unwrap();
        // Should find 1 shell (the triangle) and possibly the universe hole as a shell if orientation reversed?
        // Our logic adds holes without shell as shells (reversed).
        // The universe hole is (0,0)->(0,10)->(10,0)->(0,0) (CW).
        // Signed area is negative.
        // It will be classified as a hole.
        // It's not contained in the triangle (shell).
        // So it becomes a new shell (reversed).
        // Wait, the universe hole corresponds to the infinite face.
        // If we reverse it, it becomes the "universe" polygon?
        // Usually polygonizers typically only return finite polygons.
        // JTS Polygonizer returns only finite polygons unless configured?
        // JTS documentation says "Extracts the polygons".
        // The infinite face usually is not returned as a polygon.
        // But our logic converts "unparented holes" to shells.
        // We should probably filter out the infinite face if possible, or maybe our logic matches JTS behavior where it returns everything.

        // Let's check count.
        assert!(polygons.len() >= 1);

        // One of them should be the triangle with positive area ~50.
        let triangle = polygons.iter().find(|p| p.unsigned_area() > 49.0 && p.unsigned_area() < 51.0);
        assert!(triangle.is_some());
    }

    #[test]
    fn test_polygonize_hole() {
        let mut poly = Polygonizer::new();
        // Outer square (CCW) 0,0 -> 10,0 -> 10,10 -> 0,10 -> 0,0
        poly.add_geometry(LineString::from(vec![
            (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)
        ]).into());

        // Inner square (CW) 2,2 -> 2,8 -> 8,8 -> 8,2 -> 2,2
        // Wait, input lines don't have direction in terms of graph. The graph builds directed edges both ways.
        // So we just add lines.
        poly.add_geometry(LineString::from(vec![
            (2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0), (2.0, 2.0)
        ]).into());

        // Connect them? No, disconnected graph components.
        // The outer cycle forms a Shell (Area 100).
        // The inner cycle forms a Shell (Area 36) and a Hole (Area -36) relative to its interior/exterior.
        // Wait.
        // The graph for outer square has 2 faces: Infinite and Square.
        // Square is CCW (Area 100). Infinite is CW.

        // The graph for inner square has 2 faces: Infinite (inside the square?) No.
        // Inner square cycle:
        // CCW traversal: (2,2)->(8,2)->(8,8)->(2,8)->(2,2). Area 36. This is the "hole" shape but filled.
        // CW traversal: (2,2)->(2,8)->(8,8)->(8,2)->(2,2). Area -36. This is the "outside" of the inner square.

        // So we get:
        // 1. Outer Square Shell (CCW) -> +100
        // 2. Outer Square Hole (CW) -> -100 (Infinite face)
        // 3. Inner Square Shell (CCW) -> +36 (The hole itself as a polygon)
        // 4. Inner Square Hole (CW) -> -36 (The space outside the inner square, i.e. the ring)

        // We have:
        // Shells: Outer(+100), Inner(+36).
        // Holes: Outer(-100), Inner(-36).

        // Logic:
        // Outer(-100) is not contained in anything (it's infinite). Becomes Shell(+100).
        // Inner(-36) is contained in Outer(+100).
        // So Inner(-36) becomes a hole of Outer(+100).

        // Resulting Polygons:
        // A. Outer(+100) with Hole Inner(-36). (The donut).
        // B. Inner(+36). (The island in the hole).
        // C. Outer(-100 turned to +100) aka Universe?

        // Wait, JTS Polygonizer logic is subtle.
        // Typically, "included" polygons are those formed by the edges.
        // The space between Outer and Inner squares is the Donut.
        // The trace of that space is:
        // Outer ring CCW + Inner ring CW.
        // Wait, graph traversal finds MINIMAL cycles.
        // A minimal cycle for the donut face would traverse outer boundary and inner boundary?
        // No, planar graph faces are simple cycles.
        // If the graph is disconnected (nested rings), the face logic depends on "LineSweep" or "HoleAssigner" inferring the relationship.
        // The graph traversal only finds the rings.
        // It finds ring Outer(CCW) and ring Inner(CW) as the boundaries of the donut face?
        // No, it finds ring Outer(CCW) as one cycle.
        // It finds ring Inner(CW) as another cycle?
        // Actually, for the Inner square, the CW cycle surrounds the inner square (infinite outwards).
        // The CCW cycle surrounds the interior of the inner square.

        // So the graph produces:
        // 1. Ring Outer CCW (Area 100).
        // 2. Ring Outer CW (Area -100).
        // 3. Ring Inner CCW (Area 36).
        // 4. Ring Inner CW (Area -36).

        // Classification:
        // Shells: OuterCCW, InnerCCW.
        // Holes: OuterCW, InnerCW.

        // Hole Assignment:
        // InnerCW (Env: 2..8) is contained in OuterCCW (Env: 0..10).
        // OuterCW (Env: 0..10) is NOT contained in anything.

        // Assignment:
        // OuterCCW gets InnerCW as hole. -> Donut Polygon (100 - 36 = 64 area).
        // InnerCCW gets no holes. -> Inner Polygon (36 area).
        // OuterCW gets no shell -> Converted to Shell? Or ignored if we filter universe.
        // If we convert, we get another huge polygon.

        // So we expect at least the Donut and the Island.

        let polygons = poly.polygonize().unwrap();

        // We expect a polygon with area ~64 (100-36).
        let donut = polygons.iter().find(|p| (p.unsigned_area() - 64.0).abs() < 1.0);
        if donut.is_none() {
            let areas: Vec<f64> = polygons.iter().map(|p| p.unsigned_area()).collect();
            panic!("Donut polygon not found. Found areas: {:?}", areas);
        }
        assert!(donut.is_some(), "Donut polygon not found");
        assert_eq!(donut.unwrap().interiors().len(), 1);

        // We expect a polygon with area ~36.
        let island = polygons.iter().find(|p| (p.unsigned_area() - 36.0).abs() < 1.0);
        assert!(island.is_some(), "Island polygon not found");
    }
}
