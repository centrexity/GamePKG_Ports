// Names share their immutable prefix. Tail edits retain only changed suffixes.
pub enum Name {
    Compact(std::sync::Arc<str>),
    Large(std::sync::Arc<String>, usize),
    Edited(std::sync::Arc<NameEdit>),
}
pub struct NameEdit {
    base: std::sync::Arc<String>,
    end: usize,
    suffix: String,
}
impl NameEdit {
    fn name_edit_copy(&self) -> Self {
        Self {
            base: self.base.clone(),
            end: self.end,
            suffix: self.suffix.clone(),
        }
    }
    fn name_edit_pop(&mut self) {
        if !self.suffix.is_empty() {
            self.suffix.pop();
        } else if let Some(character) = self.base[..self.end].chars().next_back() {
            self.end -= character.len_utf8();
        }
    }
    fn name_edit_push(&mut self, character: char) {
        if self.suffix.is_empty() && self.base[self.end..].starts_with(character) {
            self.end += character.len_utf8();
        } else {
            self.suffix.push(character);
        }
    }
}
impl Name {
    pub fn from_string(value: String) -> Self {
        if value.len() <= 64 {
            Self::Compact(std::sync::Arc::from(value))
        } else {
            let end = value.len();
            Self::Large(std::sync::Arc::new(value), end)
        }
    }
    pub fn name_segments(&self) -> [&str; 2] {
        match self {
            Self::Compact(value) => [value.as_ref(), ""],
            Self::Large(value, end) => [&value[..*end], ""],
            Self::Edited(value) => [&value.base[..value.end], value.suffix.as_str()],
        }
    }
    pub fn len(&self) -> usize {
        let pieces = self.name_segments();
        pieces[0].len() + pieces[1].len()
    }
    pub fn equals(&self, other: &Self) -> bool {
        let a = self.name_segments();
        let b = other.name_segments();
        self.len() == other.len()
            && a[0]
                .bytes()
                .chain(a[1].bytes())
                .eq(b[0].bytes().chain(b[1].bytes()))
    }
    pub fn less_than(&self, other: &Self) -> bool {
        let a = self.name_segments();
        let b = other.name_segments();
        a[0].bytes()
            .chain(a[1].bytes())
            .lt(b[0].bytes().chain(b[1].bytes()))
    }
    pub fn clone(&self) -> Self {
        match self {
            Self::Compact(value) => Self::Compact(value.clone()),
            Self::Large(value, end) => Self::Large(value.clone(), *end),
            Self::Edited(value) => Self::Edited(value.clone()),
        }
    }
    pub fn name_text(&self) -> String {
        let mut result = String::with_capacity(self.len());
        for piece in self.name_segments() {
            result.push_str(piece);
        }
        result
    }
    pub fn into_string(self) -> String {
        match self {
            Self::Compact(value) => value.to_string(),
            Self::Large(value, end) => match std::sync::Arc::try_unwrap(value) {
                Ok(mut owned) => {
                    owned.truncate(end);
                    owned
                }
                Err(shared) => shared[..end].to_string(),
            },
            Self::Edited(value) => {
                let mut result = String::with_capacity(value.end + value.suffix.len());
                result.push_str(&value.base[..value.end]);
                result.push_str(&value.suffix);
                result
            }
        }
    }
    pub fn shares_storage(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Compact(a), Self::Compact(b)) => std::sync::Arc::ptr_eq(a, b),
            (Self::Large(a, _), Self::Large(b, _)) => std::sync::Arc::ptr_eq(a, b),
            (Self::Edited(a), Self::Edited(b)) => std::sync::Arc::ptr_eq(a, b),
            _ => false,
        }
    }
    pub fn pop(&mut self) {
        match self {
            Self::Compact(value) => {
                let mut owned = value.to_string();
                owned.pop();
                *self = Self::from_string(owned);
            }
            Self::Large(value, end) => {
                if let Some(character) = value[..*end].chars().next_back() {
                    *end -= character.len_utf8();
                }
            }
            Self::Edited(value) => {
                if let Some(owned) = std::sync::Arc::get_mut(value) {
                    owned.name_edit_pop();
                } else {
                    let mut owned = value.name_edit_copy();
                    owned.name_edit_pop();
                    *value = std::sync::Arc::new(owned);
                }
            }
        }
    }
    pub fn push(&mut self, character: char) {
        match self {
            Self::Compact(value) => {
                let mut owned = value.to_string();
                owned.push(character);
                *self = Self::from_string(owned);
            }
            Self::Large(value, end) => {
                if value[*end..].starts_with(character) {
                    *end += character.len_utf8();
                } else if let Some(owned) = std::sync::Arc::get_mut(value) {
                    owned.truncate(*end);
                    owned.push(character);
                    *end = owned.len();
                } else {
                    let mut suffix = String::new();
                    suffix.push(character);
                    *self = Self::Edited(std::sync::Arc::new(NameEdit {
                        base: value.clone(),
                        end: *end,
                        suffix,
                    }));
                }
            }
            Self::Edited(value) => {
                if let Some(owned) = std::sync::Arc::get_mut(value) {
                    owned.name_edit_push(character);
                } else {
                    let mut owned = value.name_edit_copy();
                    owned.name_edit_push(character);
                    *value = std::sync::Arc::new(owned);
                }
            }
        }
    }
}
// Adapted from the pinned upstream editor.rs. Storage is a scoped GamePKG blob.
// Pattern cells retain the original ASCII values: 0..8 or 255 for empty.
pub const SAVE_BYTES: usize = 1048576;
pub struct Pattern {
    pub name: Name,
    cells: [u8; 100],
}
impl Pattern {
    pub fn pattern_from_cells(name: Name, cells: [u8; 200]) -> Option<Self> {
        let mut pattern = Self {
            name,
            cells: [0; 100],
        };
        if pattern.pattern_pack(cells) {
            Some(pattern)
        } else {
            None
        }
    }
    pub fn pattern_pack(&mut self, cells: [u8; 200]) -> bool {
        let mut packed = [0; 100];
        for index in 0..200 {
            let cell = cells[index];
            if cell > 8 && cell != 255 {
                return false;
            }
            let nibble = if cell == 255 { 15 } else { cell };
            packed[index / 2] |= nibble << ((index % 2) * 4);
        }
        self.cells = packed;
        true
    }
    pub fn pattern_get_cell(&self, index: usize) -> u8 {
        let nibble = (self.cells[index / 2] >> ((index % 2) * 4)) & 15;
        if nibble == 15 {
            255
        } else {
            nibble
        }
    }
    pub fn pattern_unpack(&self) -> [u8; 200] {
        let mut cells = [255; 200];
        for (index, cell) in cells.iter_mut().enumerate() {
            *cell = self.pattern_get_cell(index);
        }
        cells
    }

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
        Self::pattern_from_cells(Name::from_string(name.to_string()), cells).unwrap()
    }
    pub fn ascii_write(&self) -> String {
        let mut content = String::from("# Pattern: ");
        for piece in self.name.name_segments() {
            content.push_str(piece);
        }
        content
            .push_str("\n# * = empty, 0-5 = normal, 6 = Ice, 7 = Explosive, 8 = Undestroyable\n\n");
        for row in 0..10 {
            for col in 0..20 {
                let cell = self.pattern_get_cell(row * 20 + col);
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
    pub name: Name,
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
    backup: Option<Vec<(usize, Option<[u8; 200]>)>>,
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
            name: Name::from_string(String::from("my_pattern")),
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
        for cell in self.cells {
            if cell > 8 && cell != 255 {
                self.editor_message("INVALID BLOCK TYPE");
                return false;
            }
        }
        // Names are identifiers inside the blob, never filesystem paths.
        // Apply only the actual host save-size bound; do not truncate names.
        let mut size = 20usize;
        for pattern in &self.patterns {
            if !pattern.name.equals(&self.name) {
                size = size.saturating_add(204).saturating_add(pattern.name.len());
            }
        }
        size = size.saturating_add(204).saturating_add(self.name.len());
        if size > SAVE_BYTES {
            self.editor_message("SAVE STORAGE FULL");
            return false;
        }
        if self.backup.is_none() {
            self.backup = Some(Vec::new());
        }
        for (index, pattern) in self.patterns.iter_mut().enumerate() {
            if pattern.name.equals(&self.name) {
                if let Some(undo) = self.backup.as_mut() {
                    undo.push((index, Some(pattern.pattern_unpack())));
                }
                pattern.pattern_pack(self.cells);
                self.save_requested = true;
                return true;
            }
        }
        let mut index = 0;
        while index < self.patterns.len() && self.patterns[index].name.less_than(&self.name) {
            index += 1;
        }
        self.patterns.insert(
            index,
            Pattern::pattern_from_cells(self.name.clone(), self.cells).unwrap(),
        );
        if let Some(undo) = self.backup.as_mut() {
            undo.push((index, None));
        }
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
            if let Some(mut undo) = self.backup.take() {
                // Reverse order restores indices shifted by later inserts.
                while let Some((index, previous)) = undo.pop() {
                    if let Some(cells) = previous {
                        self.patterns[index].pattern_pack(cells);
                    } else {
                        self.patterns.remove(index);
                    }
                }
            }
            self.editor_message("SAVE FAILED / NOT STORED");
        }
        self.save_requested = false;
    }
    pub fn pattern_insert(patterns: &mut Vec<Pattern>, entry: Pattern) -> bool {
        let mut low = 0;
        let mut high = patterns.len();
        while low < high {
            let middle = low + (high - low) / 2;
            if patterns[middle].name.equals(&entry.name) {
                return false;
            }
            if patterns[middle].name.less_than(&entry.name) {
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
            self.cells = pattern.pattern_unpack();
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
        // Reserve the whole archive once. Appending cells after a large name
        // must not double the allocation under the guest's memory ceiling.
        let mut length = 20;
        for pattern in &self.patterns {
            length += 204 + pattern.name.len();
        }
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(b"ARK2");
        bytes.extend_from_slice(&u32::to_le_bytes(best));
        bytes.extend_from_slice(&u32::to_le_bytes(unlocked));
        bytes.extend_from_slice(&u32::to_le_bytes(u32::from(gravity)));
        bytes.extend_from_slice(&u32::to_le_bytes(self.patterns.len() as u32));
        for pattern in &self.patterns {
            bytes.extend_from_slice(&u32::to_le_bytes(pattern.name.len() as u32));
            for piece in pattern.name.name_segments() {
                bytes.extend_from_slice(piece.as_bytes());
            }
            bytes.extend_from_slice(&pattern.pattern_unpack());
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
    pub fn save_take_encode_settings(
        &mut self,
        best: u32,
        unlocked: u32,
        gravity: bool,
        volume: u32,
        muted: bool,
    ) -> Vec<u8> {
        if self.patterns.len() != 1 {
            return self.save_encode_settings(best, unlocked, gravity, volume, muted);
        }
        // Lend the sole identifier's buffer to the synchronous save call.
        // save_restore_encoded must run before committing or rolling back.
        let identifier =
            core::mem::replace(&mut self.patterns[0].name, Name::from_string(String::new()));
        let mut bytes = identifier.into_string().into_bytes();
        let length = bytes.len();
        bytes.reserve_exact(224);
        bytes.resize(length + 224, 0);
        bytes.copy_within(0..length, 24);
        bytes[..4].copy_from_slice(b"ARK3");
        bytes[4..8].copy_from_slice(&u32::to_le_bytes(best));
        bytes[8..12].copy_from_slice(&u32::to_le_bytes(unlocked));
        let flags = u32::from(gravity) | (volume.min(128) << 1) | (u32::from(muted) << 9);
        bytes[12..16].copy_from_slice(&u32::to_le_bytes(flags));
        bytes[16..20].copy_from_slice(&u32::to_le_bytes(1));
        bytes[20..24].copy_from_slice(&u32::to_le_bytes(length as u32));
        bytes[24 + length..].copy_from_slice(&self.patterns[0].pattern_unpack());
        bytes
    }
    pub fn save_take_encode_audio_settings(
        &mut self,
        best: u32,
        unlocked: u32,
        gravity: bool,
        volumes: [u32; 2],
        muted: [bool; 2],
    ) -> Vec<u8> {
        let mut bytes =
            self.save_take_encode_settings(best, unlocked, gravity, volumes[0], muted[0]);
        bytes[..4].copy_from_slice(b"ARK4");
        let flags = u32::from(gravity)
            | (volumes[0].min(128) << 1)
            | (u32::from(muted[0]) << 9)
            | (volumes[1].min(128) << 10)
            | (u32::from(muted[1]) << 18);
        bytes[12..16].copy_from_slice(&u32::to_le_bytes(flags));
        bytes
    }
    pub fn save_restore_encoded(&mut self, mut bytes: Vec<u8>) {
        if self.patterns.len() == 1 {
            let length = bytes.len() - 224;
            bytes.copy_within(24..24 + length, 0);
            bytes.truncate(length);
            if let Ok(name) = String::from_utf8(bytes) {
                self.patterns[0].name = Name::from_string(name);
            }
        }
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
    pub fn save_decode_owned(&mut self, mut bytes: Vec<u8>) -> Option<[u32; 3]> {
        // A single long identifier can occupy almost the whole archive. Keep
        // its original allocation rather than copying it beside the input.
        if bytes.len() < 224 || bytes.len() > SAVE_BYTES || bytes[16..20] != u32::to_le_bytes(1) {
            return self.save_decode(&bytes);
        }
        let length = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]) as usize;
        if length != bytes.len() - 224 {
            return None;
        }
        let mut header = Vec::with_capacity(20);
        header.extend_from_slice(&bytes[..16]);
        header.extend_from_slice(&u32::to_le_bytes(0));
        let mut decoded = Self::new();
        let metadata = decoded.save_decode(&header)?;
        if str::from_utf8(&bytes[24..24 + length]).is_err() {
            return None;
        }
        let mut cells = [255; 200];
        cells.copy_from_slice(&bytes[24 + length..]);
        for cell in cells {
            if cell > 8 && cell != 255 {
                return None;
            }
        }
        bytes.truncate(24 + length);
        bytes.drain(..24);
        let name = String::from_utf8(bytes).ok()?;
        let patterns = Vec::from([Pattern::pattern_from_cells(Name::from_string(name), cells)?]);
        self.patterns = patterns;
        self.sfx_volume = decoded.sfx_volume;
        self.sfx_muted = decoded.sfx_muted;
        self.music_volume = decoded.music_volume;
        self.music_muted = decoded.music_muted;
        Some(metadata)
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
        let music_volume = if music_settings {
            (flags >> 10) & 255
        } else {
            64
        };
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
        // Every archive version stores a boolean gravity bit. Validate before
        // the legacy early return so corrupt saves cannot clear existing data.
        if gravity > 1 {
            return None;
        }
        if bytes[0..4] == *b"ARK1" && bytes.len() == 16 {
            self.patterns.clear();
            self.sfx_volume = 64;
            self.sfx_muted = false;
            self.music_volume = 64;
            self.music_muted = false;
            return Some([best, unlocked, gravity]);
        }
        if (bytes[0..4] != *b"ARK2" && !settings) || bytes.len() < 20 {
            return None;
        }
        let count = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
        if count > (bytes.len() - 20) / 204 {
            return None;
        }
        // Count is bounded above by the validated archive length. Avoid
        // geometric growth retaining thousands of unused pattern slots.
        let mut patterns = Vec::with_capacity(count);
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
                Pattern::pattern_from_cells(Name::from_string(name.to_string()), cells)?,
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
