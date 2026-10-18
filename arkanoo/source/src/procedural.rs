// Adapted from the pinned draw_animated_background; copyright 2025 c0m4r, WTFPL 2.
use crate::background::rectangle;
use crate::background::Background;
use crate::pattern_rng::Rng;
fn seeded_background_rng(level: u32) -> Rng {
    let level = u64::from(level);
    let seed = level
        .wrapping_mul(12345)
        .wrapping_add((level / 3).wrapping_mul(67890))
        .wrapping_add((level % 7).wrapping_mul(11111));
    Rng::new(seed)
}
pub fn procedural_theme(level: u32) -> u32 {
    let mut rng = seeded_background_rng(level);
    rng.range_i32(0, 20) as u32
}
pub fn procedural_background(level: u32, frame: u64) -> Option<Background> {
    if level < 10 {
        return None;
    }
    match procedural_theme(level) {
        0 => {
            let (color, segments) = cyber_segments(level, frame);
            Some(segment_background(segments, color))
        }
        1 => Some(draw_star_voyage(level, frame)),
        3 => {
            let (color, segments) = aurora_segments(level, frame);
            Some(segment_background(segments, color))
        }
        6 => Some(segment_background(rain_segments(level, frame), 0x0a0a14ff)),
        7 => Some(draw_snow(level, frame)),
        9 => Some(draw_smoke(level, frame)),
        11 => Some(draw_fire(level, frame)),
        13 => Some(draw_kaleidoscope(frame)),
        14 => Some(segment_background(warp_segments(level, frame), 0x000000ff)),
        16 => Some(draw_supernova(level, frame)),
        17 => Some(draw_space_wars(level, frame)),
        18 => Some(draw_deep_sea(level, frame)),
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
pub fn smoke_rectangles(
    level: u32,
    frame: u64,
) -> (u32, Vec<crate::background::BackgroundRectangle>) {
    let time = frame as f32;
    let mut rectangles = Vec::new();
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
        rectangles.push(crate::background::BackgroundRectangle {
            position: [x as i32 - size as i32 / 2, y as i32 - size as i32 / 2],
            size: [size, size],
            color: 0x64646400 | u32::from(opacity),
        });
    }
    (0x141414ff, rectangles)
}
fn draw_smoke(level: u32, frame: u64) -> Background {
    let (color, rectangles) = smoke_rectangles(level, frame);
    let mut dots = Vec::new();
    for rect in rectangles {
        rectangle(
            &mut dots,
            rect.position[0],
            rect.position[1],
            rect.size[0] as i32,
            rect.size[1] as i32,
            rect.color,
        );
    }
    Background {
        color,
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
fn draw_fire(level: u32, frame: u64) -> Background {
    let time = frame as f32;
    let mut dots = Vec::new();
    for i in 0..300u64 {
        let mut rng = Rng::new(u64::from(level) * 7000 + i);
        let x_base = rng.range_i32(0, 1280);
        let speed = rng.float(2.0, 5.0);
        let y_anim = (time * speed + rng.float(0.0, 1000.0)) % (720.0 / 1.5);
        let y = 720.0 - y_anim;
        let progress = 1.0 - y_anim / (720.0 / 1.5);
        let color = if progress > 0.8 {
            0xffffc8ff
        } else if progress > 0.6 {
            0xffc800c8
        } else if progress > 0.3 {
            0xff320096
        } else {
            0x64323264
        };
        let wiggle = (time * 0.1 + i as f32).sin() * 5.0;
        let x = x_base as f32 + wiggle;
        let size = (progress * 8.0) as u32 + 2;
        rectangle(
            &mut dots,
            x as i32,
            y as i32,
            size as i32,
            size as i32,
            color,
        );
    }
    Background {
        color: 0x140500ff,
        points: dots,
    }
}
fn draw_deep_sea(level: u32, frame: u64) -> Background {
    let time = frame as f32;
    // Five shafts are at most 120×720, followed by 200 plankton points.
    // Reserve once so growing the list cannot retain old and new buffers.
    let mut dots = Vec::with_capacity(5 * 120 * 720 + 200);
    for i in 0..5 {
        let x = i * 200 + (time * 0.1).sin() as i32 * 50;
        let width = 100 + (time * 0.05 + i as f32).cos() as i32 * 20;
        for height in (0..720).step_by(20) {
            let opacity = (50.0 * (1.0 - height as f32 / 720.0)) as u8;
            rectangle(
                &mut dots,
                x,
                height,
                width,
                20,
                0x64c8ff00 | u32::from(opacity),
            );
        }
    }
    for i in 0..200u64 {
        let mut rng = Rng::new(u64::from(level) * 13000 + i);
        let x_base = rng.range_i32(0, 1280);
        let y_base = rng.range_i32(0, 720);
        let drift_x = (time * 0.02 + i as f32).sin() * 20.0;
        let drift_y = (time * 0.03 + i as f32).cos() * 20.0;
        dots.push(crate::physics::EffectPoint {
            x: (x_base as f32 + drift_x) as i32,
            y: (y_base as f32 + drift_y) as i32,
            color: 0xc8ffc896,
        });
    }
    Background {
        color: 0x000514ff,
        points: dots,
    }
}
fn draw_supernova(level: u32, frame: u64) -> Background {
    let time = frame as f32;
    let mut dots = Vec::new();
    let pulse = (time * 0.1).sin() * 20.0 + 50.0;
    rectangle(
        &mut dots,
        640 - pulse as i32 / 2,
        360 - pulse as i32 / 2,
        pulse as i32,
        pulse as i32,
        0xffc864c8,
    );
    for i in 0..5 {
        let wave_dist = (time * 2.0 + i as f32 * 100.0) % 600.0;
        let opacity = (255.0 * (1.0 - wave_dist / 600.0)) as u8;
        for p in 0..32 {
            let angle = (p as f32 / 32.0) * core::f32::consts::PI * 2.0;
            let x = 640.0 + angle.cos() * wave_dist;
            let y = 360.0 + angle.sin() * wave_dist;
            dots.push(crate::physics::EffectPoint {
                x: x as i32,
                y: y as i32,
                color: 0xff643200 | u32::from(opacity),
            });
        }
    }
    for i in 0..100u64 {
        let mut rng = Rng::new(u64::from(level) * 11000 + i);
        let angle = rng.float(0.0, core::f32::consts::PI * 2.0);
        let speed = rng.float(1.0, 5.0);
        let distance = (time * speed + rng.float(0.0, 1000.0)) % 800.0;
        let x = 640.0 + angle.cos() * distance;
        let y = 360.0 + angle.sin() * distance;
        let red = rng.range_u8(100, 255);
        let green = rng.range_u8(50, 150);
        let blue = rng.range_u8(50, 100);
        let color =
            (u32::from(red) << 24) | (u32::from(green) << 16) | (u32::from(blue) << 8) | 255;
        rectangle(&mut dots, x as i32, y as i32, 3, 3, color);
    }
    Background {
        color: 0x050005ff,
        points: dots,
    }
}
fn draw_space_wars(level: u32, frame: u64) -> Background {
    let time = frame as f32;
    let mut dots = Vec::new();
    for i in 0..20u64 {
        let mut rng = Rng::new(u64::from(level) * 12000 + i);
        let interval = rng.range(50, 200);
        let offset = rng.range(0, 200);
        let phase = (time as u64).wrapping_add(offset) % interval;
        if phase < 10 {
            let y = rng.range_i32(0, 720);
            let color = if rng.half_probability() {
                0xff0000c8
            } else {
                0x00ff00c8
            };
            let x_start = phase as f32 * 30.0;
            rectangle(&mut dots, x_start as i32, y, 100, 4, color);
        }
    }
    Background {
        color: 0x00000aff,
        points: dots,
    }
}

fn draw_star_voyage(level: u32, frame: u64) -> Background {
    let mut rng = seeded_background_rng(level);
    rng.range_i32(0, 20);
    let red = rng.range_u8(0, 20);
    let green = rng.range_u8(0, 20);
    let blue = rng.range_u8(10, 40);
    let color = (u32::from(red) << 24) | (u32::from(green) << 16) | (u32::from(blue) << 8) | 255;
    let stars = rng.range(80, 200);
    let multiplier = rng.float(0.5, 2.5);
    let tint_red = rng.range_u8(150, 255);
    let tint_green = rng.range_u8(150, 255);
    let tint_blue = rng.range_u8(150, 255);
    let tint =
        (u32::from(tint_red) << 24) | (u32::from(tint_green) << 16) | (u32::from(tint_blue) << 8);
    let time = frame as f32;
    let mut dots = Vec::new();
    for i in 0..stars {
        let mut star_rng = Rng::new(u64::from(level) * 1000 + i);
        let angle = star_rng.float(0.0, core::f32::consts::PI * 2.0);
        let speed = star_rng.float(0.5, 2.0) * multiplier;
        let start_distance = star_rng.float(0.0, 800.0);
        let distance = (start_distance + time * speed) % 700.0;
        let x = 640.0 + angle.cos() * distance;
        let y = 360.0 + angle.sin() * distance;
        let size = (distance / 200.0).max(1.0) as u32;
        let opacity = (distance / 700.0 * 255.0) as u8;
        if (0.0..1280.0).contains(&x) && (0.0..720.0).contains(&y) {
            rectangle(
                &mut dots,
                x as i32,
                y as i32,
                size as i32,
                size as i32,
                tint | u32::from(opacity),
            );
        }
    }
    Background {
        color,
        points: dots,
    }
}
pub fn kaleidoscope_segments(frame: u64) -> Vec<crate::background::BackgroundLine> {
    let time = frame as f32;
    let mut segments = Vec::new();
    for layer in 0..5 {
        let radius = 100.0 + layer as f32 * 80.0;
        let rotation =
            time * (0.01 + layer as f32 * 0.005) * if layer % 2 == 0 { 1.0 } else { -1.0 };
        let red = (100 + layer * 30) as u8;
        let green = (50 + layer * 20) as u8;
        let blue = (200 - layer * 20) as u8;
        let color =
            (u32::from(red) << 24) | (u32::from(green) << 16) | (u32::from(blue) << 8) | 100;
        for i in 0..12 {
            let angle = (i as f32 / 12.0) * core::f32::consts::PI * 2.0 + rotation;
            let x = 640.0 + angle.cos() * radius;
            let y = 360.0 + angle.sin() * radius;
            let size = (20 + layer * 5) as f32;
            let vertices = [
                [x as i32, (y - size) as i32],
                [(x + size) as i32, y as i32],
                [x as i32, (y + size) as i32],
                [(x - size) as i32, y as i32],
            ];
            for side in 0..4 {
                segments.push(crate::background::BackgroundLine {
                    start: vertices[side],
                    end: vertices[(side + 1) % 4],
                    color,
                });
            }
        }
    }
    segments
}
fn draw_kaleidoscope(frame: u64) -> Background {
    let mut dots = Vec::new();
    for segment in kaleidoscope_segments(frame) {
        crate::background::background_line_points(
            &mut dots,
            segment.start,
            segment.end,
            [1280, 720],
            segment.color,
        );
    }
    Background {
        color: 0x0a000aff,
        points: dots,
    }
}
pub fn rain_segments(level: u32, frame: u64) -> Vec<crate::background::BackgroundLine> {
    let time = frame as f32;
    let mut segments = Vec::new();
    for i in 0..400u64 {
        let mut rng = Rng::new(u64::from(level) * 2000 + i);
        let x_base = rng.range_i32(0, 1480);
        let speed = rng.float(15.0, 25.0);
        let length = rng.range_i32(10, 20);
        let y_anim = (time * speed + rng.float(0.0, 1000.0)) % 770.0;
        let y = y_anim as i32 - 20;
        let x = x_base - (y as f32 * 0.2) as i32;
        if (0..1280).contains(&x) && (0..720).contains(&y) {
            segments.push(crate::background::BackgroundLine {
                start: [x, y],
                end: [x - 2, y + length],
                color: 0x9696c896,
            });
        }
    }
    segments
}
pub fn warp_segments(level: u32, frame: u64) -> Vec<crate::background::BackgroundLine> {
    let time = frame as f32;
    let mut segments = Vec::new();
    for i in 0..300u64 {
        let mut rng = Rng::new(u64::from(level) * 9000 + i);
        let angle = rng.float(0.0, core::f32::consts::PI * 2.0);
        let speed = rng.float(2.0, 10.0);
        let base = (time * speed + rng.float(0.0, 1000.0)) % 1000.0;
        let distance = base.powf(1.5) * 0.05;
        let x = 640.0 + angle.cos() * distance;
        let y = 360.0 + angle.sin() * distance;
        if (0.0..1280.0).contains(&x) && (0.0..720.0).contains(&y) {
            let opacity = (distance / 2.0).min(255.0) as u8;
            let color = if speed > 8.0 { 0xc8c8ff00 } else { 0xffc8c800 } | u32::from(opacity);
            let tail = speed * 2.0;
            let tail_x = x - angle.cos() * tail;
            let tail_y = y - angle.sin() * tail;
            segments.push(crate::background::BackgroundLine {
                start: [x as i32, y as i32],
                end: [tail_x as i32, tail_y as i32],
                color,
            });
        }
    }
    segments
}
fn segment_background(segments: Vec<crate::background::BackgroundLine>, color: u32) -> Background {
    let mut dots = Vec::new();
    for segment in segments {
        crate::background::background_line_points(
            &mut dots,
            segment.start,
            segment.end,
            [1280, 720],
            segment.color,
        );
    }
    Background {
        color,
        points: dots,
    }
}

pub fn cyber_segments(level: u32, frame: u64) -> (u32, Vec<crate::background::BackgroundLine>) {
    let mut rng = seeded_background_rng(level);
    rng.range_i32(0, 20);
    let r = rng.range_u8(0, 50);
    let g = rng.range_u8(0, 50);
    let b = rng.range_u8(40, 100);
    let background =
        (u32::from(r / 2) << 24) | (u32::from(g / 2) << 16) | (u32::from(b / 2) << 8) | 255;
    let horizon = 720 / rng.range_i32(2, 4);
    let spacing = rng.range_i32(60, 120);
    let speed = rng.float(0.3, 0.8);
    let offset = (frame as f32 * speed) % 40.0;
    let red = rng.range_u8(r, 255);
    let green = rng.range_u8(g, 255);
    let blue = rng.range_u8(b, 255);
    let color = (u32::from(red) << 24) | (u32::from(green) << 16) | (u32::from(blue) << 8) | 100;
    let mut segments = Vec::new();
    for i in -15..25 {
        segments.push(crate::background::BackgroundLine {
            start: [640 + i * spacing, 720],
            end: [640 + i * 10, horizon],
            color,
        });
    }
    for i in 0..25 {
        let z = 1.0 + i as f32 * 0.5 - (offset / 40.0) * 0.5;
        if z > 0.1 {
            let y = horizon as f32 + (720.0 - horizon as f32) / z;
            if y < 720.0 {
                segments.push(crate::background::BackgroundLine {
                    start: [0, y as i32],
                    end: [1280, y as i32],
                    color,
                });
            }
        }
    }
    (background, segments)
}
pub fn aurora_segments(level: u32, frame: u64) -> (u32, Vec<crate::background::BackgroundLine>) {
    let mut rng = seeded_background_rng(level);
    rng.range_i32(0, 20);
    let r = rng.range_u8(5, 25);
    let g = rng.range_u8(5, 25);
    let b = rng.range_u8(10, 35);
    let background = (u32::from(r) << 24) | (u32::from(g) << 16) | (u32::from(b) << 8) | 255;
    let waves = rng.range_i32(3, 8);
    let vertical = rng.half_probability();
    let mut segments = Vec::new();
    for i in 0..waves {
        let red = rng.range_u8(0, 100);
        let green = rng.range_u8(50, 200);
        let blue = rng.range_u8(100, 255);
        let opacity = rng.range_u8(20, 50);
        let color = (u32::from(red) << 24)
            | (u32::from(green) << 16)
            | (u32::from(blue) << 8)
            | u32::from(opacity);
        let offset = i as f32 * 40.0;
        let speed = rng.float(0.01, 0.03);
        let amplitude = rng.float(30.0, 80.0);
        if vertical {
            for x in (0..1280).step_by(5) {
                let base = 360.0 + offset - (waves as f32 * 20.0);
                let wave = (x as f32 * 0.01 + frame as f32 * speed).sin() * amplitude;
                let y = (base + wave) as i32;
                segments.push(crate::background::BackgroundLine {
                    start: [x, y],
                    end: [x, y + 100],
                    color,
                });
            }
        } else {
            for y in (0..720).step_by(5) {
                let base = 640.0 + offset - (waves as f32 * 20.0);
                let wave = (y as f32 * 0.01 + frame as f32 * speed).sin() * amplitude;
                let x = (base + wave) as i32;
                segments.push(crate::background::BackgroundLine {
                    start: [x, y],
                    end: [x + 100, y],
                    color,
                });
            }
        }
    }
    (background, segments)
}

pub fn matrix_rectangles(
    level: u32,
    frame: u64,
) -> (u32, Vec<crate::background::BackgroundRectangle>) {
    let mut rng = seeded_background_rng(level);
    rng.range_i32(0, 20);
    let streams = rng.range(30, 60);
    let multiplier = rng.float(0.5, 1.5);
    let time = frame as f32;
    let mut rectangles = Vec::new();
    for i in 0..streams {
        let mut stream_rng = Rng::new(u64::from(level) * 1000 + i);
        let x = stream_rng.range_i32(0, 1280);
        let speed = stream_rng.float(2.0, 5.0) * multiplier;
        let length = stream_rng.range_i32(5, 15);
        let head = ((time * speed) as i32).wrapping_add(stream_rng.range_i32(0, 500)) % 920 - 100;
        for j in 0..length {
            let y = head - j * 15;
            if (0..720).contains(&y) {
                let opacity = if j == 0 {
                    255
                } else {
                    (255 - j * 20).max(0) as u8
                };
                let green = if j == 0 { 255 } else { 150 };
                rectangles.push(crate::background::BackgroundRectangle {
                    position: [x, y],
                    size: [8, 12],
                    color: (green << 16) | u32::from(opacity),
                });
            }
        }
    }
    (0x000a00ff, rectangles)
}
pub fn nebula_rectangles(
    level: u32,
    frame: u64,
) -> (u32, Vec<crate::background::BackgroundRectangle>) {
    let mut rng = seeded_background_rng(level);
    rng.range_i32(0, 20);
    let r = rng.range_u8(10, 30);
    let g = rng.range_u8(10, 30);
    let b = rng.range_u8(20, 40);
    let color = (u32::from(r) << 24) | (u32::from(g) << 16) | (u32::from(b) << 8) | 255;
    let clouds = rng.range(15, 30);
    let time = frame as f32;
    let mut rectangles = Vec::new();
    for i in 0..clouds {
        let mut cloud_rng = Rng::new(u64::from(level) * 1000 + i);
        let x_base = cloud_rng.range_i32(0, 1280);
        let y_base = cloud_rng.range_i32(0, 720);
        let size = cloud_rng.range_i32(50, 150);
        let red = cloud_rng.range_u8(50, 150);
        let green = cloud_rng.range_u8(0, 100);
        let blue = cloud_rng.range_u8(100, 200);
        let x_drift = ((time * 0.05 + i as f32).sin() * 50.0) as i32;
        let y_drift = ((time * 0.03 + i as f32 * 0.5).cos() * 30.0) as i32;
        let x = x_base + x_drift;
        let y = y_base + y_drift;
        let tint = (u32::from(red) << 24) | (u32::from(green) << 16) | (u32::from(blue) << 8) | 10;
        for s in (0..size).step_by(10) {
            let side = size - s;
            let offset = s / 2;
            rectangles.push(crate::background::BackgroundRectangle {
                position: [x + offset, y + offset],
                size: [side as u32, side as u32],
                color: tint,
            });
        }
    }
    (color, rectangles)
}

pub fn hex_commands(level: u32, frame: u64) -> (u32, Vec<crate::background::BackgroundCommand>) {
    let mut rng = seeded_background_rng(level);
    rng.range_i32(0, 20);
    let r = rng.range_u8(5, 40);
    let g = rng.range_u8(5, 40);
    let b = rng.range_u8(5, 40);
    let color = (u32::from(r) << 24) | (u32::from(g) << 16) | (u32::from(b) << 8) | 255;
    let side = rng.range_i32(20, 60) as u32;
    let columns = 1280 / side + 2;
    let rows = 720 / side + 2;
    let speed = rng.float(0.01, 0.05);
    let red = rng.range_u8(50, 200);
    let green = rng.range_u8(50, 200);
    let blue = rng.range_u8(50, 200);
    let tint = (u32::from(red) << 24) | (u32::from(green) << 16) | (u32::from(blue) << 8);
    let mut commands = Vec::new();
    for row in 0..rows {
        for column in 0..columns {
            let offset = if row % 2 == 0 { 0 } else { side / 2 };
            let x = (column * side) as i32 + offset as i32;
            let y = (row * side) as i32;
            let pulse =
                ((frame as f32 * speed + column as f32 * 0.2 + row as f32 * 0.2).sin() + 1.0) / 2.0;
            let opacity = (10.0 + pulse * 40.0) as u8;
            commands.push(crate::background::BackgroundCommand {
                kind: 2,
                values: [x, y, (side - 2) as i32, (side - 2) as i32],
                color: tint | u32::from(opacity),
            });
        }
    }
    (color, commands)
}
pub fn bubble_commands(level: u32, frame: u64) -> (u32, Vec<crate::background::BackgroundCommand>) {
    let mut commands = Vec::new();
    for i in 0..150u64 {
        let mut rng = Rng::new(u64::from(level) * 6000 + i);
        let x_base = rng.range_i32(0, 1280);
        let speed = rng.float(0.5, 2.0);
        let size = rng.range_i32(2, 10);
        let phase = (frame as f32 * speed + rng.float(0.0, 1000.0)) % 770.0;
        let y = 720.0 - phase;
        let sway = (frame as f32 * 0.02 + i as f32).sin() * 10.0;
        let x = x_base as f32 + sway;
        commands.push(crate::background::BackgroundCommand {
            kind: 2,
            values: [x as i32, y as i32, size, size],
            color: 0x64c8ff64,
        });
        commands.push(crate::background::BackgroundCommand {
            kind: 0,
            values: [x as i32 + 1, y as i32 + 1, 0, 0],
            color: 0xffffff96,
        });
    }
    (0x001428ff, commands)
}

pub fn night_commands(level: u32, frame: u64) -> (u32, Vec<crate::background::BackgroundCommand>) {
    let time = frame as f32;
    let mut commands = Vec::new();
    for i in 0..200u64 {
        let mut rng = Rng::new(u64::from(level) * 4000 + i);
        let x = rng.range_i32(0, 1280);
        let y = rng.range_i32(0, 720);
        let side = if rng.ten_percent() { 2 } else { 1 };
        let speed = rng.float(0.05, 0.2);
        let opacity = ((time * speed + i as f32).sin() * 100.0 + 155.0) as u8;
        commands.push(crate::background::BackgroundCommand {
            kind: 3,
            values: [x, y, side, side],
            color: 0xffffff00 | u32::from(opacity),
        });
    }
    let sat_x = (time * 0.5) % 1330.0 - 20.0;
    let sat_y = 100.0 + (time * 0.01).sin() * 20.0;
    commands.push(crate::background::BackgroundCommand {
        kind: 3,
        values: [sat_x as i32, sat_y as i32, 3, 3],
        color: 0xc8c8ffff,
    });
    if (time as u64 / 30) % 2 == 0 {
        commands.push(crate::background::BackgroundCommand {
            kind: 0,
            values: [sat_x as i32 + 1, sat_y as i32 + 1, 0, 0],
            color: 0xff0000ff,
        });
    }
    let mut rng = Rng::new(((time * 0.005) as u64).wrapping_add(u64::from(level)));
    if rng.thirty_percent() {
        let progress = (time % 200.0) / 200.0;
        if progress < 0.3 {
            let x = rng.range_i32(0, 1280) as f32 + progress * 800.0;
            let y = rng.range_i32(0, 360) as f32 + progress * 600.0;
            commands.push(crate::background::BackgroundCommand {
                kind: 1,
                values: [x as i32, y as i32, (x - 40.0) as i32, (y - 30.0) as i32],
                color: 0xffffffc8,
            });
        }
    }
    (0x00000aff, commands)
}
pub fn circuit_commands(
    level: u32,
    frame: u64,
) -> (u32, Vec<crate::background::BackgroundCommand>) {
    let time = frame as f32;
    let mut commands = Vec::new();
    for row in 0..8u32 {
        for column in 0..13u32 {
            let x = (column * 100) as i32;
            let y = (row * 100) as i32;
            let opacity = ((time * 0.05 + (column + row) as f32).sin() * 100.0 + 100.0) as u8;
            commands.push(crate::background::BackgroundCommand {
                kind: 3,
                values: [x - 2, y - 2, 5, 5],
                color: 0x00ff0000 | u32::from(opacity),
            });
            let mut rng =
                Rng::new(u64::from(level) * 8000 + u64::from(row) * 100 + u64::from(column));
            if rng.seventy_percent() {
                let phase = (time * 2.0) % 100.0;
                commands.push(crate::background::BackgroundCommand {
                    kind: 1,
                    values: [x, y, x + 100, y],
                    color: 0x00640064,
                });
                commands.push(crate::background::BackgroundCommand {
                    kind: 3,
                    values: [x + phase as i32, y - 1, 4, 3],
                    color: 0x64ff64c8,
                });
            }
            if rng.seventy_percent() {
                let phase = (time * 2.0 + 50.0) % 100.0;
                commands.push(crate::background::BackgroundCommand {
                    kind: 1,
                    values: [x, y, x, y + 100],
                    color: 0x00640064,
                });
                commands.push(crate::background::BackgroundCommand {
                    kind: 3,
                    values: [x - 1, y + phase as i32, 3, 4],
                    color: 0x64ff64c8,
                });
            }
        }
    }
    (0x000a00ff, commands)
}
pub fn maze_commands(level: u32, frame: u64) -> (u32, Vec<crate::background::BackgroundCommand>) {
    let time = frame as f32;
    let mut commands = Vec::new();
    for row in 0..19u32 {
        for column in 0..33u32 {
            let mut rng =
                Rng::new(u64::from(level) * 10000 + u64::from(row) * 100 + u64::from(column));
            let x = (column * 40) as i32;
            let y = (row * 40) as i32;
            let values = if rng.half_probability() {
                [x, y, x + 40, y + 40]
            } else {
                [x + 40, y, x, y + 40]
            };
            commands.push(crate::background::BackgroundCommand {
                kind: 1,
                values,
                color: 0x1e1e3cff,
            });
        }
    }
    let x = (time * 2.0) % 1280.0;
    let y = (time * 1.5) % 720.0;
    commands.push(crate::background::BackgroundCommand {
        kind: 3,
        values: [x as i32 - 3, y as i32 - 3, 6, 6],
        color: 0x00ff00ff,
    });
    (0x0a0a14ff, commands)
}
