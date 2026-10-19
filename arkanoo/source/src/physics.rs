use crate::gamepkg;
use crate::levels;
// Original 1280x720 world is uniformly scaled by 1/2 and letterboxed at y=60.
// Ball positions are centers; other positions are rectangle top-lefts.
pub const TOP: f32 = 60.0;
pub const BOTTOM: f32 = 420.0;
pub const PADDLE_Y: f32 = 395.0;
pub const BALL_RADIUS: f32 = 3.0;
pub const BLOCK_WIDTH: f32 = 30.0;
pub const BLOCK_HEIGHT: f32 = 10.0;

// SDL constructs integer rectangles in original coordinates before intersecting.
pub fn overlap(first: [f32; 4], second: [f32; 4]) -> (f32, f32) {
    let [x, y, width, height] = first;
    let [other_x, other_y, other_width, other_height] = second;
    let left = (x * 2.0) as i32;
    let top = ((y - TOP) * 2.0) as i32;
    let right = left + (width * 2.0) as i32;
    let bottom = top + (height * 2.0) as i32;
    let other_left = (other_x * 2.0) as i32;
    let other_top = ((other_y - TOP) * 2.0) as i32;
    let w = right.min(other_left + (other_width * 2.0) as i32) - left.max(other_left);
    let h = bottom.min(other_top + (other_height * 2.0) as i32) - top.max(other_top);
    if w > 0 && h > 0 {
        (w as f32 * 0.5, h as f32 * 0.5)
    } else {
        (0.0, 0.0)
    }
}
pub fn below(bound: u64) -> u64 {
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let value = gamepkg::random_u64();
        if value >= threshold {
            return value % bound;
        }
    }
}
pub struct Ball {
    pub trail: Vec<[f32; 2]>,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub spin: f32,
    pub attached: bool,
    pub launch_timer: u32,
}
impl Ball {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            trail: Vec::new(),
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            spin: 0.0,
            attached: true,
            launch_timer: 30,
        }
    }
    pub fn launch(&mut self) {
        if self.attached {
            let direction = below(3);
            self.vx = if direction == 0 {
                -2.0
            } else if direction == 1 {
                0.0
            } else {
                2.0
            };
            self.vy = -2.0;
            self.attached = false;
        }
    }

    pub fn advance(&mut self, gravity: bool) {
        self.record_trail();
        self.apply_forces(gravity);
        self.x += self.vx;
        self.y += self.vy;
        if self.x <= BALL_RADIUS {
            self.x = BALL_RADIUS;
            self.vx = self.vx.abs();
        } else if self.x >= 640.0 - BALL_RADIUS {
            self.x = 640.0 - BALL_RADIUS;
            self.vx = -self.vx.abs();
        }
        if self.y <= TOP + BALL_RADIUS {
            self.y = TOP + BALL_RADIUS;
            self.vy = self.vy.abs();
        }
        if self.vx.abs() < 1.0 {
            self.vx = if self.vx >= 0.0 { 1.0 } else { -1.0 };
        }
    }
    pub fn record_trail(&mut self) {
        let speed = (self.vx * self.vx + self.vy * self.vy).sqrt() * 120.0;
        if speed >= 800.0 {
            self.trail.push([
                (self.x - BALL_RADIUS) * 2.0,
                (self.y - TOP - BALL_RADIUS) * 2.0,
            ]);
            let maximum = if speed >= 1400.0 { 20 } else { 8 };
            // The original pops one entry, rather than truncating on a tier drop.
            if self.trail.len() > maximum {
                self.trail.remove(0);
            }
        } else {
            self.trail.clear();
        }
    }
    pub fn active(&self) -> bool {
        self.y < BOTTOM + BALL_RADIUS
    }
    pub fn intersects(&self, x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
        overlap(
            [
                self.x - BALL_RADIUS,
                self.y - BALL_RADIUS,
                BALL_RADIUS * 2.0,
                BALL_RADIUS * 2.0,
            ],
            [x, y, width, height],
        )
    }
    pub fn resolve_block(&mut self, x: f32, y: f32, ghost: bool) -> bool {
        let (width, height) = self.intersects(x, y, BLOCK_WIDTH, BLOCK_HEIGHT);
        if width == 0.0 {
            return false;
        }
        if !ghost {
            if width < height {
                if self.x < x + BLOCK_WIDTH / 2.0 {
                    self.x -= width;
                } else {
                    self.x += width;
                }
                self.vx = -self.vx;
            } else {
                if self.y < y + BLOCK_HEIGHT / 2.0 {
                    self.y -= height;
                } else {
                    self.y += height;
                }
                self.vy = -self.vy;
            }
        }
        true
    }
    // Velocities/spin use half of the original 1280-wide world's units.
    pub fn apply_forces(&mut self, gravity: bool) {
        if gravity {
            if self.vy > 0.0 && self.vy < 4.5 {
                self.vy = (self.vy + 0.15).min(4.5);
            }
            self.spin = 0.0;
        } else {
            self.vx += self.spin * 0.05;
            self.spin *= 0.98;
        }
    }
}
pub struct Drop {
    pub x: f32,
    pub y: f32,
    pub kind: u32,
}
pub struct Particle {
    // Keep original 1280x720 coordinates so float stepping and integer shard
    // projection do not drift through repeated scale/letterbox conversions.
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: u32,
    pub max_life: u32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub size: i32,
    pub color: u32,
}
pub fn random_pair() -> [f32; 2] {
    let word = gamepkg::random_u64();
    [
        (word >> 40) as f32 / 16777216.0,
        ((word >> 16) & 16777215) as f32 / 16777216.0,
    ]
}
impl Particle {
    pub fn new(x: f32, y: f32, vx: f32, vy: f32, color: u32) -> Self {
        // Disjoint bits keep rotation, spin, age and size independent. Rejection
        // preserves uniform integer ranges while avoiding per-field host RPCs.
        let word = gamepkg::random_u64();
        let rotation = (word >> 40) as f32 / 16777216.0 * 360.0;
        let rotation_speed = ((word >> 16) & 16777215) as f32 / 16777216.0 * 20.0 - 10.0;
        let mut age_sample = word as u8;
        while age_sample >= 240 {
            age_sample = gamepkg::random_u64() as u8;
        }
        let mut size_sample = (word >> 8) as u8;
        while size_sample == 255 {
            size_sample = gamepkg::random_u64() as u8;
        }
        let life = u32::from(age_sample) % 20 + 20;
        Self {
            x: x * 2.0,
            y: (y - TOP) * 2.0,
            vx: vx * 2.0,
            vy: vy * 2.0,
            life,
            max_life: life,
            rotation,
            rotation_speed,
            size: i32::from(size_sample) % 5 + 3,
            color,
        }
    }
    pub fn advance(&mut self) {
        self.x += self.vx;
        self.y += self.vy;
        self.vy += 0.3;
        self.rotation += self.rotation_speed;
        self.life = self.life.saturating_sub(1);
    }
    pub fn alpha(&self) -> u8 {
        ((1.0 - (self.max_life - self.life) as f32 / self.max_life as f32) * 255.0) as u8
    }
    pub fn shard_coordinates(&self) -> Vec<[i32; 2]> {
        let mut shard_points = Vec::new();
        let half = self.size / 2;
        let angle = self.rotation.to_radians();
        let cosine = angle.cos();
        let sine = angle.sin();
        for dx in -half..=half {
            for dy in -half..=half {
                if dx.abs() + dy.abs() <= half {
                    let rx = (dx as f32 * cosine - dy as f32 * sine) as i32;
                    let ry = (dx as f32 * sine + dy as f32 * cosine) as i32;
                    shard_points.push([self.x as i32 + rx, self.y as i32 + ry]);
                }
            }
        }
        shard_points
    }
}
pub struct ParticleImage {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
pub fn particle_image(particles: &[Particle]) -> Option<ParticleImage> {
    let mut left = 1280i32;
    let mut top = 720i32;
    let mut right = -1i32;
    let mut bottom = -1i32;
    for p in particles {
        let x = p.x as i32;
        let y = p.y as i32;
        left = left.min((x - 5).max(0));
        top = top.min((y - 5).max(0));
        right = right.max((x + 5).min(1279));
        bottom = bottom.max((y + 5).min(719));
    }
    if left > right || top > bottom {
        return None;
    }
    let width = (right - left + 1) as u32;
    let height = (bottom - top + 1) as u32;
    let length = width as usize * height as usize * 4;
    let mut pixels = Vec::with_capacity(length);
    pixels.resize(length, 0u8);
    for p in particles {
        let opacity = u32::from(p.alpha());
        if opacity == 0 {
            continue;
        }
        for [x, y] in p.shard_coordinates() {
            if x < left || x > right || y < top || y > bottom {
                continue;
            }
            let offset = ((y - top) as usize * width as usize + (x - left) as usize) * 4;
            blend_effect_pixel(&mut pixels, offset, p.color, opacity);
        }
    }
    Some(ParticleImage {
        left,
        top,
        width,
        height,
        pixels,
    })
}
// Compose only the occupied 128px tiles, in the original row-major order.
// The tile grid remains anchored to the original full-image bounds.
pub fn particle_tiles(particles: &[Particle]) -> Vec<ParticleImage> {
    let mut left = 1280i32;
    let mut top = 720i32;
    let mut right = -1i32;
    let mut bottom = -1i32;
    for p in particles {
        let x = p.x as i32;
        let y = p.y as i32;
        left = left.min((x - 5).max(0));
        top = top.min((y - 5).max(0));
        right = right.max((x + 5).min(1279));
        bottom = bottom.max((y + 5).min(719));
    }
    let mut rendered = Vec::new();
    if left > right || top > bottom {
        return rendered;
    }
    let columns = (right - left + 128) / 128;
    let rows = (bottom - top + 128) / 128;
    let mut tiles: Vec<Option<ParticleImage>> = Vec::new();
    for _ in 0..columns * rows {
        tiles.push(None);
    }
    for p in particles {
        let opacity = u32::from(p.alpha());
        if opacity == 0 {
            continue;
        }
        for [x, y] in p.shard_coordinates() {
            if x < left || x > right || y < top || y > bottom {
                continue;
            }
            let tile_x = (x - left) / 128;
            let tile_y = (y - top) / 128;
            let slot = &mut tiles[(tile_y * columns + tile_x) as usize];
            if slot.is_none() {
                let tile_left = left + tile_x * 128;
                let tile_top = top + tile_y * 128;
                let width = (right - tile_left + 1).min(128) as u32;
                let height = (bottom - tile_top + 1).min(128) as u32;
                let mut pixels = Vec::new();
                pixels.resize(width as usize * height as usize * 4, 0u8);
                *slot = Some(ParticleImage {
                    left: tile_left,
                    top: tile_top,
                    width,
                    height,
                    pixels,
                });
            }
            if let Some(image) = slot {
                let offset = ((y - image.top) as usize * image.width as usize
                    + (x - image.left) as usize)
                    * 4;
                blend_effect_pixel(&mut image.pixels, offset, p.color, opacity);
            }
        }
    }
    for image in tiles.into_iter().flatten() {
        rendered.push(image);
    }
    rendered
}
pub struct EffectPoint {
    pub x: i32,
    pub y: i32,
    pub color: u32,
}
pub fn blend_effect_pixel(pixels: &mut [u8], offset: usize, rgba: u32, opacity: u32) {
    if opacity == 0 {
        return;
    }
    let old_alpha = u32::from(pixels[offset + 3]);
    // The original expression cancels for a transparent destination or opaque source.
    // Avoid three divisions without changing blend order, colors or alpha.
    if old_alpha == 0 || opacity == 255 {
        pixels[offset] = (rgba >> 24) as u8;
        pixels[offset + 1] = (rgba >> 16) as u8;
        pixels[offset + 2] = (rgba >> 8) as u8;
        pixels[offset + 3] = opacity as u8;
        return;
    }
    let denominator = opacity * 255 + old_alpha * (255 - opacity);
    for channel in 0..3 {
        let color = (rgba >> (24 - channel * 8)) & 255;
        pixels[offset + channel] = ((color * opacity * 255
            + u32::from(pixels[offset + channel]) * old_alpha * (255 - opacity))
            / denominator) as u8;
    }
    pixels[offset + 3] = (denominator / 255) as u8;
}
pub fn effect_image(dots: &[EffectPoint]) -> Option<ParticleImage> {
    let mut left = 1280i32;
    let mut top = 720i32;
    let mut right = -1i32;
    let mut bottom = -1i32;
    for dot in dots {
        if (0..1280).contains(&dot.x) && (0..720).contains(&dot.y) && dot.color & 255 != 0 {
            left = left.min(dot.x);
            right = right.max(dot.x);
            top = top.min(dot.y);
            bottom = bottom.max(dot.y);
        }
    }
    if left > right || top > bottom {
        return None;
    }
    let width = (right - left + 1) as u32;
    let height = (bottom - top + 1) as u32;
    let length = width as usize * height as usize * 4;
    let mut pixels = Vec::with_capacity(length);
    pixels.resize(length, 0u8);
    for dot in dots {
        if dot.x >= left && dot.x <= right && dot.y >= top && dot.y <= bottom {
            let offset = ((dot.y - top) as usize * width as usize + (dot.x - left) as usize) * 4;
            blend_effect_pixel(&mut pixels, offset, dot.color, dot.color & 255);
        }
    }
    Some(ParticleImage {
        left,
        top,
        width,
        height,
        pixels,
    })
}
pub fn effect_circle(dots: &mut Vec<EffectPoint>, x: i32, y: i32, radius: i32, color: u32) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx * dx + dy * dy <= radius * radius {
                dots.push(EffectPoint {
                    x: x + dx,
                    y: y + dy,
                    color,
                });
            }
        }
    }
}
pub fn portal_points(frame: u64, timer: u64) -> Vec<EffectPoint> {
    let mut dots = Vec::new();
    if timer <= 150 {
        portal_rings(&mut dots, frame, timer);
    } else if timer <= 180 {
        let progress = (timer - 151) as f32 / 30.0;
        let radius = (50.0 + progress * 200.0) as i32;
        let point_opacity = ((1.0 - progress) * 255.0) as u8;
        portal_glow(&mut dots, radius, point_opacity, 0xffc8ff00);
        effect_circle(
            &mut dots,
            640,
            360,
            30,
            0xffffff00 | u32::from(point_opacity),
        );
    } else if timer < 270 {
        let progress = (timer - 181) as f32 / 89.0;
        let point_opacity = ((1.0 - progress) * 150.0) as u8;
        portal_glow(&mut dots, 80, point_opacity, 0xc896ff00);
    }
    dots
}
fn portal_glow(dots: &mut Vec<EffectPoint>, radius: i32, opacity: u8, color: u32) {
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let square = dx * dx + dy * dy;
            if square <= radius * radius {
                let distance = (square as f32).sqrt();
                let edge = 1.0 - distance / radius as f32;
                let point_opacity = (opacity as f32 * edge) as u8;
                dots.push(EffectPoint {
                    x: 640 + dx,
                    y: 360 + dy,
                    color: color | u32::from(point_opacity),
                });
            }
        }
    }
}
fn portal_rings(dots: &mut Vec<EffectPoint>, frame: u64, timer: u64) {
    let closing = timer >= 31;
    let progress = if closing {
        (timer - 31) as f32 / 120.0
    } else {
        0.0
    };
    for i in 0..10 {
        let base_radius = 150 - i * 10;
        let radius = if closing {
            (base_radius as f32 * (1.0 - progress)).max(0.0) as i32
        } else {
            base_radius
        };
        if radius <= 0 {
            continue;
        }
        let rotation = frame as f32 * if closing { 0.15 } else { 0.1 } + i as f32 * 0.3;
        let base_alpha = ((frame as f32 * 0.05 + i as f32 * 0.5).sin() * 100.0 + 155.0) as u8;
        let point_opacity = if closing {
            (base_alpha as f32 * (1.0 - progress * 0.3)) as u8
        } else {
            base_alpha
        };
        let shift = (i as f32 / 10.0 * 100.0) as u8;
        let red = if closing {
            (150u16 + shift as u16 + (progress * 50.0) as u16).min(255) as u8
        } else {
            150 + shift
        };
        let blue = if closing {
            (255 - shift).saturating_sub((progress * 100.0) as u8)
        } else {
            255 - shift
        };
        let color =
            (u32::from(red) << 24) | (50 << 16) | (u32::from(blue) << 8) | u32::from(point_opacity);
        for j in 0..32 {
            let angle = (j as f32 / 32.0) * core::f32::consts::PI * 2.0 + rotation;
            let x = 640 + (angle.cos() * radius as f32) as i32;
            let y = 360 + (angle.sin() * radius as f32) as i32;
            effect_circle(dots, x, y, 4, color);
        }
    }
}
pub fn ball_effect_points(ball: &Ball, frame: u64) -> Vec<EffectPoint> {
    let mut dots = Vec::new();
    let cx = ((ball.x - BALL_RADIUS) * 2.0) as i32 + 6;
    let cy = ((ball.y - TOP - BALL_RADIUS) * 2.0) as i32 + 6;
    let vx = ball.vx * 2.0;
    let vy = ball.vy * 2.0;
    let speed = (vx.powi(2) + vy.powi(2)).sqrt();
    let speed_per_second = speed * 60.0;
    for (index, position) in ball.trail.iter().enumerate() {
        let opacity = ((index + 1) as f32 / ball.trail.len() as f32 * 180.0) as u8;
        for dy in 0i32..9 {
            for dx in 0i32..9 {
                let cdx = dx - 4;
                let cdy = dy - 4;
                if cdx * cdx + cdy * cdy <= 16 {
                    dots.push(EffectPoint {
                        x: position[0] as i32 + dx,
                        y: position[1] as i32 + dy,
                        color: 0xc8c8ff00 | u32::from(opacity),
                    });
                }
            }
        }
    }
    if speed_per_second >= 800.0 {
        let super_fireball = speed_per_second >= 1400.0;
        let dx = -vx / speed;
        let dy = -vy / speed;
        let length = if super_fireball { 30 } else { 20 };
        let intensity = if super_fireball { 220.0 } else { 180.0 };
        for index in 1..=length {
            let distance = index as f32 * 2.0;
            let x = cx + (dx * distance) as i32;
            let y = cy + (dy * distance) as i32;
            let pulse = if super_fireball {
                (frame as f32 * 0.3).sin() * 0.2 + 0.8
            } else {
                1.0
            };
            let opacity = ((length - index) as f32 / length as f32 * intensity * pulse) as u8;
            let radius = 6 - index / 4;
            let color = if super_fireball {
                if index < 10 {
                    0xffffc800
                } else if index < 20 {
                    0xff000000 | (((220 - (index - 10) * 8) as u32) << 16) | 0x6400
                } else {
                    0xff643200
                }
            } else if index < 7 {
                0xff000000 | (((255 - index * 20) as u32) << 16) | 0x6400
            } else if index < 14 {
                0xff000000 | (((140 - (index - 7) * 15) as u32) << 16) | 0x3200
            } else {
                0xc8321400
            };
            effect_circle(&mut dots, x, y, radius, color | u32::from(opacity));
        }
        if super_fireball {
            let spin = frame as f32 * 0.2;
            for index in 0..12 {
                let angle = (index as f32 / 12.0) * core::f32::consts::PI * 2.0 + spin;
                let x = cx + (angle.cos() * 18.0) as i32;
                let y = cy + (angle.sin() * 18.0) as i32;
                let opacity =
                    ((frame as f32 * 0.3 + index as f32 * 0.5).sin() * 80.0 + 175.0) as u8;
                effect_circle(&mut dots, x, y, 3, 0xffffc800 | u32::from(opacity));
            }
        }
    }
    dots
}
pub struct Penguin {
    pub x: f32,
    pub y: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub state: u32,
    pub frame_count: u32,
}
impl Penguin {
    pub fn new(target_x: f32, target_y: f32) -> Self {
        Self {
            x: 1230.0,
            y: 620.0,
            target_x,
            target_y,
            state: 0,
            frame_count: 0,
        }
    }
    pub fn advance_penguin(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);
        if self.state == 0 {
            let dx = self.target_x - self.x;
            let dy = self.target_y - self.y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 5.0 {
                self.x += dx / distance * 5.5;
                self.y += dy / distance * 5.5;
            } else {
                self.x = self.target_x;
                self.y = self.target_y;
                self.state = 1;
                self.frame_count = 0;
            }
        } else if self.state == 1 {
            if self.frame_count >= 30 {
                self.state = 2;
                self.frame_count = 0;
            }
        } else if self.state == 2 {
            self.x += 7.0;
            self.y += 5.0;
            if self.x > 1380.0 || self.y > 820.0 {
                self.state = 3;
            }
        }
    }
}
pub struct PenguinRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub color: u32,
}
pub fn penguin_shape(
    shapes: &mut Vec<PenguinRect>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    color: u32,
) {
    shapes.push(PenguinRect {
        x,
        y,
        width,
        height,
        color,
    });
}
pub fn penguin_rects(penguin: &Penguin) -> Vec<PenguinRect> {
    let mut shapes = Vec::new();
    let x = penguin.x as i32;
    let y = penguin.y as i32;
    let flying = penguin.state == 0 || penguin.state == 2;
    let hover = if flying {
        (penguin.frame_count as f32 * 0.15).sin() * 3.0
    } else {
        0.0
    };
    let body_y = y + hover as i32;
    if flying {
        let flicker = penguin.frame_count % 4;
        penguin_shape(&mut shapes, x + 8, body_y + 42, 4, 8 + flicker, 0xffc800ff);
        penguin_shape(&mut shapes, x + 13, body_y + 42, 4, 8 + flicker, 0xffc800ff);
        penguin_shape(&mut shapes, x + 9, body_y + 42, 2, 5 + flicker, 0xffff96ff);
        penguin_shape(&mut shapes, x + 14, body_y + 42, 2, 5 + flicker, 0xffff96ff);
    }
    penguin_shape(&mut shapes, x, body_y + 8, 25, 30, 0x1e1e28ff);
    penguin_shape(&mut shapes, x + 6, body_y + 12, 13, 22, 0xf0f0faff);
    penguin_shape(&mut shapes, x + 5, body_y + 20, 5, 15, 0x646478ff);
    penguin_shape(&mut shapes, x + 15, body_y + 20, 5, 15, 0x646478ff);
    penguin_shape(&mut shapes, x + 3, body_y, 19, 18, 0x1e1e28ff);
    penguin_shape(&mut shapes, x + 8, body_y + 5, 3, 3, 0xffffffff);
    penguin_shape(&mut shapes, x + 14, body_y + 5, 3, 3, 0xffffffff);
    penguin_shape(&mut shapes, x + 10, body_y + 10, 5, 3, 0xff8c00ff);
    penguin_shape(&mut shapes, x + 7, body_y + 38, 5, 2, 0xff8c00ff);
    penguin_shape(&mut shapes, x + 13, body_y + 38, 5, 2, 0xff8c00ff);
    if penguin.state == 1 || penguin.state == 2 {
        let hx = x + if penguin.state == 1 { 20 } else { 18 };
        let hy = body_y + 15;
        penguin_shape(&mut shapes, hx, hy + 2, 8, 6, 0xdc143cff);
        penguin_shape(&mut shapes, hx + 1, hy, 3, 3, 0xdc143cff);
        penguin_shape(&mut shapes, hx + 4, hy, 3, 3, 0xdc143cff);
        penguin_shape(&mut shapes, hx + 2, hy + 8, 4, 2, 0xdc143cff);
    }
    shapes
}
pub fn heart_points(cx: i32, cy: i32, size: i32) -> Vec<EffectPoint> {
    let mut dots = Vec::new();
    let half = size / 2;
    for dx in -half..=half {
        for dy in -half..=0 {
            let left = ((dx + half / 2).pow(2) + dy.pow(2)) as f32;
            let right = ((dx - half / 2).pow(2) + dy.pow(2)) as f32;
            let radius = (half as f32 / 1.5).powf(2.0);
            if left <= radius || right <= radius {
                dots.push(EffectPoint {
                    x: cx + dx,
                    y: cy + dy,
                    color: 0xdc143cff,
                });
            }
        }
    }
    for dy in 0..=size {
        let width = size - dy;
        for dx in -(width / 2)..=width / 2 {
            dots.push(EffectPoint {
                x: cx + dx,
                y: cy + dy,
                color: 0xdc143cff,
            });
        }
    }
    dots
}
pub struct Game {
    pub penguin: Option<Penguin>,
    pub stolen_heart: Option<[f32; 2]>,
    pub splash_timer: u32,
    pub started: bool,
    pub menu: crate::menu::Menu,
    pub fullscreen_requests: u32,
    pub quit_requested: bool,
    pub homepage_requested: bool,
    pub editor: crate::editor::Editor,
    pub testing: bool,
    pub block_color: [u8; 200],
    pub mode: u32,
    pub level: u32,
    pub unlocked: u32,
    pub score: u32,
    pub best: u32,
    pub lives: u32,
    pub paddle: f32,
    pub last_paddle: f32,
    pub paddle_glow: f32,
    pub width: f32,
    pub long: u32,
    pub ghost: u32,
    pub ammo: u32,
    pub balls: Vec<Ball>,
    pub drops: Vec<Drop>,
    pub rockets: Vec<(f32, f32)>,
    pub particles: Vec<Particle>,
    pub particle_spawns: Vec<(f32, f32, u32)>,
    pub blocks: [u8; 200],
    pub block_type: [u8; 200],
    pub block_health: [u8; 200],
    pub block_max_health: [u8; 200],
    pub block_x: [f32; 200],
    pub block_y: [f32; 200],
    pub gravity: bool,
    pub portal: bool,
    pub portal_timer: u32,
    pub max_speed: f32,
    pub max_speed_record_frame: u64,
    pub fps: u32,
    pub fps_times: Vec<u64>,
    pub frame: u64,
    pub lost_life: bool,
    pub cooldown: u32,
    pub sound: u32,
    pub effects: crate::audio::Mixer,
    pub dirty: bool,
}
impl Game {
    pub fn new() -> Self {
        let mut game = Self {
            penguin: None,
            stolen_heart: None,
            splash_timer: 0,
            started: false,
            menu: crate::menu::Menu::new(),
            fullscreen_requests: 0,
            quit_requested: false,
            homepage_requested: false,
            editor: crate::editor::Editor::new(),
            testing: false,
            block_color: [0; 200],
            mode: 7,
            level: 1,
            unlocked: 1,
            score: 0,
            best: 0,
            lives: 3,
            paddle: 285.0,
            last_paddle: 285.0,
            paddle_glow: 0.0,
            width: 70.0,
            long: 0,
            ghost: 0,
            ammo: 0,
            balls: Vec::new(),
            drops: Vec::new(),
            rockets: Vec::new(),
            particles: Vec::new(),
            particle_spawns: Vec::new(),
            blocks: [0; 200],
            block_type: [0; 200],
            block_health: [1; 200],
            block_max_health: [1; 200],
            block_x: [0.0; 200],
            block_y: [0.0; 200],
            gravity: false,
            portal: false,
            portal_timer: 0,
            max_speed: 0.0,
            max_speed_record_frame: 0,
            fps: 0,
            fps_times: Vec::new(),
            frame: 0,
            lost_life: false,
            cooldown: 0,
            sound: 0,
            effects: crate::audio::Mixer::new(),
            dirty: false,
        };
        game.populate();
        game.attach();
        game
    }
    pub fn populate(&mut self) {
        self.portal = false;
        self.portal_timer = 0;
        self.max_speed = 0.0;
        for index in 0..200 {
            self.block_type[index] = 0;
            self.block_color[index] = ((index / 20) % 6) as u8;
            self.block_health[index] = 1;
            self.block_max_health[index] = 1;
            self.block_x[index] = 20.0 + (index % 20) as f32 * 30.0;
            self.block_y[index] = 100.0 + (index / 20) as f32 * BLOCK_HEIGHT;
        }
        self.blocks = levels::generate(self.level);
        if !self.editor.patterns.is_empty() && self.level > 9 {
            let cells = levels::generate_cells(self.level, &self.editor.patterns);
            self.blocks = [0; 200];
            for (index, cell) in cells.iter().enumerate() {
                if *cell <= 8 {
                    self.set_block(index, if *cell < 6 { 0 } else { *cell - 5 });
                    self.block_color[index] = if *cell < 6 { *cell } else { 0 };
                }
            }
        }
    }
    pub fn attach(&mut self) {
        self.penguin = None;
        self.stolen_heart = None;
        self.long = 0;
        self.ghost = 0;
        self.ammo = 0;
        self.width = 70.0;
        self.paddle = 285.0;
        self.last_paddle = self.paddle;
        self.paddle_glow = 0.0;
        self.lost_life = false;
        self.balls.clear();
        self.respawn();
        self.drops.clear();
        self.rockets.clear();
        self.particles.clear();
        self.particle_spawns.clear();
    }
    pub fn respawn(&mut self) {
        self.balls.push(Ball::new(
            self.paddle + self.width / 2.0,
            PADDLE_Y - BALL_RADIUS,
        ));
    }
    pub fn finish_level(&mut self) {
        if self.lost_life && self.lives < 3 {
            self.lives += 1;
        }
        self.mode = if self.level == 9 { 5 } else { 3 };
        self.unlocked = self.unlocked.max(self.level.saturating_add(1));
        self.dirty = true;
    }
    pub fn burst(&mut self, x: f32, y: f32, color: u32) {
        let count = below(6) + 10;
        for _ in 0..count {
            let choices = random_pair();
            let angle = choices[0] * core::f32::consts::TAU;
            let speed = choices[1] * 2.0 + 1.0;
            self.particles.push(Particle::new(
                x,
                y,
                angle.cos() * speed,
                angle.sin() * speed - 1.0,
                color,
            ));
        }
    }
    pub fn collide_balls(&mut self) {
        if self.portal {
            return;
        }
        // Snapshot contacts before responses, as the original does. Coordinates
        // are ball centers and distances/velocities are half original units.
        let mut contacts = Vec::new();
        for i in 0..self.balls.len() {
            for j in i + 1..self.balls.len() {
                let first = &self.balls[i];
                let second = &self.balls[j];
                if !first.active() || !second.active() {
                    continue;
                }
                let dx = second.x - first.x;
                let dy = second.y - first.y;
                if (dx * dx + dy * dy).sqrt() < 6.0 {
                    contacts.push((i, j, first.x + dx / 2.0, first.y + dy / 2.0));
                }
            }
        }
        for (i, j, x, y) in contacts {
            self.balls[i].vy = -self.balls[i].vy.abs().max(4.0);
            self.balls[i].vx = if self.balls[i].x < x { -2.5 } else { 2.5 };
            self.balls[j].vy = -self.balls[j].vy.abs().max(4.0);
            self.balls[j].vx = if self.balls[j].x < x { -2.5 } else { 2.5 };
            for k in 0..36 {
                let angle = (k as f32 * 10.0).to_radians();
                self.particles.push(Particle::new(
                    x,
                    y,
                    angle.cos() * 3.0,
                    angle.sin() * 3.0,
                    0xc8ffffff,
                ));
            }
            self.sound = 1;
        }
    }
    pub fn award(&mut self, points: u32) {
        self.score = self.score.saturating_add(points);
        // Persist only completed-level/lost-life checkpoints, never every frame.
        if self.score > self.best {
            self.best = self.score;
        }
    }
    pub fn set_block(&mut self, index: usize, kind: u8) -> bool {
        if index >= 200 || kind > 3 {
            return false;
        }
        self.blocks[index] = 1;
        self.block_type[index] = kind;
        self.block_health[index] = if kind == 1 {
            2
        } else if kind == 3 {
            255
        } else {
            1
        };
        self.block_max_health[index] = self.block_health[index];
        true
    }
    pub fn destroy(&mut self, index: usize) {
        if self.blocks[index] == 0 {
            return;
        }
        self.blocks[index] = 0;
        self.award(10);
        self.sound = 1;
        let colors = [
            0xff0000ff, 0xffa500ff, 0xffff00ff, 0x00ff00ff, 0x0000ffff, 0x8a2be2ff,
        ];
        self.particle_spawns.push((
            self.block_x[index] + BLOCK_WIDTH / 2.0,
            self.block_y[index] + BLOCK_HEIGHT / 2.0,
            colors[self.block_color[index] as usize % 6],
        ));
    }
    pub fn hit(&mut self, index: usize) -> bool {
        if self.blocks[index] == 0 {
            return false;
        }
        if self.block_type[index] == 3 {
            self.sound = 1;
            return false;
        }
        if self.block_type[index] == 1 {
            self.block_health[index] = self.block_health[index].saturating_sub(1);
            if self.block_health[index] != 0 {
                self.sound = 2;
                return false;
            }
        }
        self.destroy(index);
        let x = self.block_x[index];
        let y = self.block_y[index];
        let roll = below(100);
        if roll < 15 && self.cooldown >= 60 {
            let weight = below(100);
            let kind = if weight < 50 {
                0
            } else if weight < 75 {
                1
            } else if weight < 90 {
                2
            } else {
                3
            };
            self.drops.push(Drop {
                x: x + BLOCK_WIDTH / 2.0,
                y,
                kind,
            });
            self.cooldown = 0;
        }
        true
    }
    pub fn explode(&mut self, x: f32, y: f32, rocket: bool) {
        for index in 0..200 {
            if self.blocks[index] == 0 {
                continue;
            }
            let dx = self.block_x[index] + BLOCK_WIDTH / 2.0 - x;
            let dy = self.block_y[index] + BLOCK_HEIGHT / 2.0 - y;
            let squared = dx * dx + dy * dy;
            // These different radii and sqrt/squared comparisons are deliberate:
            // rocket code uses 120 original pixels; explosive bricks use 60.
            let inside = if rocket {
                squared.sqrt() <= 60.0
            } else {
                squared <= 30.0 * 30.0
            };
            if inside {
                self.destroy(index);
            }
        }
        // Blast damage bypasses health/metal armor and never chains or drops bonuses.
        self.sound = if rocket { 2 } else { 4 };
    }
    pub fn collide_blocks(&mut self, ball: usize) {
        if ball >= self.balls.len() || !self.balls[ball].active() {
            return;
        }
        let mut explosions = Vec::new();
        for index in 0..200 {
            if self.blocks[index] == 0 {
                continue;
            }
            let x = self.block_x[index];
            let y = self.block_y[index];
            if !self.balls[ball].resolve_block(x, y, self.ghost > 0) {
                continue;
            }
            if self.hit(index) && self.block_type[index] == 2 {
                explosions.push((x + BLOCK_WIDTH / 2.0, y + BLOCK_HEIGHT / 2.0));
            }
            if self.ghost == 0 {
                break;
            }
        }
        for (x, y) in explosions {
            self.explode(x, y, false);
        }
    }
    pub fn update_rockets(&mut self) {
        let mut i = 0;
        while i < self.rockets.len() {
            self.rockets[i].1 -= 4.0;
            let (rx, ry) = self.rockets[i];
            let mut struck = 200;
            for index in 0..200 {
                let x = self.block_x[index];
                let y = self.block_y[index];
                if self.blocks[index] > 0
                    && ry >= TOP
                    && overlap([rx, ry, 5.0, 10.0], [x, y, BLOCK_WIDTH, BLOCK_HEIGHT]).0 > 0.0
                {
                    struck = index;
                    break;
                }
            }
            if struck < 200 {
                let x = self.block_x[struck] + BLOCK_WIDTH / 2.0;
                let y = self.block_y[struck] + BLOCK_HEIGHT / 2.0;
                self.destroy(struck);
                self.explode(x, y, true);
                self.rockets.remove(i);
            } else if ry < TOP {
                self.rockets.remove(i);
            } else {
                i += 1;
            }
        }
    }
    pub fn collect(&mut self, kind: u32) {
        self.award(2);
        match kind {
            0 => {
                if self.width == 70.0 {
                    let center = self.paddle + self.width / 2.0;
                    self.width = 90.0;
                    self.paddle = center - self.width / 2.0;
                }
                self.long = 300;
                self.paddle = self.paddle.clamp(0.0, 640.0 - self.width);
            }
            1 => {
                // Original bonus initially spawns offset from the paddle; on the
                // following attached update it aligns with the normal launch point.
                self.balls.push(Ball::new(
                    self.paddle + self.width / 2.0 + BALL_RADIUS,
                    PADDLE_Y - 7.0,
                ));
            }
            2 => self.ghost = 600,
            _ => self.ammo = self.ammo.saturating_add(1),
        }
    }
    pub fn update_paddle(&mut self, input: u32) -> f32 {
        if input & 257 != 0 {
            self.paddle = (self.paddle - 9.0).max(0.0);
        }
        if input & 514 != 0 {
            self.paddle = (self.paddle + 9.0).min(640.0 - self.width);
        }
        let movement = self.paddle - self.last_paddle;
        self.last_paddle = self.paddle;
        self.paddle_glow *= 0.9;
        if self.paddle_glow < 0.01 {
            self.paddle_glow = 0.0;
        }
        if self.long > 0 {
            self.long -= 1;
            if self.long == 0 {
                let center = self.paddle + self.width / 2.0;
                self.width = 70.0;
                self.paddle = (center - self.width / 2.0).clamp(0.0, 640.0 - self.width);
            }
        }
        if self.ghost > 0 {
            self.ghost -= 1;
        }
        movement
    }
    pub fn fire_or_launch(&mut self) {
        let mut launched = false;
        for ball in &mut self.balls {
            if ball.attached {
                ball.launch();
                for _ in 0..20 {
                    let choices = random_pair();
                    let angle = choices[0] * core::f32::consts::TAU;
                    let speed = choices[1] * 2.0 + 1.0;
                    self.particles.push(Particle::new(
                        ball.x,
                        ball.y,
                        angle.cos() * speed,
                        angle.sin() * speed,
                        0xffc832ff,
                    ));
                }
                launched = true;
            }
        }
        if !launched && self.ammo > 0 {
            self.ammo -= 1;
            self.rockets
                .push((self.paddle + self.width / 2.0 - 2.5, PADDLE_Y - 10.0));
            self.sound = 3;
        }
    }
    pub fn frame_clock(&mut self, now: u64) {
        let mut kept = 0usize;
        for index in 0..self.fps_times.len() {
            if now.saturating_sub(self.fps_times[index]) < 1000000 {
                self.fps_times[kept] = self.fps_times[index];
                kept += 1;
            }
        }
        self.fps_times.truncate(kept);
        self.fps_times.push(now);
        self.fps = self.fps_times.len() as u32;
    }
    pub fn capture_sound(&mut self) {
        if self.sound != 0 {
            self.effects
                .effect_request(self.sound, self.frame, self.menu.sfx_muted);
            self.sound = 0;
        }
    }
    pub fn reset_run(&mut self) {
        let effects = core::mem::replace(&mut self.effects, crate::audio::Mixer::new());
        let fps = self.fps;
        let fps_times = core::mem::take(&mut self.fps_times);
        let splash_timer = self.splash_timer;
        let started = self.started;
        let best = self.best;
        let unlocked = self.unlocked;
        let gravity = self.gravity;
        let sfx_volume = self.menu.sfx_volume;
        let sfx_muted = self.menu.sfx_muted;
        let fullscreen = self.menu.fullscreen;
        let fullscreen_requests = self.fullscreen_requests;
        let homepage_requested = self.homepage_requested;
        let saved_editor = core::mem::replace(&mut self.editor, crate::editor::Editor::new());
        *self = Self::new();
        self.effects = effects;
        self.fps = fps;
        self.fps_times = fps_times;
        self.splash_timer = splash_timer;
        self.started = started;
        self.editor = saved_editor;
        self.fullscreen_requests = fullscreen_requests;
        self.homepage_requested = homepage_requested;
        self.best = best;
        self.unlocked = unlocked;
        self.gravity = gravity;
        self.menu.sfx_volume = sfx_volume;
        self.menu.sfx_muted = sfx_muted;
        self.menu.fullscreen = fullscreen;
        self.dirty = true;
    }
    pub fn editor_test(&mut self) {
        self.attach();
        self.portal = false;
        self.portal_timer = 0;
        self.max_speed = 0.0;
        self.score = 0;
        self.lives = 3;
        self.testing = true;
        self.mode = 1;
        self.blocks = [0; 200];
        for index in 0..200 {
            let cell = self.editor.cells[index];
            if cell <= 8 {
                self.set_block(index, if cell < 6 { 0 } else { cell - 5 });
                self.block_color[index] = if cell < 6 { cell } else { 0 };
                self.block_health[index] = self.editor.health[index];
                self.block_max_health[index] = self.editor.health[index];
            }
            self.block_x[index] = 20.0 + (index % 20) as f32 * 30.0;
            self.block_y[index] = 100.0 + (index / 20) as f32 * BLOCK_HEIGHT;
        }
    }
    pub fn cursor_confined(&self) -> bool {
        self.mode == 1 || self.mode == 3 || self.mode == 4 || self.mode == 5
    }
    pub fn source_input(&mut self, kind: u32, x: f32, y: f32, code: u32) {
        if self.quit_requested {
            return;
        }
        if kind == 5 && code == 68 {
            self.fullscreen_requests = self.fullscreen_requests.saturating_add(1);
            return;
        }
        if self.mode == 7 {
            if (kind == 5 && (code & 65535) == 40) || (kind == 2 && code == 1) {
                self.show_menu();
            }
            return;
        }
        if self.mode == 0 || self.mode == 2 {
            let action = self.menu.menu_event(kind, x, y, code);
            if action == 1 {
                self.menu.settings = false;
                self.started = true;
                self.dirty = true;
                self.mode = 1;
            }
            if action == 2 {
                self.started = true;
                self.reset_run();
            }
            if action == 3 {
                self.gravity = !self.gravity;
                self.dirty = true;
            }
            if action == 4 {
                self.mode = 6;
                self.testing = false;
                self.editor.dragging = 0;
            }
            if action == 5 {
                self.menu.settings = true;
                self.menu.hover = 0;
            }
            if action == 6 {
                self.quit_requested = true;
                self.dirty = true;
                return;
            }
            if action == 9 {
                self.homepage_requested = true;
            }
            if action == 7 {
                self.fullscreen_requests = self.fullscreen_requests.saturating_add(1);
            }
            if action == 10 {
                self.menu.sfx_muted = !self.menu.sfx_muted;
                self.dirty = true;
            }
            if action == 11 {
                self.dirty = true;
            }
            if action == 8 {
                self.menu.settings = false;
                self.menu.hover = 0;
            }
            if action != 0 {
                return;
            }
        }
        if self.mode == 6 {
            let action = self.editor.editor_event(kind, x, y, code);
            if action == 1 {
                self.show_menu();
                self.menu.settings = false;
                self.menu.hover = 0;
                self.editor.dragging = 0;
            }
            if action == 2 {
                self.editor_test();
            }
            if action == 3 || action == 4 {
                self.editor.file_requested = action - 2;
            }
            if self.editor.save_requested {
                self.dirty = true;
            }
            return;
        }
        if kind == 9 && (code & 0x7fff_ffff) == 101 && (self.mode == 0 || self.mode == 2) {
            self.mode = 6;
            self.menu.settings = false;
            self.menu.hover = 0;
            self.testing = false;
            self.editor.dragging = 0;
            return;
        }
        if kind == 1 && self.mode == 1 && x.is_finite() && y.is_finite() {
            let original_x = (x.clamp(0.0, 640.0) * 2.0) as i32;
            let half_width = (self.width * 2.0) as i32 / 2;
            self.paddle = ((original_x - half_width) as f32 * 0.5).clamp(0.0, 640.0 - self.width);
        }
        if kind == 2 && code == 1 && (self.mode == 3 || self.mode == 5) {
            self.update(4);
        }
        if kind == 5 {
            let key = code & 65535;
            if key == 44 && self.mode == 1 {
                self.fire_or_launch();
            }
            if key == 41 && (self.mode == 0 || self.mode == 1 || self.mode == 2) {
                self.mode = if self.mode == 1 { 2 } else { 1 };
                self.menu.settings = false;
                self.menu.hover = 0;
            }
            if key == 40 && (self.mode == 3 || self.mode == 5) {
                self.update(4);
            }
        }
        if kind == 9 {
            let key = code & 0x7fff_ffff;
            if key == 113 && self.mode == 1 {
                self.blocks = [0; 200];
            } else if key == 113
                && (self.mode == 0 || self.mode == 2 || self.mode == 4 || self.mode == 5)
            {
                self.quit_requested = true;
                self.dirty = true;
                return;
            }
            if key == 114 && (self.mode == 0 || self.mode == 2 || self.mode == 4 || self.mode == 5)
            {
                self.reset_run();
            }
        }
    }
    pub fn update(&mut self, input: u32) {
        if self.quit_requested {
            return;
        }
        self.editor.editor_tick();
        if self.mode == 7 {
            // The original main loop owns this timer across Game::reset calls.
            self.splash_timer = self.splash_timer.saturating_add(1);
            if self.splash_timer >= 270 || input & 4 != 0 {
                self.show_menu();
            }
            return;
        }
        if self.mode == 6 {
            self.editor.frame_count = self.editor.frame_count.wrapping_add(1);
            return;
        }
        self.update_frame(input);
        if self.testing && (self.mode == 3 || self.mode == 4 || self.mode == 5) {
            self.mode = 6;
            self.testing = false;
            self.editor.dragging = 0;
        }
    }
    pub fn show_menu(&mut self) {
        self.mode = if self.started { 2 } else { 0 };
        self.menu.settings = false;
        self.menu.hover = 0;
    }
    pub fn update_frame(&mut self, input: u32) {
        if input & 131072 != 0 {
            self.gravity = !self.gravity;
            self.dirty = true;
        }
        if input & 8 != 0 && (self.mode == 1 || self.mode == 2) {
            self.mode = if self.mode == 1 { 2 } else { 1 };
            self.menu.settings = false;
            self.menu.hover = 0;
            if self.mode == 2 {
                return;
            }
        }
        if self.mode != 1 {
            if input & 4 != 0 {
                if self.mode == 0 {
                    self.started = true;
                    self.level = self.unlocked;
                    self.populate();
                    self.attach();
                }
                if self.mode == 3 || self.mode == 5 {
                    self.level = self.level.saturating_add(1);
                    self.populate();
                    self.attach();
                }
                if self.mode == 4 {
                    self.reset_run();
                }
                self.mode = 1;
            }
            return;
        }
        self.frame = self.frame.wrapping_add(1);
        // Launch/fire are input events before the original per-frame update.
        if input & 4 != 0 {
            self.fire_or_launch();
            self.capture_sound();
        }
        let movement = self.update_paddle(input);
        self.cooldown = self.cooldown.saturating_add(1);
        for i in 0..self.balls.len() {
            let orbiting = self.portal;
            if !orbiting {
                if self.balls[i].attached {
                    self.balls[i].x = self.paddle + self.width / 2.0;
                    self.balls[i].y = PADDLE_Y - BALL_RADIUS;
                    if self.balls[i].launch_timer > 0 {
                        self.balls[i].launch_timer -= 1;
                    } else {
                        self.balls[i].launch();
                    }
                } else {
                    self.balls[i].advance(self.gravity);
                }
                let ball = &self.balls[i];
                let speed = (ball.vx * ball.vx + ball.vy * ball.vy).sqrt() * 120.0;
                if speed > self.max_speed {
                    self.max_speed = speed;
                    self.max_speed_record_frame = self.frame;
                    let length = (ball.vx * ball.vx + ball.vy * ball.vy).sqrt();
                    if length > 0.05 {
                        let dx = -ball.vx / length;
                        let dy = -ball.vy / length;
                        for _ in 0..5 {
                            let choices = random_pair();
                            let angle = dy.atan2(dx) + choices[0] - 0.5;
                            let drift = choices[1] + 0.5;
                            let choice = below(12);
                            let color = if choice == 0 {
                                0x00ffffff
                            } else if choice == 1 {
                                0x64c8ffff
                            } else {
                                0xc8ffffff
                            };
                            self.particles.push(Particle::new(
                                ball.x - dx * 5.0,
                                ball.y - dy * 5.0,
                                angle.cos() * drift,
                                angle.sin() * drift,
                                color,
                            ));
                        }
                    }
                }
                if self.max_speed >= 3600.0 {
                    self.portal = true;
                    self.award(5000);
                    for _ in 0..100 {
                        let choices = random_pair();
                        let angle = choices[0] * core::f32::consts::TAU;
                        let speed = choices[1] * 7.5 + 2.5;
                        self.particles.push(Particle::new(
                            320.0,
                            240.0,
                            angle.cos() * speed,
                            angle.sin() * speed,
                            0x9632ffff,
                        ));
                    }
                }
            }
            if orbiting {
                let angle = self.frame as f32 * 0.1 + i as f32 * (core::f32::consts::TAU / 3.0);
                self.balls[i].x = 320.0 + angle.cos() * 75.0;
                self.balls[i].y = 240.0 + angle.sin() * 75.0;
            }
        }
        self.collide_balls();
        for i in 0..self.balls.len() {
            if !self.balls[i].active() {
                continue;
            }
            let paddle_overlap = self.balls[i].intersects(self.paddle, PADDLE_Y, self.width, 11.0);
            if paddle_overlap.0 > 0.0 {
                let ball = &mut self.balls[i];
                ball.vy = -ball.vy.abs();
                let original_center = ((ball.x - BALL_RADIUS) * 2.0) as i32 + 6;
                let paddle_center = (self.paddle * 2.0) as i32 + (self.width * 2.0) as i32 / 2;
                let offset = (original_center - paddle_center) as f32 * 0.5;
                ball.vx += offset * 0.1;
                ball.spin = if movement.abs() > 1.0 {
                    self.paddle_glow = 1.0;
                    movement * 0.3 + offset * 0.05
                } else {
                    offset * 0.02
                };
                self.award(5);
                self.sound = 1;
            }
            self.collide_blocks(i);
        }
        self.update_rockets();
        let queued = core::mem::take(&mut self.particle_spawns);
        for (x, y, color) in queued {
            self.burst(x, y, color);
        }
        let mut i = 0;
        while i < self.drops.len() {
            self.drops[i].y += 1.0;
            let drop = &self.drops[i];
            if overlap(
                [drop.x, drop.y, 20.0, 20.0],
                [self.paddle, PADDLE_Y, self.width, 11.0],
            )
            .0 > 0.0
                && drop.y <= BOTTOM
            {
                let kind = drop.kind;
                self.drops.remove(i);
                self.collect(kind);
            } else if drop.y > BOTTOM {
                self.drops.remove(i);
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        if self.portal {
            self.pull_blocks();
        }
        if let Some(penguin) = self.penguin.as_mut() {
            penguin.advance_penguin();
            if penguin.state == 1 {
                self.stolen_heart = None;
            }
            if penguin.state == 3 {
                self.penguin = None;
            }
        }
        while i < self.particles.len() {
            let particle = &mut self.particles[i];
            particle.advance();
            if particle.life == 0 {
                self.particles.remove(i);
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.balls.len() {
            if !self.balls[i].active() {
                self.balls.remove(i);
            } else {
                i += 1;
            }
        }
        if self.balls.is_empty() && !self.portal {
            self.lives = self.lives.saturating_sub(1);
            self.lost_life = true;
            self.score = self.score.saturating_sub(20);
            self.dirty = true;
            self.sound = 5;
            let heart_x = 1250.0 - self.lives as f32 * 25.0;
            self.stolen_heart = Some([heart_x, 25.0]);
            self.penguin = Some(Penguin::new(heart_x, 25.0));
            if self.lives == 0 {
                self.mode = 4;
            } else {
                self.respawn();
            }
        }
        let mut remaining = 0;
        for index in 0..200 {
            if self.block_type[index] != 3 {
                remaining += self.blocks[index] as u32;
            }
        }
        if remaining == 0 && !self.portal {
            self.finish_level();
        }
    }
    pub fn pull_blocks(&mut self) {
        let mut empty = true;
        for index in 0..200 {
            if self.blocks[index] == 0 {
                continue;
            }
            empty = false;
            let dx = 320.0 - (self.block_x[index] + BLOCK_WIDTH / 2.0);
            let dy = 240.0 - (self.block_y[index] + BLOCK_HEIGHT / 2.0);
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 2.5 {
                self.block_x[index] += (dx / distance * 8.0) as i32 as f32 * 0.5;
                self.block_y[index] += (dy / distance * 8.0) as i32 as f32 * 0.5;
            } else {
                self.blocks[index] = 0;
                for _ in 0..5 {
                    let choices = random_pair();
                    let angle = choices[0] * core::f32::consts::TAU;
                    let speed = choices[1] * 1.5;
                    self.particles.push(Particle::new(
                        320.0,
                        240.0,
                        angle.cos() * speed,
                        angle.sin() * speed,
                        0x9632ffff,
                    ));
                }
            }
        }
        if empty {
            self.portal_timer += 1;
            if self.portal_timer >= 270 {
                self.finish_level();
            }
        }
    }
}

pub fn speed_record_color(frame: u64, record: u64) -> [u8; 3] {
    if frame.saturating_sub(record) < 120 {
        let pulse = (frame as f32 * 0.15).sin() * 0.3 + 0.7;
        [
            (255.0 * pulse) as u8,
            (255.0 * pulse) as u8,
            (100.0 * pulse + 100.0) as u8,
        ]
    } else {
        [200, 200, 200]
    }
}
pub fn gravity_color(frame: u64) -> [u8; 3] {
    let pulse = (frame as f32 * 0.1).sin() * 0.3 + 0.7;
    [(255.0 * pulse) as u8, (100.0 * pulse) as u8, 0]
}
