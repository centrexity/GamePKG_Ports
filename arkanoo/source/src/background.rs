// Adapted from draw_animated_background in the pinned original rendering.rs.
// Copyright 2025 c0m4r, WTFPL version 2; see CREDITS.md.
use crate::physics::EffectPoint;
pub struct Background {
    pub color: u32,
    pub points: Vec<EffectPoint>,
}
pub fn rectangle(dots: &mut Vec<EffectPoint>, x: i32, y: i32, width: i32, height: i32, color: u32) {
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
// Altered Rust adaptation of SDL RenderDrawLineBresenham's default point path.
// Copyright 1997-2023 Sam Lantinga; see assets/LICENSE-SDL.txt (zlib).
pub fn background_line_points(
    dots: &mut Vec<EffectPoint>,
    start: [i32; 2],
    end: [i32; 2],
    size: [i32; 2],
    color: u32,
) {
    let Some(clipped) = clipped_background_line(start, end, size) else {
        return;
    };
    let start = [clipped[0], clipped[1]];
    let end = [clipped[2], clipped[3]];
    let dx = (end[0] - start[0]).abs();
    let dy = (end[1] - start[1]).abs();
    let sx = if start[0] > end[0] { -1 } else { 1 };
    let sy = if start[1] > end[1] { -1 } else { 1 };
    let mut x = start[0];
    let mut y = start[1];
    if dx >= dy {
        let mut error = 2 * dy - dx;
        for _ in 0..=dx {
            dots.push(EffectPoint { x, y, color });
            if error < 0 {
                error += dy * 2;
            } else {
                error += (dy - dx) * 2;
                y += sy;
            }
            x += sx;
        }
    } else {
        let mut error = 2 * dx - dy;
        for _ in 0..=dy {
            dots.push(EffectPoint { x, y, color });
            if error < 0 {
                error += dx * 2;
            } else {
                error += (dx - dy) * 2;
                x += sx;
            }
            y += sy;
        }
    }
}

fn background_clip_code(point: [i32; 2], size: [i32; 2]) -> u32 {
    let mut code = 0;
    if point[1] < 0 {
        code |= 1;
    } else if point[1] >= size[1] {
        code |= 2;
    }
    if point[0] < 0 {
        code |= 4;
    } else if point[0] >= size[0] {
        code |= 8;
    }
    code
}
fn clipped_background_line(
    mut start: [i32; 2],
    mut end: [i32; 2],
    size: [i32; 2],
) -> Option<[i32; 4]> {
    if size[0] <= 0 || size[1] <= 0 {
        return None;
    }
    loop {
        let first = background_clip_code(start, size);
        let second = background_clip_code(end, size);
        if first | second == 0 {
            return Some([start[0], start[1], end[0], end[1]]);
        }
        if first & second != 0 {
            return None;
        }
        let code = if first != 0 { first } else { second };
        let x1 = i128::from(start[0]);
        let y1 = i128::from(start[1]);
        let x2 = i128::from(end[0]);
        let y2 = i128::from(end[1]);
        let (x, y) = if code & 3 != 0 {
            let y = if code & 1 != 0 {
                0
            } else {
                i128::from(size[1] - 1)
            };
            (x1 + (x2 - x1) * (y - y1) / (y2 - y1), y)
        } else {
            let x = if code & 4 != 0 {
                0
            } else {
                i128::from(size[0] - 1)
            };
            (x, y1 + (y2 - y1) * (x - x1) / (x2 - x1))
        };
        if first != 0 {
            start = [x as i32, y as i32];
        } else {
            end = [x as i32, y as i32];
        }
    }
}
