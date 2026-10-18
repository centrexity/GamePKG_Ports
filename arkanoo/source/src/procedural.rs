// Adapted from the pinned draw_animated_background; copyright 2025 c0m4r, WTFPL 2.
use crate::background::rectangle;
use crate::background::Background;
use crate::pattern_rng::Rng;
pub fn procedural_theme(level: u32) -> u32 {
    let level = u64::from(level);
    let seed = level
        .wrapping_mul(12345)
        .wrapping_add((level / 3).wrapping_mul(67890))
        .wrapping_add((level % 7).wrapping_mul(11111));
    let mut rng = Rng::new(seed);
    rng.range_i32(0, 20) as u32
}
pub fn procedural_background(level: u32, frame: u64) -> Option<Background> {
    if level < 10 {
        return None;
    }
    match procedural_theme(level) {
        7 => Some(draw_snow(level, frame)),
        9 => Some(draw_smoke(level, frame)),
        19 => Some(draw_tornado(level, frame)),
        _ => None,
    }
}
fn draw_snow(level: u32, frame: u64) -> Background {
    let time = frame as f32;
    let mut dots = Vec::new();
    for i in 0..300u64 {
        let seed = u64::from(level) * 3000 + i;
        let mut rng = Rng::new(seed);
        let x_base = rng.range_i32(0, 1280);
        let speed = rng.float(1.0, 3.0);
        let size = rng.range_i32(1, 3);
        let y = (time * speed + rng.float(0.0, 1000.0)) % 730.0;
        let drift = (time * 0.02 + i as f32).sin() * 20.0;
        let x = (x_base as f32 + drift) as i32;
        let opacity = rng.range_u8(100, 255);
        rectangle(
            &mut dots,
            x,
            y as i32 - 5,
            size,
            size,
            0xffffff00 | u32::from(opacity),
        );
    }
    Background {
        color: 0x050a14ff,
        points: dots,
    }
}
fn draw_smoke(level: u32, frame: u64) -> Background {
    let time = frame as f32;
    let mut dots = Vec::new();
    for i in 0..100u64 {
        let seed = u64::from(level) * 5000 + i;
        let mut rng = Rng::new(seed);
        let x_base = rng.range_i32(0, 1280);
        let speed = rng.float(0.5, 1.5);
        let y_anim = (time * speed + rng.float(0.0, 1000.0)) % 820.0;
        let y = 720.0 - y_anim;
        let progress = 1.0 - y / 720.0;
        let size = (20.0 + progress * 60.0) as u32;
        let opacity = ((1.0 - progress) * 100.0) as u8;
        let drift = (time * 0.01 + i as f32).sin() * 30.0 * progress;
        let x = x_base as f32 + drift;
        rectangle(
            &mut dots,
            x as i32 - size as i32 / 2,
            y as i32 - size as i32 / 2,
            size as i32,
            size as i32,
            0x64646400 | u32::from(opacity),
        );
    }
    Background {
        color: 0x141414ff,
        points: dots,
    }
}
fn draw_tornado(level: u32, frame: u64) -> Background {
    let time = frame as f32;
    let mut dots = Vec::new();
    for i in 0..300u64 {
        let seed = u64::from(level) * 14000 + i;
        let mut rng = Rng::new(seed);
        let height_pct = rng.float(0.0, 1.0);
        let y = height_pct * 720.0;
        let width = 50.0 + height_pct * 300.0;
        let angle_speed = 0.1 + (1.0 - height_pct) * 0.2;
        let angle = time * angle_speed + rng.float(0.0, core::f32::consts::PI * 2.0);
        let radius = rng.float(width * 0.8, width);
        let x = 640.0 + angle.cos() * radius;
        let gray = rng.range_u8(100, 200);
        let color =
            (u32::from(gray) << 24) | (u32::from(gray) << 16) | (u32::from(gray) << 8) | 150;
        let size = rng.range_i32(2, 5);
        rectangle(&mut dots, x as i32, y as i32, size, size, color);
    }
    Background {
        color: 0x1e1e1eff,
        points: dots,
    }
}
