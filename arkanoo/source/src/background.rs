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
                    // Round from double precision before rasterization: native
                    // single-precision CRT trig can move an integer pixel.
                    let cosine = (radians as f64).cos() as f32;
                    let sine = (radians as f64).sin() as f32;
                    let x = 640 + (radius as f32 * cosine) as i32;
                    let y = 360 + (radius as f32 * sine) as i32;
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
pub struct BackgroundLine {
    pub start: [i32; 2],
    pub end: [i32; 2],
    pub color: u32,
}
// Precompose the recorded default SDL/OpenGL opaque color convention: source
// and destination products round separately before addition. Untouched pixels
// remain sparse/transparent, preserving tile counts and drawing budgets.
// Repeat encoded RGBA bytes with bounded block copies rather than one Vec
// append per source pixel. This retains hidden RGB and alpha exactly.
fn filled_background_pixels(width: u32, height: u32, color: u32) -> Vec<u8> {
    let length = width as usize * height as usize * 4;
    let mut pixels = Vec::with_capacity(length);
    if length == 0 {
        return pixels;
    }
    let background = u32::to_be_bytes(color);
    pixels.extend_from_slice(&[background[0], background[1], background[2], 0]);
    while pixels.len() < length {
        let count = pixels.len().min(length - pixels.len());
        pixels.extend_from_within(0..count);
    }
    pixels
}
pub fn composited_background(scene: &Background) -> Option<crate::physics::ParticleImage> {
    let mut left = 1280i32;
    let mut top = 720i32;
    let mut right = -1i32;
    let mut bottom = -1i32;
    for dot in &scene.points {
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
    let mut pixels = filled_background_pixels(width, height, scene.color);
    for dot in &scene.points {
        let opacity = dot.color & 255;
        if dot.x >= left && dot.x <= right && dot.y >= top && dot.y <= bottom && opacity != 0 {
            let at = ((dot.y - top) as usize * width as usize + (dot.x - left) as usize) * 4;
            for channel in 0..3 {
                let value = (dot.color >> (24 - channel * 8)) & 255;
                pixels[at + channel] = (((value * opacity + 127) / 255)
                    + ((u32::from(pixels[at + channel]) * (255 - opacity) + 127) / 255))
                    as u8;
            }
            pixels[at + 3] = 255;
        }
    }
    Some(crate::physics::ParticleImage {
        left,
        top,
        width,
        height,
        pixels,
    })
}

pub struct BackgroundRectangle {
    pub position: [i32; 2],
    pub size: [u32; 2],
    pub color: u32,
}
// Replay original filled rectangles directly into the canonical logical image.
// Do not retain a point for every overlapping translucent rectangle pixel.
pub fn composited_rectangles(
    rectangles: &[BackgroundRectangle],
    color: u32,
) -> Option<crate::physics::ParticleImage> {
    let mut left = 1280i32;
    let mut top = 720i32;
    let mut right = -1i32;
    let mut bottom = -1i32;
    for rect in rectangles {
        let l = rect.position[0].max(0);
        let t = rect.position[1].max(0);
        let r = (i64::from(rect.position[0]) + i64::from(rect.size[0].max(1)) - 1).min(1279) as i32;
        let b = (i64::from(rect.position[1]) + i64::from(rect.size[1].max(1)) - 1).min(719) as i32;
        if rect.color & 255 != 0 && l <= r && t <= b {
            left = left.min(l);
            top = top.min(t);
            right = right.max(r);
            bottom = bottom.max(b);
        }
    }
    if left > right || top > bottom {
        return None;
    }
    let width = (right - left + 1) as u32;
    let height = (bottom - top + 1) as u32;
    let mut pixels = filled_background_pixels(width, height, color);
    for rect in rectangles {
        let opacity = rect.color & 255;
        if opacity == 0 {
            continue;
        }
        let l = rect.position[0].max(left);
        let t = rect.position[1].max(top);
        let r = (i64::from(rect.position[0]) + i64::from(rect.size[0].max(1)) - 1)
            .min(i64::from(right)) as i32;
        let b = (i64::from(rect.position[1]) + i64::from(rect.size[1].max(1)) - 1)
            .min(i64::from(bottom)) as i32;
        for y in t..=b {
            for x in l..=r {
                let at = ((y - top) as usize * width as usize + (x - left) as usize) * 4;
                for channel in 0..3 {
                    let value = (rect.color >> (24 - channel * 8)) & 255;
                    pixels[at + channel] = (((value * opacity + 127) / 255)
                        + ((u32::from(pixels[at + channel]) * (255 - opacity) + 127) / 255))
                        as u8;
                }
                pixels[at + 3] = 255;
            }
        }
    }
    Some(crate::physics::ParticleImage {
        left,
        top,
        width,
        height,
        pixels,
    })
}

// Original SDL default rectangle outlines are four endpoint-exclusive strips.
// Altered scalar Rust adaptation of SDL RenderDrawLinesWithRectsF; same zlib
// attribution and license as the line helper above.
pub struct BackgroundCommand {
    // 0: point; 1: line endpoint; 2: outline dimensions; 3: fill dimensions.
    pub kind: u32,
    pub values: [i32; 4],
    pub color: u32,
}
pub fn background_command_points(dots: &mut Vec<EffectPoint>, command: &BackgroundCommand) {
    let [x, y, a, b] = command.values;
    match command.kind {
        0 => rectangle(dots, x, y, 1, 1, command.color),
        1 => background_line_points(dots, [x, y], [a, b], [1280, 720], command.color),
        2 => {
            let w = a.max(1);
            let h = b.max(1);
            if w == 1 && h == 1 {
                rectangle(dots, x, y, 1, 1, command.color);
            } else {
                rectangle(dots, x, y, w - 1, 1, command.color);
                rectangle(dots, x + w - 1, y, 1, h - 1, command.color);
                rectangle(dots, x + 1, y + h - 1, w - 1, 1, command.color);
                rectangle(dots, x, y + 1, 1, h - 1, command.color);
            }
        }
        3 => rectangle(dots, x, y, a.max(1), b.max(1), command.color),
        _ => (),
    }
}
// Preserve ordered primitive blends without retaining an expanded outline grid.
pub fn composited_commands(
    commands: &[BackgroundCommand],
    color: u32,
) -> Option<crate::physics::ParticleImage> {
    let mut rectangles = Vec::new();
    for command in commands {
        let [x, y, a, b] = command.values;
        match command.kind {
            0 => rectangles.push(BackgroundRectangle {
                position: [x, y],
                size: [1, 1],
                color: command.color,
            }),
            1 => {
                let mut dots = Vec::new();
                background_line_points(&mut dots, [x, y], [a, b], [1280, 720], command.color);
                for dot in dots {
                    rectangles.push(BackgroundRectangle {
                        position: [dot.x, dot.y],
                        size: [1, 1],
                        color: dot.color,
                    });
                }
            }
            2 => {
                let w = a.max(1);
                let h = b.max(1);
                if w == 1 && h == 1 {
                    rectangles.push(BackgroundRectangle {
                        position: [x, y],
                        size: [1, 1],
                        color: command.color,
                    });
                }
                if w > 1 {
                    rectangles.push(BackgroundRectangle {
                        position: [x, y],
                        size: [(w - 1) as u32, 1],
                        color: command.color,
                    });
                }
                if h > 1 {
                    rectangles.push(BackgroundRectangle {
                        position: [x + w - 1, y],
                        size: [1, (h - 1) as u32],
                        color: command.color,
                    });
                }
                if w > 1 {
                    rectangles.push(BackgroundRectangle {
                        position: [x + 1, y + h - 1],
                        size: [(w - 1) as u32, 1],
                        color: command.color,
                    });
                }
                if h > 1 {
                    rectangles.push(BackgroundRectangle {
                        position: [x, y + 1],
                        size: [1, (h - 1) as u32],
                        color: command.color,
                    });
                }
            }
            3 => rectangles.push(BackgroundRectangle {
                position: [x, y],
                size: [a.max(1) as u32, b.max(1) as u32],
                color: command.color,
            }),
            _ => (),
        }
    }
    composited_rectangles(&rectangles, color)
}
