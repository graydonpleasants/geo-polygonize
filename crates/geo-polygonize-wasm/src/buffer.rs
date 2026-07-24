use geo_polygonize_core::{Coord3D, Line3D};

pub struct BufferError {
    pub name: &'static str,
    pub message: String,
}

#[doc(hidden)]
pub fn parse_buffer_lines(
    coords: &[f64],
    offsets: &[u32],
    stride: u8,
    line_ids: Option<&[u32]>,
) -> Result<Vec<Line3D>, BufferError> {
    if stride != 2 && stride != 3 {
        return Err(BufferError {
            name: "InvalidArgumentType",
            message: "stride must be 2 or 3".to_string(),
        });
    }
    if let Some(ids) = line_ids {
        if !offsets.is_empty() && ids.len() != offsets.len() {
            return Err(BufferError {
                name: "InvalidBufferShape",
                message: format!(
                    "line_ids length {} does not match line count {}",
                    ids.len(),
                    offsets.len()
                ),
            });
        }
    }

    let stride = usize::from(stride);
    let mut lines = Vec::new();
    for (i, &start) in offsets.iter().enumerate() {
        let start = start as usize;
        let end = offsets
            .get(i + 1)
            .map_or(coords.len() / stride, |&offset| offset as usize);
        if start > end {
            return Err(BufferError {
                name: "InvalidInput",
                message: format!(
                    "Invalid offsets: start offset ({start}) is greater than end offset ({end}) at index {i}"
                ),
            });
        }
        let coordinate_end = end.checked_mul(stride).ok_or_else(|| BufferError {
            name: "InvalidArgumentType",
            message: "Invalid offsets: calculated end offset exceeds coordinate capacity"
                .to_string(),
        })?;
        if coordinate_end > coords.len() {
            return Err(BufferError {
                name: "InvalidArgumentType",
                message: format!(
                    "Invalid offsets: calculated end offset {coordinate_end} exceeds coordinate capacity {} for stride {stride}",
                    coords.len()
                ),
            });
        }

        let line_id = line_ids.map_or(0, |ids| ids[i]);
        for j in start..end.saturating_sub(1) {
            let index = j * stride;
            let next = (j + 1) * stride;
            lines.push(Line3D::new(
                Coord3D::new(
                    coords[index],
                    coords[index + 1],
                    if stride == 3 { coords[index + 2] } else { 0.0 },
                ),
                Coord3D::new(
                    coords[next],
                    coords[next + 1],
                    if stride == 3 { coords[next + 2] } else { 0.0 },
                ),
                line_id,
            ));
        }
    }
    Ok(lines)
}
