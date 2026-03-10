import sys
with open('crates/geo-polygonize-wasm/src/lib.rs', 'r') as f:
    content = f.read()

repl = """        for j in start..end.saturating_sub(1) {
            let idx = j * stride as usize;
            let jdx = (j + 1) * stride as usize;
            let z1 = if stride == 3 { coords[idx + 2] } else { 0.0 };
            let z2 = if stride == 3 { coords[jdx + 2] } else { 0.0 };

            if !coords[idx].is_finite() || !coords[idx + 1].is_finite() || !z1.is_finite() || !coords[jdx].is_finite() || !coords[jdx + 1].is_finite() || !z2.is_finite() {
                return Err(to_js_error("InvalidGeometry", "NaN or Inf coordinates detected in buffers"));
            }

            lines.push(Line3D::new(
                Coord3D::new(coords[idx], coords[idx + 1], z1),
                Coord3D::new(coords[jdx], coords[jdx + 1], z2),
                0,
            ));
        }"""

content = content.replace("""        for j in start..end.saturating_sub(1) {
            let idx = j * stride as usize;
            let jdx = (j + 1) * stride as usize;
            let z1 = if stride == 3 { coords[idx + 2] } else { 0.0 };
            let z2 = if stride == 3 { coords[jdx + 2] } else { 0.0 };

            lines.push(Line3D::new(
                Coord3D::new(coords[idx], coords[idx + 1], z1),
                Coord3D::new(coords[jdx], coords[jdx + 1], z2),
                0,
            ));
        }""", repl)

with open('crates/geo-polygonize-wasm/src/lib.rs', 'w') as f:
    f.write(content)
