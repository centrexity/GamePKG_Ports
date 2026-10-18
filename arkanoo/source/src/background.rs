// Adapted from draw_animated_background in the pinned original rendering.rs.
// Copyright 2025 c0m4r, WTFPL version 2; see CREDITS.md.
use crate::physics::EffectPoint;
pub struct Background {
    pub color: u32,
    pub points: Vec<EffectPoint>,
}
fn rectangle(dots: &mut Vec<EffectPoint>, x: i32, y: i32, width: i32, height: i32, color: u32) {
    for row in y.max(0)..(y + height).min(720) {
        for column in x.max(0)..(x + width).min(1280) {
            dots.push(EffectPoint {
                x: column,
                y: row,
                color,
            });
        }
    }
}
pub fn campaign(level: u32, frame: u64) -> Option<Background> {
    let mut dots = Vec::new();
    let time = frame as f32;
    let color = match level {
        7 => {
            for i in 0i32..100 {
                let x = (i * 137).wrapping_add((time * 0.5 * i as f32 * 0.01) as i32) % 1280;
                let y = (i * 241) % 720;
                let brightness = ((time * 0.05 + i as f32 * 0.5).sin() * 127.0 + 128.0) as u8;
                let size = 1 + i % 3;
                let color = (u32::from(brightness) << 24)
                    | (u32::from(brightness) << 16)
                    | 0xff00
                    | u32::from(brightness);
                rectangle(&mut dots, x, y, size, size, color);
            }
            0x050514ff
        }
        8 => {
            for ring in 1..=6 {
                let pulse = (time * 0.05 + ring as f32 * 0.5).sin();
                let radius = (ring as f32 * 80.0 + pulse * 20.0) as i32;
                let opacity = ((pulse + 1.0) * 60.0) as u8;
                let color = 0x64329600 | u32::from(opacity);
                for angle in 0..360 {
                    let radians = (angle as f32).to_radians();
                    let x = 640 + (radius as f32 * radians.cos()) as i32;
                    let y = 360 + (radius as f32 * radians.sin()) as i32;
                    dots.push(EffectPoint { x, y, color });
                }
            }
            0x0a050fff
        }
        9 => {
            for column in 0..40 {
                let offset = ((time * 2.0 + column as f32 * 10.0) as i32) % 820;
                for i in 0..10 {
                    let y = offset - i * 20;
                    if (0..720).contains(&y) {
                        let opacity = if i == 0 { 255 } else { 255 - i * 25 };
                        let green = if i == 0 { 255 } else { 150 - i * 15 };
                        rectangle(
                            &mut dots,
                            column * 32,
                            y,
                            2,
                            15,
                            (green as u32) << 16 | opacity as u32,
                        );
                    }
                }
            }
            0x000000ff
        }
        _ => return None,
    };
    Some(Background {
        color,
        points: dots,
    })
}
