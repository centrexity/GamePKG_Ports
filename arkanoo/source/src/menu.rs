// Adapted from the pinned original menu.rs; see CREDITS.md.
// Original 1280x720 rectangles use the same half scale and y=60 projection.
pub struct Menu {
    pub settings: bool,
    pub fullscreen: bool,
    pub sfx_volume: u32,
    pub sfx_muted: bool,
    pub music_available: bool,
    pub music_volume: u32,
    pub music_muted: bool,
    pub dragging_music: bool,
    pub save_failed: bool,
    pub dragging_volume: bool,
    left_down: bool,
    pub hover: u32,
    pub window_size: Option<[u32; 2]>,
    pub resolution_pending: bool,
    pub resolution_ticks: u32,
    pub resolution_request: u32,
    pub resolution_failed: bool,
    pub vsync: Option<bool>,
    pub vsync_requested: bool,
    pub vsync_failed: bool,
}
impl Menu {
    pub fn new() -> Self {
        Self {
            settings: false,
            fullscreen: false,
            sfx_volume: 64,
            sfx_muted: false,
            music_available: false,
            music_volume: 64,
            music_muted: false,
            dragging_music: false,
            save_failed: false,
            dragging_volume: false,
            left_down: false,
            hover: 0,
            window_size: None,
            resolution_pending: false,
            resolution_ticks: 0,
            resolution_request: 0,
            resolution_failed: false,
            vsync: None,
            vsync_requested: false,
            vsync_failed: false,
        }
    }
    pub fn resolution(action: u32) -> Option<[u32; 2]> {
        match action {
            13 => Some([1280, 720]),
            14 => Some([1366, 768]),
            15 => Some([1600, 900]),
            16 => Some([1920, 1080]),
            17 => Some([2560, 1440]),
            _ => None,
        }
    }
    // Observe committed host state before input. A newly accepted request is
    // displayed immediately; the following tick reconciles it with the host.
    pub fn observe_resolution(
        &mut self,
        size: Option<[u32; 2]>,
        fullscreen: Option<bool>,
        pending: bool,
    ) {
        if let Some(fullscreen) = fullscreen {
            self.fullscreen = fullscreen;
            if !fullscreen {
                self.window_size = size;
            }
        } else {
            self.window_size = None;
        }
        if pending {
            if self.resolution_pending {
                self.resolution_ticks = self.resolution_ticks.saturating_sub(1);
            } else {
                self.resolution_ticks = 300;
            }
        } else {
            self.resolution_ticks = 0;
        }
        self.resolution_pending = pending;
    }
    pub fn resolution_result(&mut self, action: u32, accepted: bool) {
        self.resolution_failed = !accepted;
        if accepted {
            self.resolution_pending = (13..=17).contains(&action);
            self.resolution_ticks = if self.resolution_pending { 300 } else { 0 };
            self.hover = 0;
            self.dragging_volume = false;
            self.dragging_music = false;
        }
    }
    pub fn bounds(action: u32) -> [f32; 4] {
        if (1..=6).contains(&action) {
            [270.0, 177.5 + (action - 1) as f32 * 25.0, 100.0, 20.0]
        } else if action == 7 {
            [270.0, 260.0, 100.0, 20.0]
        } else if action == 8 {
            [270.0, 440.0, 100.0, 20.0]
        } else if action == 9 {
            [585.0, 395.0, 50.0, 20.0]
        } else if action == 12 {
            [270.0, 170.0, 100.0, 20.0]
        } else if action == 10 {
            [270.0, 215.0, 100.0, 20.0]
        } else if (13..=17).contains(&action) {
            [270.0, 315.0 + (action - 13) as f32 * 17.5, 100.0, 15.0]
        } else if action == 18 {
            [240.0, 415.0, 50.0, 17.5]
        } else if action == 19 {
            [300.0, 415.0, 50.0, 17.5]
        } else if action == 20 {
            [270.0, 285.0, 100.0, 20.0]
        } else {
            [0.0; 4]
        }
    }
    pub fn hit(&self, x: f32, y: f32) -> u32 {
        if !x.is_finite() || !y.is_finite() {
            return 0;
        }
        if self.settings
            && (self.resolution_pending || (self.window_size.is_some() && !self.fullscreen))
        {
            let first = if self.resolution_pending { 18 } else { 13 };
            let last = if self.resolution_pending { 19 } else { 17 };
            let original_x = (x * 2.0) as i32;
            let original_y = ((y - 60.0) * 2.0) as i32;
            for action in first..=last {
                let [left, top, width, height] = Self::bounds(action);
                let left = (left * 2.0) as i32;
                let top = ((top - 60.0) * 2.0) as i32;
                if original_x >= left
                    && original_x < left + (width * 2.0) as i32
                    && original_y >= top
                    && original_y < top + (height * 2.0) as i32
                {
                    return action;
                }
            }
            if self.resolution_pending {
                return 0;
            }
        }
        if self.settings && self.vsync.is_some() {
            let original_x = (x * 2.0) as i32;
            let original_y = ((y - 60.0) * 2.0) as i32;
            if (540..740).contains(&original_x) && (450..490).contains(&original_y) {
                return 20;
            }
        }
        if self.settings
            && self.music_available
            && (270.0..370.0).contains(&x)
            && (170.0..190.0).contains(&y)
        {
            return 12;
        }
        if self.settings && (270.0..370.0).contains(&x) && (215.0..235.0).contains(&y) {
            return 10;
        }
        if !self.settings {
            let original_x = (x * 2.0) as i32;
            let original_y = ((y - 60.0) * 2.0) as i32;
            if (1170..1270).contains(&original_x) && (670..710).contains(&original_y) {
                return 9;
            }
        }
        let first = if self.settings { 7 } else { 1 };
        let last = if self.settings { 8 } else { 6 };
        // SDL Rect truncates projected coordinates before testing inclusive left/top.
        let original_x = (x * 2.0) as i32;
        let original_y = ((y - 60.0) * 2.0) as i32;
        for action in first..=last {
            let [left, top, width, height] = Self::bounds(action);
            let rectangle_x = (left * 2.0) as i32;
            let rectangle_y = ((top - 60.0) * 2.0) as i32;
            if original_x >= rectangle_x
                && original_x < rectangle_x + (width * 2.0) as i32
                && original_y >= rectangle_y
                && original_y < rectangle_y + (height * 2.0) as i32
            {
                return action;
            }
        }
        0
    }
    pub fn menu_event(&mut self, kind: u32, x: f32, y: f32, code: u32) -> u32 {
        if kind == 0
            || kind == 8
            || ((kind == 1 || kind == 2 || kind == 3) && (!x.is_finite() || !y.is_finite()))
        {
            self.hover = 0;
            self.left_down = false;
            self.dragging_volume = false;
            self.dragging_music = false;
            return 0;
        }
        if !self.settings {
            self.dragging_volume = false;
            self.dragging_music = false;
        }
        if kind == 2 && code == 1 {
            self.left_down = true;
            self.dragging_volume = false;
            self.dragging_music = false;
        }
        if kind == 3 && code == 1 {
            self.left_down = false;
            self.dragging_volume = false;
            self.dragging_music = false;
        }
        if kind == 1 || kind == 2 || kind == 3 {
            self.hover = self.hit(x, y);
        }
        if kind == 1 && self.settings {
            if code & 1 == 0 {
                self.left_down = false;
                self.dragging_volume = false;
                self.dragging_music = false;
            }
            if self.left_down {
                let original_x = (x.clamp(0.0, 640.0) * 2.0) as i32;
                let original_y = ((y - 60.0) * 2.0) as i32;
                if (540..740).contains(&original_x) && (360..380).contains(&original_y) {
                    self.dragging_volume = true;
                }
                let mut changed = false;
                if self.music_available
                    && (540..740).contains(&original_x)
                    && (270..290).contains(&original_y)
                {
                    self.dragging_music = true;
                }
                if self.dragging_music && self.music_available {
                    let next = ((original_x - 540).clamp(0, 200) * 128 / 200) as u32;
                    changed = next != self.music_volume;
                    self.music_volume = next;
                }
                if self.dragging_volume {
                    let next = ((original_x - 540).clamp(0, 200) * 128 / 200) as u32;
                    if next != self.sfx_volume {
                        self.sfx_volume = next;
                        changed = true;
                    }
                }
                if changed {
                    return 11;
                }
            }
        }
        if kind == 2 && code == 1 {
            return self.hover;
        }
        0
    }
}
