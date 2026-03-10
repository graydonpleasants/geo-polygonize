import sys
with open('crates/geo-polygonize-core/src/arrow_api.rs', 'r') as f:
    content = f.read()

# Make sure we check for NaN or Inf. The plan says "Reject completely malformed numeric inputs (NaN/Inf) gracefully rather than letting downstream math operations panic or silently produce garbage."

process_linestring_array_repl = """fn process_linestring_array(arr: &LineStringArray, lines: &mut Vec<Line3D>) -> Result<(), String> {
    for i in 0..arr.len() {
        if let Ok(Some(geom)) = arr.get(i) {
            let ls = geom.to_line_string();
            for line in ls.lines() {
                if !line.start.x.is_finite() || !line.start.y.is_finite() || !line.end.x.is_finite() || !line.end.y.is_finite() {
                    return Err("NaN or Inf coordinates detected in LineStringArray".to_string());
                }
                let p1 = Coord3D::new(line.start.x, line.start.y, 0.0);
                let p2 = Coord3D::new(line.end.x, line.end.y, 0.0);
                lines.push(Line3D::new(p1, p2, 0));
            }
        }
    }
    Ok(())
}"""

content = content.replace("""fn process_linestring_array(arr: &LineStringArray, lines: &mut Vec<Line3D>) {
    for i in 0..arr.len() {
        if let Ok(Some(geom)) = arr.get(i) {
            let ls = geom.to_line_string();
            for line in ls.lines() {
                let p1 = Coord3D::new(line.start.x, line.start.y, 0.0);
                let p2 = Coord3D::new(line.end.x, line.end.y, 0.0);
                lines.push(Line3D::new(p1, p2, 0));
            }
        }
    }
}""", process_linestring_array_repl)

# Update calls to process_linestring_array
content = content.replace('process_linestring_array(&arr, &mut lines);', 'process_linestring_array(&arr, &mut lines)?;')


# Validate in process_list_array
process_list_array_repl = """        for j in start..end - 1 {
            let x1 = x_vals.value(j);
            let y1 = y_vals.value(j);
            let x2 = x_vals.value(j + 1);
            let y2 = y_vals.value(j + 1);

            if !x1.is_finite() || !y1.is_finite() || !x2.is_finite() || !y2.is_finite() {
                return Err("NaN or Inf coordinates detected in list array".to_string());
            }

            let p1 = Coord3D::new(x1, y1, 0.0);
            let p2 = Coord3D::new(x2, y2, 0.0);
            lines.push(Line3D::new(p1, p2, 0));
        }"""

content = content.replace("""        for j in start..end - 1 {
            let x1 = x_vals.value(j);
            let y1 = y_vals.value(j);
            let x2 = x_vals.value(j + 1);
            let y2 = y_vals.value(j + 1);

            let p1 = Coord3D::new(x1, y1, 0.0);
            let p2 = Coord3D::new(x2, y2, 0.0);
            lines.push(Line3D::new(p1, p2, 0));
        }""", process_list_array_repl)


with open('crates/geo-polygonize-core/src/arrow_api.rs', 'w') as f:
    f.write(content)
