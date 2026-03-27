with open("crates/geo-polygonize-core/src/containment.rs", "r") as f:
    content = f.read()

content = content.replace(
"""        for cand_idx in candidates {
            let idx = cand_idx;
            let simd_shell = &self.simd_shells[idx];

            if simd_shell.contains(probe_point.0) {
                // If it contains the hole and is the smallest containing shell found so far
                if area > hole_area + 1e-6 && area < min_area {""",
"""        for cand_idx in candidates {
            let idx = cand_idx;
            let simd_shell = self.simd_shells[idx].as_ref().unwrap();

            if simd_shell.contains(probe_point.0) {
                let area = self.shell_areas[idx].unwrap();
                // If it contains the hole and is the smallest containing shell found so far
                if area > hole_area + 1e-6 && area < min_area {""")

content = content.replace(
"""        for cand_idx in candidates {
            let idx = cand_idx;
            let simd_shell = &self.simd_shells[idx];

            if simd_shell.contains(probe_point.0) {
                // Using cached areas instead of `shells[idx].exterior_unsigned_area_2d()`
                let area = self.shell_areas[idx];

                if area > hole_area + 1e-6 && area < min_area {""",
"""        for cand_idx in candidates {
            let idx = cand_idx;
            let simd_shell = self.simd_shells[idx].as_ref().unwrap();

            if simd_shell.contains(probe_point.0) {
                // Using cached areas instead of `shells[idx].exterior_unsigned_area_2d()`
                let area = self.shell_areas[idx].unwrap();

                if area > hole_area + 1e-6 && area < min_area {""")

with open("crates/geo-polygonize-core/src/containment.rs", "w") as f:
    f.write(content)
