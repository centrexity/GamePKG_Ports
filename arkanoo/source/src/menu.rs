// Adapted from the pinned original menu.rs; see CREDITS.md.
// Original 1280x720 rectangles use the same half scale and y=60 projection.
pub struct Menu {
    pub settings: bool,
    pub sfx_volume: u32,
    pub sfx_muted: bool,
    pub save_failed: bool,
    pub dragging_volume: bool,
    left_down: bool,
    pub hover: u32,
}
impl Menu {
    pub fn new() -> Self {
        Self {
            settings: false,
            sfx_volume: 64,
            sfx_muted: false,
            save_failed: false,
            dragging_volume: false,
            left_down: false,
            hover: 0,
        }
    }
    pub fn bounds(action: u32) -> [f32; 4] {
        if (1..=6).contains(&action) {
            [270.0, 177.5 + (action - 1) as f32 * 25.0, 100.0, 20.0]
        } else if action == 7 {
            [270.0, 260.0, 100.0, 20.0]
        } else if action == 8 {
            [270.0, 440.0, 100.0, 20.0]
        } else if action == 10 {
            [270.0, 215.0, 100.0, 20.0]
        } else {
            [0.0; 4]
        }
    }
    pub fn hit(&self, x: f32, y: f32) -> u32 {
        if !x.is_finite() || !y.is_finite() {
            return 0;
        }
        if self.settings && (270.0..370.0).contains(&x) && (215.0..235.0).contains(&y) {
            return 10;
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
            return 0;
        }
        if !self.settings {
            self.dragging_volume = false;
        }
        if kind == 2 && code == 1 {
            self.left_down = true;
            self.dragging_volume = false;
        }
        if kind == 3 && code == 1 {
            self.left_down = false;
            self.dragging_volume = false;
        }
        if kind == 1 || kind == 2 || kind == 3 {
            self.hover = self.hit(x, y);
        }
        if kind == 1 && self.settings {
            if code & 1 == 0 {
                self.left_down = false;
                self.dragging_volume = false;
            }
            if self.left_down {
                let original_x = (x.clamp(0.0, 640.0) * 2.0) as i32;
                let original_y = ((y - 60.0) * 2.0) as i32;
                if (540..740).contains(&original_x) && (360..380).contains(&original_y) {
                    self.dragging_volume = true;
                }
                if self.dragging_volume {
                    let next = ((original_x - 540).clamp(0, 200) * 128 / 200) as u32;
                    if next != self.sfx_volume {
                        self.sfx_volume = next;
                        return 11;
                    }
                }
            }
        }
        if kind == 2 && code == 1 {
            return self.hover;
        }
        0
    }
}
