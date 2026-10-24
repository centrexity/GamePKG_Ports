use crate::editor;
use crate::gamepkg;

// Read bounded pieces directly into their final allocations. Never retain a
// second full archive alongside a decoded pattern collection.
struct SaveReader {
    size: u32,
    start: u32,
    bytes: Vec<u8>,
}
impl SaveReader {
    fn read_piece(&mut self, offset: u32, bytes: &mut [u8]) -> Option<()> {
        let mut done = 0;
        while done < bytes.len() {
            let position = offset + done as u32;
            if position >= self.size {
                return None;
            }
            if self.bytes.is_empty()
                || position < self.start
                || position >= self.start + self.bytes.len() as u32
            {
                self.start = position;
                let length = (self.size - position).min(4096);
                self.bytes.resize(length as usize, 0);
                if gamepkg::save_read(position, &mut self.bytes) != Some(length) {
                    return None;
                }
            }
            let source = (position - self.start) as usize;
            let length = (self.bytes.len() - source).min(bytes.len() - done);
            bytes[done..done + length].copy_from_slice(&self.bytes[source..source + length]);
            done += length;
        }
        Some(())
    }
}
pub fn load_saved_editor(size: u32) -> Option<(editor::Editor, [u32; 3])> {
    if !(16..=1048576).contains(&size) {
        return None;
    }
    let mut reader = SaveReader {
        size,
        start: 0,
        bytes: Vec::with_capacity(4096),
    };
    let mut header = Vec::with_capacity(20);
    header.resize(16, 0);
    reader.read_piece(0, &mut header)?;
    let mut loaded = editor::Editor::new();
    if size == 16 {
        let metadata = loaded.save_decode(&header)?;
        return Some((loaded, metadata));
    }
    if size < 20 {
        return None;
    }
    header.resize(20, 0);
    reader.read_piece(16, &mut header[16..])?;
    let count = u32::from_le_bytes([header[16], header[17], header[18], header[19]]);
    if count > (size - 20) / 204 {
        return None;
    }
    header[16..20].copy_from_slice(&u32::to_le_bytes(0));
    let metadata = loaded.save_decode(&header)?;
    let mut patterns = Vec::with_capacity(count as usize);
    let mut offset = 20;
    for _ in 0..count {
        if size - offset < 204 {
            return None;
        }
        let mut word = [0; 4];
        reader.read_piece(offset, &mut word)?;
        offset += 4;
        let length = u32::from_le_bytes(word);
        if length > size - offset - 200 {
            return None;
        }
        let mut name = Vec::with_capacity(length as usize + if count == 1 { 224 } else { 0 });
        name.resize(length as usize, 0);
        reader.read_piece(offset, &mut name)?;
        offset += length;
        let name = String::from_utf8(name).ok()?;
        let mut cells = [255; 200];
        reader.read_piece(offset, &mut cells)?;
        offset += 200;
        for cell in cells {
            if cell > 8 && cell != 255 {
                return None;
            }
        }
        if !editor::Editor::pattern_insert(
            &mut patterns,
            editor::Pattern::pattern_from_cells(editor::Name::from_string(name), cells)?,
        ) {
            return None;
        }
    }
    if offset != size {
        return None;
    }
    loaded.patterns = patterns;
    Some((loaded, metadata))
}

struct SaveWriter {
    bytes: Vec<u8>,
    offset: u32,
    total: u32,
}
impl SaveWriter {
    fn append_piece(&mut self, mut bytes: &[u8]) -> bool {
        while !bytes.is_empty() {
            let length = (4096 - self.bytes.len()).min(bytes.len());
            self.bytes.extend_from_slice(&bytes[..length]);
            bytes = &bytes[length..];
            if self.bytes.len() == 4096 {
                if !gamepkg::save_chunk(self.offset, self.total, &self.bytes) {
                    return false;
                }
                self.offset += self.bytes.len() as u32;
                self.bytes.clear();
            }
        }
        true
    }
    fn finish_archive(&mut self) -> bool {
        if !self.bytes.is_empty() {
            if !gamepkg::save_chunk(self.offset, self.total, &self.bytes) {
                return false;
            }
            self.offset += self.bytes.len() as u32;
            self.bytes.clear();
        }
        self.offset == self.total
    }
}
pub fn save_stream_settings(
    edit: &editor::Editor,
    best: u32,
    unlocked: u32,
    gravity: bool,
    volume: u32,
    muted: bool,
) -> bool {
    let flags = u32::from(gravity) | (volume.min(128) << 1) | (u32::from(muted) << 9);
    save_stream_archive(edit, best, unlocked, flags, false)
}
pub fn save_stream_audio_settings(
    edit: &editor::Editor,
    best: u32,
    unlocked: u32,
    gravity: bool,
    volumes: [u32; 2],
    muted: [bool; 2],
) -> bool {
    let flags = u32::from(gravity)
        | (volumes[0].min(128) << 1)
        | (u32::from(muted[0]) << 9)
        | (volumes[1].min(128) << 10)
        | (u32::from(muted[1]) << 18);
    save_stream_archive(edit, best, unlocked, flags, true)
}
fn save_stream_archive(
    edit: &editor::Editor,
    best: u32,
    unlocked: u32,
    flags: u32,
    music: bool,
) -> bool {
    let mut total = 20usize;
    for pattern in &edit.patterns {
        total = total.saturating_add(204).saturating_add(pattern.name.len());
    }
    if total > editor::SAVE_BYTES {
        return false;
    }
    let mut output = SaveWriter {
        bytes: Vec::with_capacity(4096),
        offset: 0,
        total: total as u32,
    };
    if !output.append_piece(if music { b"ARK4" } else { b"ARK3" }) {
        return false;
    }
    for word in [best, unlocked, flags, edit.patterns.len() as u32] {
        if !output.append_piece(&u32::to_le_bytes(word)) {
            return false;
        }
    }
    for pattern in &edit.patterns {
        if !output.append_piece(&u32::to_le_bytes(pattern.name.len() as u32)) {
            return false;
        }
        for piece in pattern.name.name_segments() {
            if !output.append_piece(piece.as_bytes()) {
                return false;
            }
        }
        if !output.append_piece(&pattern.pattern_unpack()) {
            return false;
        }
    }
    output.finish_archive()
}
