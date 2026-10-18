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
        1 => Some(draw_star_voyage(level, frame)),
        7 => Some(draw_snow(level, frame)),
        9 => Some(draw_smoke(level, frame)),
        11 => Some(draw_fire(level, frame)),
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
    let mut dots = Vec::new();
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
