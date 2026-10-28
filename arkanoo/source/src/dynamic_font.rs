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
    while let Some(character) = characters.next() {
        if character == '\0' {
            return None;
        }
        // Inspect the cluster without retaining a second copy of its scalars.
        // Variation selectors affect the complete cluster, including its base.
        let mut lookahead = characters.clone();
        let mut raw_cluster = unicode_variation(character as u32);
        while let Some(next) = lookahead.peek() {
            if !unicode_mark_char(*next as u32) {
                break;
            }
            raw_cluster |= unicode_variation(*next as u32);
            lookahead.next();
        }
        if raw_cluster {
            unicode_append(character as u32 | 0x80000000, &mut points);
        } else {
            unicode_decompose(character as u32, &mut points);
        }
        while let Some(next) = characters.peek() {
            if !unicode_mark_char(*next as u32) {
                break;
            }
            let cp = *next as u32;
            if raw_cluster {
                unicode_append(cp | 0x80000000, &mut points);
            } else {
                unicode_decompose(cp, &mut points);
            }
            characters.next();
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
    // Exact pen and ink bounds for each group of 64 shaped glyphs. Raster
    // tiles can skip offscreen groups without assuming positive advances or
    // truncating a centered name or its combining marks.
    raster_groups: Vec<[i32; 3]>,
    pub width: i32,
    pub height: i32,
    origin_x: i32,
    origin_y: i32,
}
pub fn unicode_layout(content: &str) -> Option<DynamicLabel> {
    let points = unicode_shape(content)?;
    let mut raster_groups = Vec::with_capacity(points.len().div_ceil(64));
    let mut x = 0i32;
    let mut min_x = 0;
    let mut max_x = 0;
    let mut min_y = 0;
    let mut max_y = dynamic_font_data::UNICODE_HEIGHT;
    for index in 0..points.len() {
        if index % 64 == 0 {
            raster_groups.push([x, i32::MAX, i32::MIN]);
        }
        let metrics = unicode_metrics(points[index]);
        let left = x.div_euclid(64) + metrics[0];
        let top = dynamic_font_data::UNICODE_ASCENT - metrics[1];
        if metrics[2] > 0 && metrics[3] > 0 {
            let group = &mut raster_groups[index / 64];
            group[1] = group[1].min(left);
            group[2] = group[2].max(left + metrics[2]);
        }
        min_x = min_x.min(left);
        max_x = max_x.max(left + metrics[2]);
        min_y = min_y.min(top);
        max_y = max_y.max(top + metrics[3]);
        x += unicode_advance(&points, index);
    }
    max_x = max_x.max(x.div_euclid(64));
    Some(DynamicLabel {
        points,
        raster_groups,
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
    for (group_index, group) in label.raster_groups.iter().enumerate() {
        if group[1] >= group[2]
            || i64::from(group[1]) - i64::from(label.origin_x) - i64::from(slice_left)
                >= i64::from(actual_width)
            || i64::from(group[2]) - i64::from(label.origin_x) - i64::from(slice_left) <= 0
        {
            continue;
        }
        let mut x = group[0];
        for index in group_index * 64..((group_index + 1) * 64).min(label.points.len()) {
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
                                pixels[(py as usize * actual_width as usize + px as usize) * 4
                                    + 3] |= data[at + 1];
                            }
                        }
                        offset += count;
                    }
                    at += 2;
                }
            }
            x += unicode_advance(&label.points, index);
        }
    }
    pixels
}

// Derive a conservative visible-prefix bound from this immutable font, rather
// than imposing a text-length limit. Unproven shaping cases retain the full path.
static ASCII_VIEWPORT_BOUNDS: std::sync::Mutex<Option<[i32; 2]>> = std::sync::Mutex::new(None);
fn ascii_viewport_bounds() -> Option<[i32; 2]> {
    let Ok(mut cached) = ASCII_VIEWPORT_BOUNDS.lock() else {
        return None;
    };
    if let Some(font_bounds) = *cached {
        return if font_bounds[0] > 0 {
            Some(font_bounds)
        } else {
            None
        };
    }
    let mut glyphs = Vec::new();
    for byte in 32u8..=126 {
        let encoded = [byte];
        let viewport_text = str::from_utf8(&encoded).ok()?;
        let points = unicode_shape(viewport_text)?;
        if points.len() != 1 || unicode_is_mark(points[0]) || unicode_zero_glyph(points[0]) {
            return None;
        }
        if !glyphs.contains(&points[0]) {
            glyphs.push(points[0]);
        }
    }
    let context = dynamic_font_data::unicode_context_marks();
    for viewport_glyph in &glyphs {
        let at = unicode_find(context, 8, *viewport_glyph);
        if at < context.len() && unicode_word(context, at) == *viewport_glyph {
            return None;
        }
    }
    let rules = dynamic_font_data::unicode_ligatures();
    for at in (0..rules.len()).step_by(16) {
        if glyphs.contains(&unicode_word(rules, at))
            && glyphs.contains(&unicode_word(rules, at + 4))
            && (unicode_word(rules, at + 8) == 0 || glyphs.contains(&unicode_word(rules, at + 8)))
        {
            let viewport_glyph = unicode_word(rules, at + 12);
            if !glyphs.contains(&viewport_glyph) {
                glyphs.push(viewport_glyph);
            }
        }
    }
    let mut minimum_step = i32::MAX;
    let mut bearing = 0;
    for viewport_glyph in &glyphs {
        let metrics = unicode_metrics(*viewport_glyph);
        let top = dynamic_font_data::UNICODE_ASCENT - metrics[1];
        if top < 0 || top + metrics[3] > dynamic_font_data::UNICODE_HEIGHT {
            *cached = Some([0, 0]);
            return None;
        }
        bearing = bearing.min(metrics[0]);
        minimum_step = minimum_step.min(unicode_advance(&[*viewport_glyph], 0));
        for following in &glyphs {
            minimum_step = minimum_step.min(unicode_advance(&[*viewport_glyph, *following], 0));
        }
    }
    *cached = Some([minimum_step, bearing]);
    if minimum_step > 0 {
        Some([minimum_step, bearing])
    } else {
        None
    }
}
// The table contains every scalar whose single-character normalization can
// differ from the nominal missing glyph. Never retain a copy of input names.
static UNICODE_VIEWPORT_SCALARS: std::sync::Mutex<Option<Vec<[u32; 3]>>> =
    std::sync::Mutex::new(None);
fn viewport_singleton(cp: u32) -> u32 {
    if cp == 0
        || unicode_mark_char(cp)
        || unicode_class(cp) != 0
        || unicode_invisible(cp)
        || unicode_variation(cp)
    {
        return u32::MAX;
    }
    let Some(character) = char::from_u32(cp) else {
        return u32::MAX;
    };
    let mut encoded = [0u8; 4];
    let viewport_text = character.encode_utf8(&mut encoded);
    let Some(points) = unicode_shape(viewport_text) else {
        return u32::MAX;
    };
    if points.len() != 1 {
        return u32::MAX;
    }
    let viewport_glyph = points[0];
    let context = dynamic_font_data::unicode_context_marks();
    let at = unicode_find(context, 8, viewport_glyph);
    if unicode_is_mark(viewport_glyph)
        || unicode_zero_glyph(viewport_glyph)
        || at < context.len() && unicode_word(context, at) == viewport_glyph
    {
        u32::MAX
    } else {
        viewport_glyph
    }
}
// Approve only compositions whose exact two-scalar normalization produces
// the font's single nominal result; do not assume Unicode category is enough.
fn viewport_composition(first: u32, second: u32) -> u32 {
    let combined = unicode_composition(first, second);
    if combined == 0 {
        return u32::MAX;
    }
    let Some(base_character) = char::from_u32(first) else {
        return u32::MAX;
    };
    let Some(mark_character) = char::from_u32(second) else {
        return u32::MAX;
    };
    let mut encoded = [0u8; 8];
    let first_length = base_character.encode_utf8(&mut encoded).len();
    let second_length = mark_character
        .encode_utf8(&mut encoded[first_length..])
        .len();
    let Ok(viewport_text) = str::from_utf8(&encoded[..first_length + second_length]) else {
        return u32::MAX;
    };
    let Some(points) = unicode_shape(viewport_text) else {
        return u32::MAX;
    };
    if points.len() == 1
        && points[0] == unicode_nominal(combined)
        && viewport_singleton(combined) == points[0]
    {
        combined
    } else {
        u32::MAX
    }
}
// A selector may be ignored in the bounds only if this exact pinned shaper
// emits an unflagged, zero-metric glyph that kerning/ligature lookahead skips.
fn viewport_variation(cp: u32) -> bool {
    if !unicode_variation(cp) || !unicode_invisible(cp) || unicode_class(cp) != 0 {
        return false;
    }
    let Some(character) = char::from_u32(cp) else {
        return false;
    };
    let mut encoded = [0u8; 5];
    encoded[0] = b'A';
    let length = character.encode_utf8(&mut encoded[1..]).len();
    let Ok(viewport_text) = str::from_utf8(&encoded[..length + 1]) else {
        return false;
    };
    let Some(points) = unicode_shape(viewport_text) else {
        return false;
    };
    if points.len() != 2
        || points[0] != unicode_nominal(65)
        || points[1] != (dynamic_font_data::UNICODE_GLYPH_COUNT - 1) as u32
    {
        return false;
    }
    let metrics = unicode_metrics(points[1]);
    metrics[0] == 0 && metrics[1] == 0 && metrics[2] == 0 && metrics[3] == 0 && metrics[4] == 0
}
fn unicode_viewport_limit(parts: &[&str], right: i32, total: usize) -> usize {
    let Ok(mut cached) = UNICODE_VIEWPORT_SCALARS.lock() else {
        return total;
    };
    if cached.is_none() {
        let tables = [
            dynamic_font_data::unicode_cmap(),
            dynamic_font_data::unicode_decomp(),
            dynamic_font_data::unicode_space_fallback(),
        ];
        let strides = [8usize, 12, 8];
        let mut positions = [0usize; 3];
        let mut scalars = Vec::new();
        loop {
            let mut cp = u32::MAX;
            for index in 0..3 {
                if positions[index] < tables[index].len() {
                    cp = cp.min(unicode_word(tables[index], positions[index]));
                }
            }
            if cp == u32::MAX {
                break;
            }
            let viewport_glyph = viewport_singleton(cp);
            let identity = if viewport_glyph != u32::MAX && viewport_glyph == unicode_nominal(cp) {
                cp
            } else {
                u32::MAX
            };
            scalars.push([cp, viewport_glyph, identity]);
            for index in 0..3 {
                if positions[index] < tables[index].len()
                    && unicode_word(tables[index], positions[index]) == cp
                {
                    positions[index] += strides[index];
                }
            }
        }
        *cached = Some(scalars);
    }
    let Some(scalars) = cached.as_ref() else {
        return total;
    };
    let mut recent = [[u32::MAX; 3]; 16];
    let mut compositions = [[u32::MAX; 4]; 16];
    // Repeated identifiers reuse exact scalar properties. Cache collisions only
    // replace an entry; every hit checks the complete scalar before reuse.
    let mut properties = [[u32::MAX; 4]; 16];
    let mut selectors = [[u32::MAX; 2]; 16];
    let mut variation_tail = false;
    let mut used_glyphs = [false; dynamic_font_data::UNICODE_GLYPH_COUNT];
    let mut current_glyph = u32::MAX;
    let mut current_scalar = u32::MAX;
    let mut previous_mark_class = 0;
    let mut cluster_length = 0usize;
    let mut maximum_cluster = 1usize;
    for part in parts {
        for character in part.chars() {
            let cp = character as u32;
            let property_bucket = ((cp ^ (cp >> 4)) % 16) as usize;
            if properties[property_bucket][0] != cp {
                properties[property_bucket] = [
                    cp,
                    u32::from(unicode_mark_char(cp)),
                    unicode_class(cp),
                    u32::from(unicode_invisible(cp)),
                ];
            }
            let descriptor = properties[property_bucket];
            if descriptor[1] != 0 {
                if unicode_variation(cp) {
                    if current_scalar == u32::MAX {
                        return total;
                    }
                    let bucket = (cp % 16) as usize;
                    let valid = if selectors[bucket][0] == cp {
                        selectors[bucket][1] != 0
                    } else {
                        let accepted = viewport_variation(cp);
                        selectors[bucket] = [cp, u32::from(accepted)];
                        accepted
                    };
                    if !valid {
                        return total;
                    }
                    variation_tail = true;
                    cluster_length += 1;
                    continue;
                }
                let mark_class = descriptor[2];
                if current_scalar == u32::MAX
                    || variation_tail
                    || mark_class == 0
                    || mark_class < previous_mark_class
                    || unicode_variation(cp)
                    || descriptor[3] != 0
                {
                    return total;
                }
                let bucket = (current_scalar.wrapping_add(cp) % 16) as usize;
                let combined =
                    if compositions[bucket][0] == current_scalar && compositions[bucket][1] == cp {
                        compositions[bucket][2]
                    } else {
                        let found = viewport_composition(current_scalar, cp);
                        compositions[bucket] = [current_scalar, cp, found, unicode_nominal(found)];
                        found
                    };
                if combined == u32::MAX {
                    return total;
                }
                current_scalar = combined;
                current_glyph = compositions[bucket][3];
                previous_mark_class = mark_class;
                cluster_length += 1;
                continue;
            }
            if current_glyph != u32::MAX {
                used_glyphs[current_glyph as usize] = true;
                maximum_cluster = maximum_cluster.max(cluster_length);
            }
            let bucket = (cp % 16) as usize;
            let descriptor = if recent[bucket][0] == cp {
                recent[bucket]
            } else {
                let mut lower = 0;
                let mut upper = scalars.len();
                while lower < upper {
                    let middle = lower + (upper - lower) / 2;
                    if scalars[middle][0] < cp {
                        lower = middle + 1;
                    } else {
                        upper = middle;
                    }
                }
                let found = if lower < scalars.len() && scalars[lower][0] == cp {
                    scalars[lower]
                } else if cp == 0
                    || descriptor[2] != 0
                    || descriptor[3] != 0
                    || unicode_variation(cp)
                {
                    [cp, u32::MAX, u32::MAX]
                } else {
                    [cp, 0, cp]
                };
                recent[bucket] = found;
                found
            };
            current_glyph = descriptor[1];
            current_scalar = descriptor[2];
            previous_mark_class = 0;
            variation_tail = false;
            cluster_length = 1;
            if current_glyph as usize >= used_glyphs.len() {
                return total;
            }
        }
    }
    if current_glyph != u32::MAX {
        used_glyphs[current_glyph as usize] = true;
        maximum_cluster = maximum_cluster.max(cluster_length);
    }
    viewport_glyph_limit(
        parts,
        right,
        total,
        used_glyphs,
        maximum_cluster,
        false,
        false,
    )
}
struct ViewportBudget {
    scalars: usize,
    top: i32,
    bottom: i32,
}
fn viewport_glyph_budget(
    right: i32,
    mut used_glyphs: [bool; dynamic_font_data::UNICODE_GLYPH_COUNT],
    maximum_cluster: usize,
    allow_marks: bool,
    allow_extents: bool,
) -> Option<ViewportBudget> {
    // Overapproximate every possible two/three-base ligature, including rules
    // whose result could feed a later rule. Only proven, unflagged zero
    // selectors remain after cluster composition; ligature/kerning lookahead
    // skips those without changing geometry. The cluster path additionally admits
    // proven zero-advance marks, retaining their complete visible rasters.
    let rules = dynamic_font_data::unicode_ligatures();
    loop {
        let mut changed = false;
        for at in (0..rules.len()).step_by(16) {
            let first = unicode_word(rules, at) as usize;
            let second = unicode_word(rules, at + 4) as usize;
            let third = unicode_word(rules, at + 8) as usize;
            let result = unicode_word(rules, at + 12) as usize;
            if used_glyphs[first]
                && used_glyphs[second]
                && (third == 0 || used_glyphs[third])
                && !used_glyphs[result]
            {
                used_glyphs[result] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut minimum_step = i32::MAX;
    let mut bearing = 0;
    let mut minimum_top = 0;
    let mut maximum_bottom = dynamic_font_data::UNICODE_HEIGHT;
    for (index, used) in used_glyphs.iter().enumerate() {
        if *used {
            let viewport_glyph = index as u32;
            let metrics = unicode_metrics(viewport_glyph);
            let top = dynamic_font_data::UNICODE_ASCENT - metrics[1];
            if (unicode_is_mark(viewport_glyph) && (!allow_marks || metrics[4] != 0))
                || unicode_zero_glyph(viewport_glyph)
                || (!allow_extents
                    && (top < 0 || top + metrics[3] > dynamic_font_data::UNICODE_HEIGHT))
            {
                return None;
            }
            if !unicode_is_mark(viewport_glyph) {
                minimum_step = minimum_step.min(metrics[4]);
            }
            bearing = bearing.min(metrics[0]);
            minimum_top = minimum_top.min(top);
            maximum_bottom = maximum_bottom.max(top + metrics[3]);
        }
    }
    let kern = dynamic_font_data::unicode_kern();
    for at in (0..kern.len()).step_by(12) {
        let first = unicode_word(kern, at) as usize;
        let second = unicode_word(kern, at + 4) as usize;
        if used_glyphs[first]
            && used_glyphs[second]
            && !unicode_is_mark(first as u32)
            && !unicode_is_mark(second as u32)
        {
            minimum_step = minimum_step
                .min(unicode_metrics(first as u32)[4] + unicode_word(kern, at + 8) as i32);
        }
    }
    if minimum_step <= 0 {
        return None;
    }
    let distance = (i64::from(right) - i64::from(bearing)).max(0);
    let glyphs = (distance * 64 / i64::from(minimum_step) + 4) as usize;
    let limit = glyphs
        .saturating_mul(3)
        .saturating_add(2)
        .saturating_mul(maximum_cluster);
    Some(ViewportBudget {
        scalars: limit,
        top: minimum_top,
        bottom: maximum_bottom,
    })
}
fn viewport_glyph_limit(
    parts: &[&str],
    right: i32,
    total: usize,
    used_glyphs: [bool; dynamic_font_data::UNICODE_GLYPH_COUNT],
    maximum_cluster: usize,
    allow_marks: bool,
    split_missing_marks: bool,
) -> usize {
    let Some(glyph_budget) =
        viewport_glyph_budget(right, used_glyphs, maximum_cluster, allow_marks, false)
    else {
        return total;
    };
    let limit = glyph_budget.scalars;
    let mut count = 0usize;
    let mut bytes = 0usize;
    for part in parts {
        for character in part.chars() {
            if count >= limit
                && (!unicode_mark_char(character as u32)
                    || (split_missing_marks
                        && unicode_class(character as u32) != 0
                        && unicode_nominal(character as u32) == 0))
            {
                return bytes;
            }
            bytes += character.len_utf8();
            count += 1;
        }
    }
    bytes
}

// Inspect complete small clusters once per repeated key. Large/unproven
// clusters retain full shaping; this cache never limits stored input length.
fn viewport_next_character(parts: &[&str], part: &mut usize, offset: &mut usize) -> Option<char> {
    while *part < parts.len() {
        if let Some(character) = parts[*part][*offset..].chars().next() {
            *offset += character.len_utf8();
            return Some(character);
        }
        *part += 1;
        *offset = 0;
    }
    None
}
fn viewport_missing_mark(
    base: u32,
    cp: u32,
    base_points: &[u32],
    verify_composition: bool,
    cached: &mut [[u32; 3]; 16],
) -> bool {
    let bucket = (base.wrapping_add(cp) % 16) as usize;
    if cached[bucket][0] == base && cached[bucket][1] == cp {
        return cached[bucket][2] & 1 != 0 && (!verify_composition || cached[bucket][2] & 2 != 0);
    }
    let mut accepted = unicode_nominal(cp) == 0
        && unicode_class(cp) != 0
        && !unicode_invisible(cp)
        && !unicode_variation(cp);
    if accepted {
        let data = dynamic_font_data::unicode_decomp();
        let at = unicode_find(data, 12, cp);
        if at < data.len() && unicode_word(data, at) == cp {
            let first = unicode_word(data, at + 4);
            let second = unicode_word(data, at + 8);
            if unicode_nominal(first) != 0 && (second == 0 || unicode_nominal(second) != 0) {
                accepted = false;
            }
        }
    }
    let mut uncomposed = false;
    if accepted {
        let mut encoded = [0u8; 8];
        let Some(base_character) = char::from_u32(base) else {
            return false;
        };
        let Some(mark_character) = char::from_u32(cp) else {
            return false;
        };
        let first = base_character.encode_utf8(&mut encoded).len();
        let second = mark_character.encode_utf8(&mut encoded[first..]).len();
        let Ok(pair) = str::from_utf8(&encoded[..first + second]) else {
            return false;
        };
        let Some(points) = unicode_shape(pair) else {
            return false;
        };
        // A missing font glyph can still compose with the starter (S + cedilla).
        // Check every distinct mark, including marks beyond the initial key.
        uncomposed = points.len() == base_points.len() + 1
            && points[..base_points.len()] == *base_points
            && points.last() == Some(&0);
    }
    // Low bits qualify structural/pair behavior; the upper byte caches CCC.
    // Long streaming runs can reuse that metadata instead of table-searching.
    cached[bucket] = [
        base,
        cp,
        u32::from(accepted) | (u32::from(uncomposed) << 1) | (unicode_class(cp) << 8),
    ];
    accepted && (!verify_composition || uncomposed)
}
fn cluster_viewport_limit(parts: &[&str], right: i32, total: usize) -> usize {
    let mut seen = [[u32::MAX; 16]; 16];
    let mut next_slot = 0usize;
    let mut used = [false; dynamic_font_data::UNICODE_GLYPH_COUNT];
    let mut maximum_cluster = 1usize;
    let mut split_missing_marks = false;
    let mut missing_cache = [[u32::MAX; 3]; 16];
    let mut part = 0usize;
    let mut offset = 0usize;
    let mut following = viewport_next_character(parts, &mut part, &mut offset);
    while let Some(first) = following {
        following = viewport_next_character(parts, &mut part, &mut offset);
        if unicode_mark_char(first as u32) || first == '\0' {
            return total;
        }
        let mut key = [u32::MAX; 16];
        key[0] = first as u32;
        let mut length = 1usize;
        let mut long_run = false;
        while let Some(next) = following {
            if !unicode_mark_char(next as u32) {
                break;
            }
            if length == key.len() {
                let mut base_encoded = [0u8; 4];
                let base_text = first.encode_utf8(&mut base_encoded);
                let Some(base_points) = unicode_shape(base_text) else {
                    return total;
                };
                let mut blocked_classes = [false; 256];
                for cp in &key[1..] {
                    let class = unicode_class(*cp) as usize;
                    if class >= blocked_classes.len()
                        || !viewport_missing_mark(
                            first as u32,
                            *cp,
                            &base_points,
                            false,
                            &mut missing_cache,
                        )
                    {
                        return total;
                    }
                    blocked_classes[class] = true;
                }
                split_missing_marks = true;
                long_run = true;
                while let Some(remaining) = following {
                    if !unicode_mark_char(remaining as u32) {
                        break;
                    }
                    let class = unicode_class(remaining as u32) as usize;
                    if class >= blocked_classes.len()
                        || !viewport_missing_mark(
                            first as u32,
                            remaining as u32,
                            &base_points,
                            !blocked_classes[class],
                            &mut missing_cache,
                        )
                    {
                        return total;
                    }
                    blocked_classes[class] = true;
                    following = viewport_next_character(parts, &mut part, &mut offset);
                }
                break;
            }
            key[length] = next as u32;
            length += 1;
            following = viewport_next_character(parts, &mut part, &mut offset);
        }
        maximum_cluster = maximum_cluster.max(if long_run { 1 } else { length });
        if !long_run && seen.contains(&key) {
            continue;
        }
        seen[next_slot] = key;
        next_slot = (next_slot + 1) % seen.len();
        let mut encoded = [0u8; 64];
        let mut bytes = 0usize;
        for cp in &key[..length] {
            let Some(character) = char::from_u32(*cp) else {
                return total;
            };
            bytes += character.encode_utf8(&mut encoded[bytes..]).len();
        }
        let Ok(cluster_text) = str::from_utf8(&encoded[..bytes]) else {
            return total;
        };
        let Some(points) = unicode_shape(cluster_text) else {
            return total;
        };
        if long_run {
            let mut base_encoded = [0u8; 4];
            let base_text = first.encode_utf8(&mut base_encoded);
            let Some(base_points) = unicode_shape(base_text) else {
                return total;
            };
            if points.len() != base_points.len() + length - 1
                || points[..base_points.len()] != base_points
            {
                return total;
            }
            for cluster_glyph in &points[base_points.len()..] {
                if *cluster_glyph != 0 {
                    return total;
                }
            }
            // Every extra mark leaves the starter unchanged and remains glyph 0.
            // Same-class marks are blocked by the first unconsumed mark of that
            // class, including when another class originally appeared between them.
            // Canonical reordering among these identical fallback glyphs has no
            // observable effect. Their positive advances and the complete short
            // cluster/ligature lookahead are checked by the shared bounds below.
        }
        let mut bases = 0usize;
        for cluster_glyph in points {
            if cluster_glyph as usize >= used.len() {
                return total;
            }
            if unicode_zero_glyph(cluster_glyph) {
                // Only the unflagged zero glyph is skipped by every relevant
                // lookahead. Its metrics must prove no pixels or pen motion.
                let metrics = unicode_metrics(cluster_glyph);
                if metrics[0] != 0
                    || metrics[1] != 0
                    || metrics[2] != 0
                    || metrics[3] != 0
                    || metrics[4] != 0
                {
                    return total;
                }
                continue;
            }
            if !unicode_is_mark(cluster_glyph) {
                bases += 1;
            }
            used[cluster_glyph as usize] = true;
        }
        if bases == 0 {
            return total;
        }
    }
    viewport_glyph_limit(
        parts,
        right,
        total,
        used,
        maximum_cluster,
        true,
        split_missing_marks,
    )
}
fn viewport_initial_limit(parts: &[&str], right: i32) -> [usize; 3] {
    let mut ascii = true;
    let mut total = 0usize;
    for part in parts {
        total = total.saturating_add(part.len());
        for byte in part.bytes() {
            if !(32..=126).contains(&byte) {
                ascii = false;
            }
        }
    }
    if !ascii {
        let limit = unicode_viewport_limit(parts, right, total);
        return [limit, total, 1];
    }
    let bound = ascii_viewport_bounds();
    if let Some([minimum_step, bearing]) = bound {
        // A ligature consumes at most three ASCII inputs. Leave extra complete
        // glyphs beyond the visible edge so final lookahead cannot affect it.
        let distance = (i64::from(right) - i64::from(bearing)).max(0);
        let glyphs = (distance * 64 / i64::from(minimum_step) + 4) as usize;
        [
            total.min(glyphs.saturating_mul(3).saturating_add(2)),
            total,
            0,
        ]
    } else {
        [total, total, 0]
    }
}
fn viewport_prefix_limit(parts: &[&str], right: i32) -> usize {
    let [limit, total, non_ascii] = viewport_initial_limit(parts, right);
    if non_ascii != 0 && limit == total {
        cluster_viewport_limit(parts, right, total)
    } else {
        limit
    }
}
fn viewport_approved_selector(cp: u32, cached: &mut [[u32; 2]; 16]) -> bool {
    if !unicode_variation(cp) {
        return false;
    }
    let bucket = (cp % 16) as usize;
    if cached[bucket][0] != cp {
        cached[bucket] = [cp, u32::from(viewport_variation(cp))];
    }
    cached[bucket][1] != 0
}
// Keep the final approved selector in each contiguous run. This preserves
// raw-cluster mode, its class-zero barrier and its exact final starter scalar.
// Intermediate proven unflagged zero glyphs have no pixels or advance.
// The complete original identifier remains owned by the editor/archive.
pub fn compact_selector_runs(parts: &[&str]) -> Option<String> {
    let mut cached = [[u32::MAX; 2]; 16];
    let mut total = 0usize;
    let mut removed = 0usize;
    let mut previous_length = 0usize;
    let mut previous_approved = false;
    for part in parts {
        total = total.checked_add(part.len())?;
        for character in part.chars() {
            let approved = viewport_approved_selector(character as u32, &mut cached);
            if previous_approved && approved {
                removed += previous_length;
            }
            previous_length = character.len_utf8();
            previous_approved = approved;
        }
    }
    if removed == 0 {
        return None;
    }
    let mut display_text = String::with_capacity(total - removed);
    let mut pending = None;
    for part in parts {
        for character in part.chars() {
            let approved = viewport_approved_selector(character as u32, &mut cached);
            if let Some(previous) = pending.take() {
                if !approved {
                    display_text.push(previous);
                }
            }
            if approved {
                pending = Some(character);
            } else {
                display_text.push(character);
            }
        }
    }
    if let Some(previous) = pending {
        display_text.push(previous);
    }
    Some(display_text)
}
pub fn viewport_label_slice(label: &str, right: i32) -> &str {
    let limit = viewport_prefix_limit(&[label], right);
    &label[..limit]
}
// The pinned composition graph has a maximum of two supported transitions.
// Keep those candidates plus the first blocking mark of each canonical class.
// Tests traverse the complete graph; a changed graph also fails closed below.
struct MissingCompositionRun {
    counts: [u32; 256],
    first: [[u32; 3]; 256],
    total: u32,
}
impl MissingCompositionRun {
    fn new() -> Self {
        Self {
            counts: [0; 256],
            first: [[0; 3]; 256],
            total: 0,
        }
    }
    fn add(&mut self, base: u32, cp: u32, cache: &mut [[u32; 3]; 16]) -> bool {
        let bucket = (base.wrapping_add(cp) % 16) as usize;
        let class = if cache[bucket][0] == base && cache[bucket][1] == cp {
            (cache[bucket][2] >> 8) as usize
        } else {
            unicode_class(cp) as usize
        };
        if class >= self.counts.len()
            || !viewport_missing_mark(base, cp, &[unicode_nominal(base)], false, cache)
        {
            return false;
        }
        if self.counts[class] < 3 {
            self.first[class][self.counts[class] as usize] = cp;
        }
        let Some(count) = self.counts[class].checked_add(1) else {
            return false;
        };
        let Some(total) = self.total.checked_add(1) else {
            return false;
        };
        self.counts[class] = count;
        self.total = total;
        true
    }
    fn normalize(&self, base: u32) -> Option<[u32; 2]> {
        // ASCII starters have no hidden decomposition marks to merge into the
        // class buckets. Other starters retain the existing complete shaper.
        if !(32..=126).contains(&base) {
            return None;
        }
        let mut starter = base;
        let mut previous_class = 0usize;
        let mut consumed = 0u32;
        for class in 1..self.counts.len() {
            for at in 0..self.counts[class].min(3) as usize {
                let cp = self.first[class][at];
                let combined = if previous_class < class || previous_class == 0 {
                    unicode_composition(starter, cp)
                } else {
                    0
                };
                if combined != 0 {
                    starter = combined;
                    consumed += 1;
                    if consumed > 2 {
                        return None;
                    }
                } else {
                    previous_class = class;
                    break; // All remaining equal-class marks are blocked.
                }
            }
        }
        // U+FFFF is a display-only representative of this font's glyph zero.
        // It avoids repeating the original quadratic canonical reordering.
        // Stored names never use this representation. Prove the complete glyph
        // and substitution sequence before admitting the synthetic display text.
        let mut proof = String::new();
        proof.push(char::from_u32(starter)?);
        proof.push_str("\u{ffff}\u{ffff}\u{ffff}");
        if unicode_shape(&proof)? != [unicode_nominal(starter), 0, 0, 0] {
            return None;
        }
        Some([starter, self.total.checked_sub(consumed)?])
    }
}
fn composed_missing_viewport(parts: &[&str], right: i32) -> Option<String> {
    let mut total = 0usize;
    for part in parts {
        total = total.checked_add(part.len())?;
    }
    if total <= 16 {
        return None;
    }
    let mut used = [false; dynamic_font_data::UNICODE_GLYPH_COUNT];
    let mut maximum_cluster = 1usize;
    let mut have_long = false;
    let mut seen_short = [[u32::MAX; 16]; 16];
    let mut next_short = 0usize;
    let mut budget = 0usize;
    let mut vertical = [0, dynamic_font_data::UNICODE_HEIGHT];
    let mut output = String::new();
    let mut cache = [[u32::MAX; 3]; 16];
    // First validate every cluster and derive a bound from its actual glyphs;
    // then emit only the proven visible display prefix. No full-name copy.
    for pass in 0..2 {
        let mut part = 0usize;
        let mut offset = 0usize;
        let mut following = viewport_next_character(parts, &mut part, &mut offset);
        let mut written = 0usize;
        while let Some(first) = following {
            let mut cut_inside_missing = false;
            if first == '\0' || unicode_mark_char(first as u32) {
                return None;
            }
            let after_first_part = part;
            let after_first_offset = offset;
            let mut key = [0u32; 16];
            key[0] = first as u32;
            let mut length = 1usize;
            following = viewport_next_character(parts, &mut part, &mut offset);
            while let Some(mark) = following {
                if !unicode_mark_char(mark as u32) || length == key.len() {
                    break;
                }
                key[length] = mark as u32;
                length += 1;
                following = viewport_next_character(parts, &mut part, &mut offset);
            }
            let long = if let Some(mark) = following {
                unicode_mark_char(mark as u32)
            } else {
                false
            };
            if long {
                if !(32..=126).contains(&(first as u32)) {
                    return None;
                }
                let mut run = MissingCompositionRun::new();
                for cp in &key[1..length] {
                    if !run.add(first as u32, *cp, &mut cache) {
                        return None;
                    }
                }
                while let Some(mark) = following {
                    let bucket = ((first as u32).wrapping_add(mark as u32) % 16) as usize;
                    let known = cache[bucket][0] == first as u32
                        && cache[bucket][1] == mark as u32
                        && cache[bucket][2] & 1 != 0;
                    if !known && !unicode_mark_char(mark as u32) {
                        break;
                    }
                    if !run.add(first as u32, mark as u32, &mut cache) {
                        return None;
                    }
                    following = viewport_next_character(parts, &mut part, &mut offset);
                }
                let [starter, missing] = run.normalize(first as u32)?;
                if pass == 0 {
                    have_long = true;
                    used[unicode_nominal(starter) as usize] = true;
                    if missing > 0 {
                        used[0] = true;
                    }
                } else {
                    output.push(char::from_u32(starter)?);
                    written += 1;
                    let take = (missing as usize).min(budget.saturating_sub(written));
                    let mut raw_part = after_first_part;
                    let mut raw_offset = after_first_offset;
                    for _ in 0..take {
                        if starter == first as u32 {
                            output.push(viewport_next_character(
                                parts,
                                &mut raw_part,
                                &mut raw_offset,
                            )?);
                        } else {
                            output.push('\u{ffff}');
                        }
                        written += 1;
                    }
                    cut_inside_missing = take < missing as usize && take >= 3;
                }
            } else if pass == 0 {
                // Repeated short clusters have the same pinned shaping proof.
                // Keep validating their complete source, but collect each
                // cached cluster's glyphs once instead of reshaping the name.
                if seen_short.contains(&key) {
                    continue;
                }
                seen_short[next_short] = key;
                next_short = (next_short + 1) % seen_short.len();
                let mut encoded = [0u8; 64];
                let mut bytes = 0usize;
                for cp in &key[..length] {
                    bytes += char::from_u32(*cp)?
                        .encode_utf8(&mut encoded[bytes..])
                        .len();
                }
                for point in unicode_shape(str::from_utf8(&encoded[..bytes]).ok()?)? {
                    if point as usize >= used.len() {
                        return None;
                    }
                    used[point as usize] = true;
                }
                maximum_cluster = maximum_cluster.max(length);
            } else {
                for cp in &key[..length] {
                    output.push(char::from_u32(*cp)?);
                }
                written += length;
            }
            if pass == 1 && written >= budget {
                if !cut_inside_missing {
                    return None;
                }
                let layout = unicode_layout(&output)?;
                if layout.origin_y != vertical[0] || layout.height != vertical[1] - vertical[0] {
                    return None;
                }
                return Some(output);
            }
        }
        if pass == 0 {
            if !have_long {
                return None;
            }
            let glyph_budget = viewport_glyph_budget(right, used, maximum_cluster, true, true)?;
            budget = glyph_budget.scalars;
            vertical = [glyph_budget.top, glyph_budget.bottom];
            output = String::with_capacity(budget.min(total).saturating_mul(4));
        }
    }
    Some(output)
}
pub fn viewport_label_parts(parts: &[&str], right: i32) -> String {
    if let Some(mut display_text) = compact_selector_runs(parts) {
        let limit = viewport_prefix_limit(&[&display_text], right);
        display_text.truncate(limit);
        return display_text;
    }
    let [mut limit, total, non_ascii] = viewport_initial_limit(parts, right);
    if non_ascii != 0 && limit == total {
        if let Some(display_text) = composed_missing_viewport(parts, right) {
            return display_text;
        }
        limit = cluster_viewport_limit(parts, right, total);
    }
    let mut viewport_text = String::with_capacity(limit);
    for part in parts {
        let length = part.len().min(limit - viewport_text.len());
        viewport_text.push_str(&part[..length]);
        if viewport_text.len() == limit {
            break;
        }
    }
    viewport_text
}
