import re

content = open("crates/geo-polygonize-core/src/types.rs").read()
old_func = """    #[inline]
    fn ring_area_and_centroid_2d(coords: &[Coord3D]) -> (f64, f64, f64) {
        if coords.len() < 3 {
            return (0.0, 0.0, 0.0);
        }
        let mut twice_area = 0.0;
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut j = coords.len() - 1;
        for i in 0..coords.len() {
            let f = coords[j].x * coords[i].y - coords[i].x * coords[j].y;
            twice_area += f;
            cx += (coords[j].x + coords[i].x) * f;
            cy += (coords[j].y + coords[i].y) * f;
            j = i;
        }
        let area = twice_area / 2.0;
        if area == 0.0 {
            return (0.0, 0.0, 0.0);
        }
        (area, cx / (3.0 * twice_area), cy / (3.0 * twice_area))
    }"""
new_func = """    #[inline]
    fn ring_area_and_centroid_2d(coords: &[Coord3D]) -> (f64, f64, f64) {
        if coords.len() < 3 {
            return (0.0, 0.0, 0.0);
        }
        let origin_x = coords[0].x;
        let origin_y = coords[0].y;
        let mut twice_area = 0.0;
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut j = coords.len() - 1;
        for i in 0..coords.len() {
            let p1_x = coords[j].x - origin_x;
            let p1_y = coords[j].y - origin_y;
            let p2_x = coords[i].x - origin_x;
            let p2_y = coords[i].y - origin_y;
            let f = p1_x * p2_y - p2_x * p1_y;
            twice_area += f;
            cx += (p1_x + p2_x) * f;
            cy += (p1_y + p2_y) * f;
            j = i;
        }
        let area = twice_area / 2.0;
        if area == 0.0 {
            return (0.0, 0.0, 0.0);
        }
        (area, cx / (3.0 * twice_area) + origin_x, cy / (3.0 * twice_area) + origin_y)
    }"""

if old_func in content:
    content = content.replace(old_func, new_func)
    open("crates/geo-polygonize-core/src/types.rs", "w").write(content)
    print("Replaced successfully")
else:
    print("Could not find block")
