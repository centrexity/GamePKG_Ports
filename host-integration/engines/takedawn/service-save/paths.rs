// Bounded value-only path segmentation. All actual containment stays in the
// host; these checked slices avoid out-of-line string-search/panic imports.
pub fn save_path_segments(path: &str) -> Result<std::vec::Vec<&str>, i32> {
    if path.is_empty() || path.len() > 4096 {
        return Err(-1);
    }
    let mut parts = std::vec::Vec::new();
    let mut start = 0usize;
    for (index, byte) in path.bytes().enumerate() {
        if byte == b'/' {
            if index == start {
                return Err(-1);
            }
            parts.push(path.get(start..index).ok_or(-1)?);
            start = index + 1;
        }
    }
    if start == path.len() {
        return Err(-1);
    }
    parts.push(path.get(start..).ok_or(-1)?);
    Ok(parts)
}
