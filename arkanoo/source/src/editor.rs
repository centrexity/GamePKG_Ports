// Adapted from the pinned upstream editor.rs. Storage is a scoped GamePKG blob.
// Pattern cells retain the original ASCII values: 0..8 or 255 for empty.
pub const SAVE_BYTES: usize = 1048576;
pub struct Pattern {
    pub name: String,
    pub cells: [u8; 200],
}
impl Pattern {
    pub fn ascii_read(name: &str, source: &str) -> Self {
        let mut cells = [255; 200];
        let mut row = 0;
        for line in source.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            if row >= 10 {
                break;
            }
            for (col, character) in line.chars().take(20).enumerate() {
                cells[row * 20 + col] = match character {
                    '0'..='8' => character as u8 - 48,
                    'I' => 6,
                    'E' => 7,
                    'U' => 8,
                    _ => 255,
                };
            }
            row += 1;
        }
        Self {
            name: name.to_string(),
            cells,
        }
    }
    pub fn ascii_write(&self) -> String {
        let mut content = String::from("# Pattern: ");
        content.push_str(&self.name);
        content
            .push_str("\n# * = empty, 0-5 = normal, 6 = Ice, 7 = Explosive, 8 = Undestroyable\n\n");
        for row in 0..10 {
            for col in 0..20 {
                let cell = self.cells[row * 20 + col];
                content.push(if cell <= 8 {
                    char::from(cell + 48)
                } else {
                    '*'
                });
            }
            content.push('\n');
        }
        content
    }
}
pub struct Editor {
    pub frame_count: u64,
    pub sfx_volume: u32,
    pub sfx_muted: bool,
    pub music_volume: u32,
    pub music_muted: bool,
    pub file_requested: u32,
    pub file_handle: Option<u32>,
    pub file_operation: u32,
    pub cells: [u8; 200],
    pub health: [u8; 200],
    pub selected: u8,
    pub hovered_color: u8,
    pub hovered_button: u8,
    pub name: String,
    pub naming: bool,
    pub confirm: bool,
    pub browser: bool,
    pub selection: usize,
    pub background: u32,
    pub dragging: u32,
    pub patterns: Vec<Pattern>,
    pub message: String,
    pub message_time: u32,
    pub save_requested: bool,
    backup: Option<Vec<Pattern>>,
    pub readable: bool,
}
impl Editor {
    pub fn new() -> Self {
        Self {
            frame_count: 0,
            sfx_volume: 64,
            sfx_muted: false,
            music_volume: 64,
            music_muted: false,
            file_requested: 0,
            file_handle: None,
            file_operation: 0,
            cells: [255; 200],
            health: [1; 200],
            selected: 0,
            hovered_color: 255,
            hovered_button: 255,
            name: String::from("my_pattern"),
            naming: false,
            confirm: false,
            browser: false,
            selection: 0,
            background: 1,
            dragging: 0,
            patterns: Vec::new(),
            message: String::new(),
            message_time: 0,
            save_requested: false,
            backup: None,
            readable: true,
        }
    }
    pub fn editor_message(&mut self, message: &str) {
        self.message = message.to_string();
        self.message_time = 180;
    }
    pub fn editor_tick(&mut self) {
        self.message_time = self.message_time.saturating_sub(1);
        if self.message_time == 0 {
            self.message.clear();
        }
    }
    pub fn editor_cancel(&mut self) {
        self.confirm = false;
        self.message.clear();
    }
    pub fn editor_paint(&mut self, x: f32, y: f32, erase: bool) {
        if !x.is_finite() || !y.is_finite() {
            return;
        }
        // Original SDL integer coordinates, projected from the half-size canvas.
        let px = (x * 2.0) as i32;
        let py = ((y - 60.0) * 2.0) as i32;
        if !(40..1240).contains(&px) || !(80..280).contains(&py) {
            return;
        }
        let col = ((px - 40) / 60) as usize;
        let row = ((py - 80) / 20) as usize;
        let index = row * 20 + col;
        if erase {
            self.cells[index] = 255;
        } else if self.cells[index] == 255 {
            self.cells[index] = self.selected;
            self.health[index] = if self.selected == 6 {
                2
            } else if self.selected == 8 {
                255
            } else {
                1
            };
        }
    }
    pub fn editor_clear(&mut self) {
        if self.confirm {
            self.cells = [255; 200];
            self.confirm = false;
            self.editor_message("PATTERN CLEARED");
        } else if self.editor_has_blocks() {
            self.confirm = true;
            self.editor_message("PRESS C AGAIN TO CLEAR");
        }
    }
    pub fn editor_has_blocks(&self) -> bool {
        for cell in self.cells {
            if cell != 255 {
                return true;
            }
        }
        false
    }
    pub fn editor_store(&mut self) -> bool {
        if !self.readable {
            self.editor_message("SAVE DAMAGED / PRESERVED");
            return false;
        }
        if !self.editor_has_blocks() {
            self.editor_message("ADD BLOCKS FIRST");
            return false;
        }
        // Names are identifiers inside the blob, never filesystem paths.
        // Apply only the actual host save-size bound; do not truncate names.
        let mut size = 20usize;
        for pattern in &self.patterns {
            if pattern.name != self.name {
                size = size.saturating_add(204).saturating_add(pattern.name.len());
            }
        }
        size = size.saturating_add(204).saturating_add(self.name.len());
        if size > SAVE_BYTES {
            self.editor_message("SAVE STORAGE FULL");
            return false;
        }
        if self.backup.is_none() {
            let mut previous = Vec::new();
            for pattern in &self.patterns {
                previous.push(Pattern {
                    name: pattern.name.clone(),
                    cells: pattern.cells,
                });
            }
            self.backup = Some(previous);
        }
        for pattern in &mut self.patterns {
            if pattern.name == self.name {
                pattern.cells = self.cells;
                self.save_requested = true;
                return true;
            }
        }
        Self::pattern_insert(
            &mut self.patterns,
            Pattern {
                name: self.name.clone(),
                cells: self.cells,
            },
        );
        self.save_requested = true;
        true
    }
    pub fn editor_commit(&mut self, success: bool) {
        if !self.save_requested {
            return;
        }
        if success {
            self.backup = None;
            self.editor_message("PATTERN SAVED");
        } else {
            if let Some(previous) = self.backup.take() {
                self.patterns = previous;
            }
            self.editor_message("SAVE FAILED / NOT STORED");
        }
        self.save_requested = false;
    }
    fn pattern_insert(patterns: &mut Vec<Pattern>, entry: Pattern) -> bool {
        let mut low = 0;
        let mut high = patterns.len();
        while low < high {
            let middle = low + (high - low) / 2;
            if patterns[middle].name == entry.name {
                return false;
            }
            if patterns[middle].name < entry.name {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        patterns.insert(low, entry);
        true
    }
    pub fn editor_load(&mut self) {
        if let Some(pattern) = self.patterns.get(self.selection) {
            self.cells = pattern.cells;
            self.name = pattern.name.clone();
            // The original load_pattern constructs editor blocks at health 1,
            // including ice. Newly painted ice still starts with two hits.
            self.health = [1; 200];
            self.browser = false;
            self.editor_message("PATTERN LOADED");
        }
    }
    pub fn editor_key(&mut self, key: u32) -> u32 {
        if self.naming {
            if key == 13 || key == 27 {
                self.naming = false;
            }
            if key == 8 {
                self.name.pop();
            }
            return 0;
        }
        self.editor_command(key)
    }
    pub fn editor_command(&mut self, key: u32) -> u32 {
        match key {
            105 => return 3,
            120 => return 4,
            27 => {
                self.editor_cancel();
                if self.browser {
                    self.browser = false;
                } else {
                    return 1;
                }
            }
            115 => {
                self.editor_store();
            }
            99 => self.editor_clear(),
            116 => {
                if self.editor_has_blocks() {
                    return 2;
                }
                self.editor_message("ADD BLOCKS FIRST");
            }
            108 => {
                self.browser = !self.browser;
                self.selection = 0;
                self.editor_cancel();
            }
            110 => {
                self.naming = true;
                self.editor_cancel();
            }
            48..=56 => {
                self.selected = (key - 48) as u8;
                self.editor_cancel();
            }
            0x40000052 => {
                if self.browser {
                    self.selection = self.selection.saturating_sub(1);
                }
            }
            0x40000051 => {
                if self.browser && self.selection + 1 < self.patterns.len() {
                    self.selection += 1;
                }
            }
            13 if self.browser => {
                self.editor_load();
            }
            _ => {}
        }
        0
    }
    pub fn toolbar_bounds(which: u32) -> [i32; 4] {
        if which < 5 {
            [225 + which as i32 * 170, 660, 150, 40]
        } else if which == 5 {
            [1020, 610, 120, 35]
        } else {
            [1150, 610, 120, 35]
        }
    }
    pub fn editor_event(&mut self, kind: u32, x: f32, y: f32, code: u32) -> u32 {
        if kind == 1 {
            self.hovered_color = 255;
            self.hovered_button = 255;
            if x.is_finite() && y.is_finite() {
                let px = (x * 2.0) as i32;
                let py = ((y - 60.0) * 2.0) as i32;
                for button_index in 0..7 {
                    let box_rect = Self::toolbar_bounds(button_index);
                    if (box_rect[0]..box_rect[0] + box_rect[2]).contains(&px)
                        && (box_rect[1]..box_rect[1] + box_rect[3]).contains(&py)
                    {
                        self.hovered_button = button_index as u8;
                    }
                }
                if (20..50).contains(&py) {
                    for index in 0..9 {
                        let left = 860 + index * 45;
                        if (left..left + 40).contains(&px) {
                            self.hovered_color = index as u8;
                        }
                    }
                }
            }
        }
        if kind == 0 || kind == 8 {
            self.dragging = 0;
        }
        if kind == 7 && self.naming {
            if let Some(character) = char::from_u32(code) {
                if character != '\0' && self.name.len() + character.len_utf8() + 224 <= SAVE_BYTES {
                    self.name.push(character);
                }
            }
        }
        if kind == 9 {
            return self.editor_key(code & 0x7fff_ffff);
        }
        if kind == 3 && (code == 1 || code == 2) {
            self.dragging = 0;
        }
        if kind == 2 && code == 1 {
            if (425.0..445.0).contains(&y) {
                if (20.0..95.0).contains(&x) {
                    return 3;
                }
                if (105.0..180.0).contains(&x) {
                    return 4;
                }
            }
            // Half-size equivalents of the original editor toolbar positions.
            if (390.0..410.0).contains(&y) && (112.5..527.5).contains(&x) {
                let button = ((x - 112.5) / 85.0) as u32;
                if x < 112.5 + button as f32 * 85.0 + 75.0 {
                    return self.editor_command(match button {
                        0 => 115,
                        1 => 99,
                        2 => 116,
                        3 => 108,
                        _ => 27,
                    });
                }
            }
            if self.browser {
                let first = (self.selection / 10) * 10;
                if (180.0..460.0).contains(&x) && (145.0..320.0).contains(&y) {
                    self.selection = first + ((y - 145.0) / 17.5) as usize;
                    self.editor_load();
                }
                return 0;
            }
            if (70.0..85.0).contains(&y) && (430.0..632.5).contains(&x) {
                let chosen = ((x - 430.0) / 22.5) as u8;
                if chosen < 9 && x < 430.0 + f32::from(chosen) * 22.5 + 20.0 {
                    self.selected = chosen;
                    self.editor_cancel();
                }
                return 0;
            }
            if (365.0..382.5).contains(&y) {
                if (510.0..570.0).contains(&x) {
                    self.background = if self.background <= 1 {
                        6
                    } else {
                        self.background - 1
                    };
                    return 0;
                }
                if (575.0..635.0).contains(&x) {
                    self.background = if self.background >= 6 {
                        1
                    } else {
                        self.background + 1
                    };
                    return 0;
                }
            }
            self.dragging |= 1;
            self.editor_paint(x, y, false);
        }
        if kind == 2 && code == 2 && !self.browser {
            self.dragging |= 2;
            self.editor_paint(x, y, true);
        }
        if kind == 1 && !self.browser {
            if self.dragging & 1 != 0 {
                self.editor_paint(x, y, false);
            } else if self.dragging & 2 != 0 {
                self.editor_paint(x, y, true);
            }
        }
        0
    }
    pub fn save_encode(&self, best: u32, unlocked: u32, gravity: bool) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"ARK2");
        bytes.extend_from_slice(&u32::to_le_bytes(best));
        bytes.extend_from_slice(&u32::to_le_bytes(unlocked));
        bytes.extend_from_slice(&u32::to_le_bytes(u32::from(gravity)));
        bytes.extend_from_slice(&u32::to_le_bytes(self.patterns.len() as u32));
        for pattern in &self.patterns {
            bytes.extend_from_slice(&u32::to_le_bytes(pattern.name.len() as u32));
            bytes.extend_from_slice(pattern.name.as_bytes());
            bytes.extend_from_slice(&pattern.cells);
        }
        bytes
    }
    pub fn save_encode_settings(
        &self,
        best: u32,
        unlocked: u32,
        gravity: bool,
        volume: u32,
        muted: bool,
    ) -> Vec<u8> {
        let mut bytes = self.save_encode(best, unlocked, gravity);
        bytes[0..4].copy_from_slice(b"ARK3");
        let flags = u32::from(gravity) | (volume.min(128) << 1) | (u32::from(muted) << 9);
        bytes[12..16].copy_from_slice(&u32::to_le_bytes(flags));
        bytes
    }
    pub fn save_encode_audio_settings(
        &self,
        best: u32,
        unlocked: u32,
        gravity: bool,
        volumes: [u32; 2],
        muted: [bool; 2],
    ) -> Vec<u8> {
        let mut bytes = self.save_encode_settings(best, unlocked, gravity, volumes[0], muted[0]);
        bytes[0..4].copy_from_slice(b"ARK4");
        let flags = u32::from(gravity)
            | (volumes[0].min(128) << 1)
            | (u32::from(muted[0]) << 9)
            | (volumes[1].min(128) << 10)
            | (u32::from(muted[1]) << 18);
        bytes[12..16].copy_from_slice(&u32::to_le_bytes(flags));
        bytes
    }
    pub fn save_decode(&mut self, bytes: &[u8]) -> Option<[u32; 3]> {
        if bytes.len() < 16 || bytes.len() > SAVE_BYTES {
            return None;
        }
        let best = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let unlocked = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        let flags = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
        let mut gravity = flags;
        let mut volume = 64;
        let mut muted = false;
        let music_settings = bytes[0..4] == *b"ARK4";
        let settings = bytes[0..4] == *b"ARK3" || music_settings;
        let music_volume = if music_settings { (flags >> 10) & 255 } else { 64 };
        let music_muted = music_settings && flags & (1 << 18) != 0;
        if settings {
            volume = (flags >> 1) & 255;
            let allowed = if music_settings { 524287 } else { 1023 };
            if flags & !allowed != 0 || volume > 128 || music_volume > 128 {
                return None;
            }
            gravity = flags & 1;
            muted = flags & 512 != 0;
        }
        if bytes[0..4] == *b"ARK1" && bytes.len() == 16 {
            self.patterns.clear();
            self.sfx_volume = 64;
            self.sfx_muted = false;
            self.music_volume = 64;
            self.music_muted = false;
            return Some([best, unlocked, gravity]);
        }
        if (bytes[0..4] != *b"ARK2" && !settings) || bytes.len() < 20 || gravity > 1 {
            return None;
        }
        let count = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        if count > (bytes.len() - 20) / 204 {
            return None;
        }
        let mut patterns = Vec::new();
        let mut offset = 20usize;
        for _ in 0..count {
            if offset + 4 > bytes.len() {
                return None;
            }
            let length = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]) as usize;
            offset += 4;
            if length > bytes.len() - offset || bytes.len() - offset - length < 200 {
                return None;
            }
            let Ok(name) = str::from_utf8(&bytes[offset..offset + length]) else {
                return None;
            };
            offset += length;
            let mut cells = [255; 200];
            cells.copy_from_slice(&bytes[offset..offset + 200]);
            offset += 200;
            for cell in cells {
                if cell > 8 && cell != 255 {
                    return None;
                }
            }
            if !Self::pattern_insert(
                &mut patterns,
                Pattern {
                    name: name.to_string(),
                    cells,
                },
            ) {
                return None;
            }
        }
        if offset != bytes.len() {
            return None;
        }
        self.patterns = patterns;
        self.sfx_volume = volume;
        self.sfx_muted = muted;
        self.music_volume = music_volume;
        self.music_muted = music_muted;
        Some([best, unlocked, gravity])
    }
}
