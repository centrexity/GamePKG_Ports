// Font-aware normalization adapted for this pinned font from HarfBuzz.
// Copyright 2011,2012 Google, Inc.; assets/LICENSE-HARFBUZZ.txt.
// Pinned Roboto Medium 26 layout, independently checked against SDL_ttf.
// Font-aware canonical composition, selected font substitutions and GPOS kern.
use crate::dynamic_font_data;
fn unicode_word(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}
fn unicode_find(data: &[u8], stride: usize, key: u32) -> usize {
    let mut lower = 0;
    let mut upper = data.len() / stride;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        if unicode_word(data, middle * stride) < key {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    lower * stride
}
fn unicode_nominal(cp: u32) -> u32 {
    let data = dynamic_font_data::unicode_cmap();
    let at = unicode_find(data, 8, cp);
    if at < data.len() && unicode_word(data, at) == cp {
        unicode_word(data, at + 4)
    } else {
        0
    }
}
fn unicode_class(cp: u32) -> u32 {
    if cp < 0x300 {
        return 0;
    }
    let data = dynamic_font_data::unicode_classes();
    let at = unicode_find(data, 8, cp);
    if at < data.len() && unicode_word(data, at) == cp {
        unicode_word(data, at + 4)
    } else {
        0
    }
}
fn unicode_decompose(cp: u32, points: &mut Vec<u32>) {
    if cp < 0xc0 {
        unicode_append(cp, points);
        return;
    }
    let data = dynamic_font_data::unicode_decomp();
    let at = unicode_find(data, 12, cp);
    if at < data.len() && unicode_word(data, at) == cp {
        let first = unicode_word(data, at + 4);
        let second = unicode_word(data, at + 8);
        if unicode_nominal(first) != 0 && (second == 0 || unicode_nominal(second) != 0) {
            unicode_decompose(first, points);
            if second != 0 {
                unicode_decompose(second, points);
            }
            return;
        }
    }
    unicode_append(cp, points);
}
fn unicode_append(cp: u32, points: &mut Vec<u32>) {
    points.push(cp);
    let class_value = unicode_class(cp & 0x1fffff);
    let mut at = points.len() - 1;
    if class_value != 0 {
        while at > 0 && unicode_class(points[at - 1] & 0x1fffff) > class_value {
            points.swap(at - 1, at);
            at -= 1;
        }
    }
}
fn unicode_mark_char(cp: u32) -> bool {
    if cp < 0x300 {
        return false;
    }
    let data = dynamic_font_data::unicode_mark_chars();
    let at = unicode_find(data, 8, cp);
    if at < data.len() && unicode_word(data, at) == cp {
        return true;
    }
    at >= 8 && cp <= unicode_word(data, at - 4)
}
fn unicode_variation(cp: u32) -> bool {
    (0x180b..=0x180d).contains(&cp)
        || cp == 0x180f
        || (0xfe00..=0xfe0f).contains(&cp)
        || (0xe0100..=0xe01ef).contains(&cp)
}
fn unicode_zero_glyph(glyph_id: u32) -> bool {
    (glyph_id & 0xffff) as usize == dynamic_font_data::UNICODE_GLYPH_COUNT - 1
}
fn unicode_next_component(points: &[u32], start: usize, blocked: u32) -> usize {
    let mut index = start;
    while index < points.len() && unicode_zero_glyph(points[index]) && points[index] & blocked == 0
    {
        index += 1;
    }
    index
}

fn unicode_pair(data: &[u8], first: u32, second: u32) -> usize {
    let mut lower = 0;
    let mut upper = data.len() / 12;
    let wanted = (u64::from(first) << 32) | u64::from(second);
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let key = (u64::from(unicode_word(data, middle * 12)) << 32)
            | u64::from(unicode_word(data, middle * 12 + 4));
        if key < wanted {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    let at = lower * 12;
    if at < data.len() && unicode_word(data, at) == first && unicode_word(data, at + 4) == second {
        at
    } else {
        data.len()
    }
}
fn unicode_composition(first: u32, second: u32) -> u32 {
    let data = dynamic_font_data::unicode_compose();
    let at = unicode_pair(data, first, second);
    if at < data.len() {
        let cp = unicode_word(data, at + 8);
        if unicode_nominal(cp) != 0 {
            return cp;
        }
    }
    0
}
fn unicode_invisible(cp: u32) -> bool {
    let data = dynamic_font_data::unicode_ignore();
    let at = unicode_find(data, 8, cp);
    if at < data.len() && unicode_word(data, at) == cp {
        return true;
    }
    at >= 8 && cp <= unicode_word(data, at - 4)
}
fn unicode_is_mark(glyph_id: u32) -> bool {
    let data = dynamic_font_data::unicode_marks();
    let at = unicode_find(data, 8, glyph_id);
    at < data.len() && unicode_word(data, at) == glyph_id
}
pub fn unicode_shape(content: &str) -> Option<Vec<u32>> {
    let mut points = Vec::new();
    let mut characters = content.chars().peekable();
    let mut cluster = Vec::new();
    while let Some(character) = characters.next() {
        if character == '\0' {
            return None;
        }
        cluster.clear();
        cluster.push(character as u32);
        while let Some(next) = characters.peek() {
            if !unicode_mark_char(*next as u32) {
                break;
            }
            cluster.push(*next as u32);
            characters.next();
        }
        let mut raw_cluster = false;
        for cp in &cluster {
            if unicode_variation(*cp) {
                raw_cluster = true;
            }
        }
        for cp in &cluster {
            if raw_cluster {
                unicode_append(*cp | 0x80000000, &mut points);
            } else {
                unicode_decompose(*cp, &mut points);
            }
        }
    }
    let mut used = 0;
    let mut starter = 0;
    let mut previous_class = 0;
    for read in 0..points.len() {
        let encoded = points[read];
        let cp = encoded & 0x1fffff;
        let class_value = unicode_class(cp);
        if used > 0
            && unicode_mark_char(cp)
            && (previous_class < class_value || previous_class == 0)
        {
            let combined = unicode_composition(points[starter] & 0x1fffff, cp);
            if combined != 0 {
                points[starter] = combined | (points[starter] & 0x80000000);
                continue;
            }
        }
        if class_value == 0 {
            starter = used;
        }
        points[used] = encoded;
        used += 1;
        previous_class = class_value;
    }
    points.truncate(used);
    let mut previous_unicode_class = 0;
    for index in 0..points.len() {
        let encoded = points[index];
        let cp = encoded & 0x1fffff;
        let glyph_id = if unicode_invisible(cp) {
            let mut flags = if cp == 0x200c { 0x10000000 } else { 0 };
            if cp == 0x034f && index > 0 && index + 1 < points.len() {
                let following_class = unicode_class(points[index + 1] & 0x1fffff);
                if following_class != 0 && previous_unicode_class > following_class {
                    flags |= 0x20000000;
                }
            }
            (dynamic_font_data::UNICODE_GLYPH_COUNT - 1) as u32 | flags
        } else {
            let space_data = dynamic_font_data::unicode_space_fallback();
            let space_at = unicode_find(space_data, 8, cp);
            if encoded & 0x80000000 == 0
                && space_at < space_data.len()
                && unicode_word(space_data, space_at) == cp
            {
                unicode_word(space_data, space_at + 4)
            } else {
                unicode_nominal(cp)
            }
        };
        previous_unicode_class = unicode_class(cp);
        points[index] = glyph_id;
    }
    let context_marks = dynamic_font_data::unicode_context_marks();
    let context_map = dynamic_font_data::unicode_context_map();
    for index in 0..points.len() {
        let following = unicode_next_component(&points, index + 1, 0x20000000);
        if following < points.len() {
            let next = unicode_find(context_marks, 8, points[following]);
            if next < context_marks.len() && unicode_word(context_marks, next) == points[following]
            {
                let found = unicode_find(context_map, 8, points[index]);
                if found < context_map.len() && unicode_word(context_map, found) == points[index] {
                    points[index] = unicode_word(context_map, found + 4);
                }
            }
        }
    }
    let rules = dynamic_font_data::unicode_ligatures();
    let mut index = 0;
    while index < points.len() {
        let mut at = unicode_find(rules, 16, points[index]);
        while at < rules.len() && unicode_word(rules, at) == points[index] {
            let second = unicode_word(rules, at + 4);
            let third = unicode_word(rules, at + 8);
            let next = unicode_next_component(&points, index + 1, 0x30000000);
            let following = unicode_next_component(&points, next.saturating_add(1), 0x30000000);
            if next < points.len()
                && points[next] == second
                && (third == 0 || (following < points.len() && points[following] == third))
            {
                points[index] = unicode_word(rules, at + 12);
                if third != 0 {
                    points.remove(following);
                }
                points.remove(next);
                break;
            }
            at += 16;
        }
        index += 1;
    }
    Some(points)
}

fn unicode_metrics(glyph_id: u32) -> [i32; 7] {
    let data = dynamic_font_data::unicode_atlas_metadata();
    let glyph_id = glyph_id & 0xffff;
    let index = if (glyph_id as usize) < dynamic_font_data::UNICODE_GLYPH_COUNT {
        glyph_id as usize
    } else {
        0
    };
    let at = index * 20;
    [
        i16::from_le_bytes([data[at], data[at + 1]]) as i32,
        i16::from_le_bytes([data[at + 2], data[at + 3]]) as i32,
        u16::from_le_bytes([data[at + 4], data[at + 5]]) as i32,
        u16::from_le_bytes([data[at + 6], data[at + 7]]) as i32,
        unicode_word(data, at + 8) as i32,
        unicode_word(data, at + 12) as i32,
        unicode_word(data, at + 16) as i32,
    ]
}
fn unicode_advance(points: &[u32], index: usize) -> i32 {
    let glyph_id = points[index];
    let mut glyph_advance = unicode_metrics(glyph_id)[4];
    if !unicode_is_mark(glyph_id) && !unicode_zero_glyph(glyph_id) {
        let mut next = index + 1;
        while next < points.len()
            && (unicode_is_mark(points[next])
                || unicode_zero_glyph(points[next]) && points[next] & 0x20000000 == 0)
        {
            next += 1;
        }
        if next < points.len() {
            let data = dynamic_font_data::unicode_kern();
            let at = unicode_pair(data, glyph_id, points[next]);
            if at < data.len() {
                glyph_advance += unicode_word(data, at + 8) as i32;
            }
        }
    }
    glyph_advance
}
pub struct DynamicLabel {
    points: Vec<u32>,
    pub width: i32,
    pub height: i32,
    origin_x: i32,
    origin_y: i32,
}
pub fn unicode_layout(content: &str) -> Option<DynamicLabel> {
    let points = unicode_shape(content)?;
    let mut x = 0i32;
    let mut min_x = 0;
    let mut max_x = 0;
    let mut min_y = 0;
    let mut max_y = dynamic_font_data::UNICODE_HEIGHT;
    for index in 0..points.len() {
        let metrics = unicode_metrics(points[index]);
        let left = x.div_euclid(64) + metrics[0];
        let top = dynamic_font_data::UNICODE_ASCENT - metrics[1];
        min_x = min_x.min(left);
        max_x = max_x.max(left + metrics[2]);
        min_y = min_y.min(top);
        max_y = max_y.max(top + metrics[3]);
        x += unicode_advance(&points, index);
    }
    max_x = max_x.max(x.div_euclid(64));
    Some(DynamicLabel {
        points,
        width: max_x - min_x,
        height: max_y - min_y,
        origin_x: min_x,
        origin_y: min_y,
    })
}
pub fn unicode_raster_slice(
    label: &DynamicLabel,
    color: [u8; 3],
    slice_left: i32,
    slice_width: i32,
) -> Vec<u8> {
    let actual_width = slice_width.clamp(0, 1282);
    let actual_height = label.height.clamp(0, 512);
    let length = actual_width as usize * actual_height as usize * 4;
    let mut pixels = Vec::with_capacity(length);
    if length > 0 {
        pixels.extend_from_slice(&[color[0], color[1], color[2], 0]);
        while pixels.len() < length {
            let count = pixels.len().min(length - pixels.len());
            pixels.extend_from_within(0..count);
        }
    }
    let data = dynamic_font_data::unicode_atlas_pixels();
    let mut x = 0i32;
    for index in 0..label.points.len() {
        let metrics = unicode_metrics(label.points[index]);
        let left = x.div_euclid(64) + metrics[0] - label.origin_x - slice_left;
        let top = dynamic_font_data::UNICODE_ASCENT - metrics[1] - label.origin_y;
        if left < actual_width && left + metrics[2] > 0 && metrics[2] > 0 {
            let mut offset = 0;
            let mut at = metrics[5] as usize;
            let end = at + metrics[6] as usize;
            while at < end {
                let run_end = offset + i32::from(data[at]);
                // Transparent runs leave the prefilled hidden RGB unchanged.
                if data[at + 1] == 0 {
                    offset = run_end;
                }
                while offset < run_end {
                    let column = offset % metrics[2];
                    let py = top + offset / metrics[2];
                    let count = (metrics[2] - column).min(run_end - offset);
                    if py >= 0 && py < actual_height {
                        let first = (left + column).max(0);
                        let last = (left + column + count).min(actual_width);
                        for px in first..last {
                            pixels[(py as usize * actual_width as usize + px as usize) * 4 + 3] |=
                                data[at + 1];
                        }
                    }
                    offset += count;
                }
                at += 2;
            }
        }
        x += unicode_advance(&label.points, index);
    }
    pixels
}
