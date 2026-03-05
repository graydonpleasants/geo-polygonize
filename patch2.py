import re

content = open("crates/geo-polygonize-core/src/types.rs").read()

old_func = """    pub fn centroid_2d(&self) -> Option<geo_types::Point<f64>> {
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut total_area = 0.0;

        let (ext_area, ext_cx, ext_cy) = Self::ring_area_and_centroid_2d(&self.exterior);
        if ext_area.abs() < 1e-12 {
            return None;
        }
        total_area += ext_area;
        cx += ext_cx * ext_area;
        cy += ext_cy * ext_area;

        for hole in &self.interiors {
            let (hole_area, hole_cx, hole_cy) = Self::ring_area_and_centroid_2d(hole);
            // Hole area is expected to have opposite sign of exterior if CCW/CW conventions are met.
            // We just add them up. If winding order isn't perfect, use signed area directly.
            total_area += hole_area;
            cx += hole_cx * hole_area;
            cy += hole_cy * hole_area;
        }

        if total_area.abs() < 1e-12 {
            None
        } else {
            Some(geo_types::Point::new(cx / total_area, cy / total_area))
        }
    }"""

new_func = """    pub fn centroid_2d(&self) -> Option<geo_types::Point<f64>> {
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut total_area = 0.0;

        let (ext_area, ext_cx, ext_cy) = Self::ring_area_and_centroid_2d(&self.exterior);
        let ext_abs_area = ext_area.abs();
        if ext_abs_area < 1e-12 {
            return None;
        }
        total_area += ext_abs_area;
        cx += ext_cx * ext_abs_area;
        cy += ext_cy * ext_abs_area;

        for hole in &self.interiors {
            let (hole_area, hole_cx, hole_cy) = Self::ring_area_and_centroid_2d(hole);
            let hole_abs_area = hole_area.abs();
            // Subtract holes based on their structural role, independent of winding order.
            total_area -= hole_abs_area;
            cx -= hole_cx * hole_abs_area;
            cy -= hole_cy * hole_abs_area;
        }

        if total_area.abs() < 1e-12 {
            None
        } else {
            Some(geo_types::Point::new(cx / total_area, cy / total_area))
        }
    }"""

if old_func in content:
    content = content.replace(old_func, new_func)
    open("crates/geo-polygonize-core/src/types.rs", "w").write(content)
    print("Replaced successfully")
else:
    print("Could not find block")
