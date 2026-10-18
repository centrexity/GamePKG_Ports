use crate::gamepkg;
use crate::physics::{Game, BLOCK_HEIGHT, BLOCK_WIDTH, PADDLE_Y};
fn glyph(character: u8) -> u64 {
    match character {
        65 => 18842895918,
        66 => 32801506878,
        67 => 16660316687,
        68 => 32801080894,
        69 => 33840644639,
        70 => 17734517279,
        71 => 16695149071,
        72 => 18842895921,
        73 => 15170932878,
        75 => 18879369809,
        76 => 33840185872,
        77 => 18842572657,
        78 => 18842507057,
        79 => 15621211694,
        80 => 17734518334,
        63 => 4299228718,
        95 => 33285996544,
        82 => 18879563326,
        83 => 32247333391,
        84 => 4433514655,
        85 => 15621211697,
        86 => 4648912433,
        87 => 11464787505,
        89 => 4433521201,
        48 => 15629733422,
        49 => 15170933124,
        50 => 33558693422,
        51 => 32247317566,
        52 => 2247698626,
        53 => 32247857695,
        54 => 15621636622,
        55 => 8866891839,
        56 => 15621113390,
        57 => 15067498030,
        47 => 17725261857,
        74 => 13507823687,
        81 => 14585218606,
        88 => 18834663985,
        90 => 33831389247,
        45 => 1015808,
        37 => 224662361,
        _ => 0,
    }
}
fn letters(label: &[u8], x: f32, y: f32, scale: f32, color: u32) {
    let width = label.len() * 6;
    if width == 0 || width > 512 {
        return;
    }
    let mut pixels = Vec::new();
    pixels.resize(width * 7 * 4, 0u8);
    for (index, character) in label.iter().enumerate() {
        let bits = glyph(*character);
        for row in 0..7 {
            for col in 0..5 {
                if bits & (1u64 << (row * 5 + 4 - col)) != 0 {
                    let offset = (row * width + index * 6 + col) * 4;
                    pixels[offset] = (color >> 24) as u8;
                    pixels[offset + 1] = (color >> 16) as u8;
                    pixels[offset + 2] = (color >> 8) as u8;
                    pixels[offset + 3] = color as u8;
                }
            }
        }
    }
    gamepkg::draw_image(
        x,
        y,
        width as f32 * scale,
        7.0 * scale,
        width as u32,
        7,
        &pixels,
    );
}
pub fn text(label: &str, x: f32, y: f32, scale: f32, color: u32) {
    letters(label.as_bytes(), x, y, scale, color);
}
fn number(mut value: u32, x: f32, y: f32) {
    let mut digits = [48u8; 10];
    let mut count = 1;
    digits[9] = 48 + (value % 10) as u8;
    value /= 10;
    while value > 0 && count < 10 {
        digits[9 - count] = 48 + (value % 10) as u8;
        value /= 10;
        count += 1;
    }
    letters(
        &digits[10 - count..],
        x,
        y,
        if count > 7 { 1.0 } else { 2.0 },
        0xe5f3ffff,
    );
}

fn brick_line(pixels: &mut [u8], points: [i32; 4], rgba: [u8; 4]) {
    let [mut x, mut y, end_x, end_y] = points;
    let dx = (end_x - x).abs();
    let dy = -(end_y - y).abs();
    let step_x = if x < end_x { 1 } else { -1 };
    let step_y = if y < end_y { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        if x >= 0 && x < 60 && y >= 0 && y < 20 {
            let offset = (y as usize * 60 + x as usize) * 4;
            pixels[offset..offset + 4].copy_from_slice(&rgba);
        }
        if x == end_x && y == end_y {
            break;
        }
        let twice = error * 2;
        if twice >= dy {
            error += dy;
            x += step_x;
        }
        if twice <= dx {
            error += dx;
            y += step_y;
        }
    }
}
fn special_brick(game: &Game, index: usize, x: f32, y: f32) {
    brick_picture(
        game.block_type[index],
        game.block_health[index],
        game.block_max_health[index],
        game.frame,
        x,
        y,
    );
}
fn brick_picture(kind: u8, health: u8, max_health: u8, frame: u64, x: f32, y: f32) {
    let time = frame as f64 * 0.05;
    let rgba = if kind == 1 {
        [200, 240, 255, (180.0 + (time.sin() * 20.0)) as u8]
    } else if kind == 2 {
        [
            220u8.saturating_add((time.sin() * 20.0) as u8),
            80u8.saturating_add((time.cos() * 30.0) as u8),
            0,
            255,
        ]
    } else {
        [30, 35, 40, 255]
    };
    let mut pixels = Vec::new();
    pixels.resize(60 * 20 * 4, 0u8);
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&rgba);
    }
    if kind == 1 && health < max_health {
        brick_line(&mut pixels, [30, 10, 5, 5], [255, 255, 255, 220]);
        brick_line(&mut pixels, [30, 10, 55, 12], [255, 255, 255, 220]);
        brick_line(&mut pixels, [30, 10, 10, 18], [255, 255, 255, 220]);
        brick_line(&mut pixels, [5, 5, 0, 15], [255, 255, 255, 220]);
        brick_line(&mut pixels, [55, 12, 60, 5], [255, 255, 255, 220]);
    }
    if kind == 3 {
        for row in [2usize, 15] {
            for col in [2usize, 55] {
                for dy in 0..3 {
                    for dx in 0..3 {
                        let offset = ((row + dy) * 60 + col + dx) * 4;
                        pixels[offset..offset + 4].copy_from_slice(&[150, 150, 160, 255]);
                    }
                }
            }
        }
    }
    gamepkg::draw_image(x, y, BLOCK_WIDTH, BLOCK_HEIGHT, 60, 20, &pixels);
}
pub fn draw(game: &Game) {
    if game.mode == 7 {
        gamepkg::draw_clear(0x0b1020ff);
        text("ARKANOO", 236.0, 190.0, 4.0, 0x68e5ffff);
        text("ENTER OR CLICK TO CONTINUE", 170.0, 290.0, 2.0, 0x8ba2c9ff);
        return;
    }
    if game.mode == 0 || game.mode == 2 {
        draw_menu(game);
        return;
    }
    if game.mode == 6 {
        draw_editor(game);
        return;
    }
    gamepkg::draw_clear(0x0b1020ff);
    for n in 0..11 {
        gamepkg::draw_rect(0.0, 60.0 + n as f32 * 32.0, 640.0, 1.0, 0x17233aff);
    }
    gamepkg::draw_rect(0.0, 60.0, 1.0, 360.0, 0x345476ff);
    gamepkg::draw_rect(639.0, 60.0, 1.0, 360.0, 0x345476ff);
    text("ARKANOO", 16.0, 10.0, 2.0, 0x68e5ffff);
    text("SCORE", 136.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.score, 136.0, 23.0);
    text("BEST", 240.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.best, 240.0, 23.0);
    text("LEVEL", 352.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.level, 352.0, 23.0);
    text("LIVES", 446.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.lives, 446.0, 23.0);
    text("AMMO", 546.0, 8.0, 1.0, 0x8ba2c9ff);
    number(game.ammo, 546.0, 23.0);
    let colors = [
        0xff0000ff, 0xffa500ff, 0xffff00ff, 0x00ff00ff, 0x0000ffff, 0x8a2be2ff,
    ];
    for index in 0..200 {
        if game.blocks[index] > 0 {
            let x = game.block_x[index];
            let y = game.block_y[index];
            if game.block_type[index] == 0 {
                gamepkg::draw_rect(
                    x,
                    y,
                    BLOCK_WIDTH,
                    BLOCK_HEIGHT,
                    colors[game.block_color[index] as usize % 6],
                );
                gamepkg::draw_rect(x + 2.0, y + 2.0, 26.0, 1.0, 0xffffff65);
            } else {
                special_brick(game, index, x, y);
            }
        }
    }
    let color = if game.ghost > 0 {
        0xb18cffff
    } else {
        0x63dcffff
    };
    gamepkg::draw_rect(game.paddle, PADDLE_Y, game.width, 11.0, color);
    gamepkg::draw_rect(
        game.paddle + 3.0,
        PADDLE_Y + 1.0,
        game.width - 6.0,
        2.0,
        0xffffffff,
    );
    if game.gravity {
        text("GRAVITY", 266.0, 452.0, 1.0, 0xffcd60ff);
    }
    for ball in &game.balls {
        draw_ball_effects(ball,game.frame);
        gamepkg::draw_circle(ball.x, ball.y, 3.0, 0xffffffff);
    }
    for drop in &game.drops {
        let colors = [0x62de9eff, 0xffcd60ff, 0xb28cffff, 0xff647fff];
        gamepkg::draw_rect(drop.x, drop.y, 20.0, 20.0, colors[drop.kind as usize]);
        let label = match drop.kind {
            0 => "L",
            1 => "B",
            2 => "G",
            _ => "R",
        };
        text(label, drop.x + 4.0, drop.y + 3.0, 2.0, 0x101828ff);
    }
    for (x, y) in &game.rockets {
        gamepkg::draw_rect(*x, *y, 5.0, 10.0, 0xffdd70ff);
    }
    draw_particles(&game.particles);
    if let Some(penguin)=&game.penguin {draw_penguin(penguin);}
    if game.portal {
        for point in 0..48 {
            let angle = point as f32 * core::f32::consts::TAU / 48.0 + game.frame as f32 * 0.04;
            gamepkg::draw_circle(
                320.0 + angle.cos() * 75.0,
                240.0 + angle.sin() * 75.0,
                2.0,
                0xb777ffff,
            );
        }
    }
    draw_hearts(game);

    text(
        "ARROWS/A/D MOVE SPACE/UP FIRE ESC/DOWN PAUSE",
        42.0,
        461.0,
        1.0,
        0x8ba2c9ff,
    );
    text("V GRAVITY", 430.0, 461.0, 1.0, 0x8ba2c9ff);
    if game.mode != 1 {
        gamepkg::draw_rect(64.0, 268.0, 512.0, 138.0, 0x101b35ff);
        let title = match game.mode {
            0 => "ARKANOO",
            2 => "PAUSED",
            3 => "LEVEL CLEARED",
            5 => "VICTORY",
            _ => "GAME OVER",
        };
        text(title, 100.0, 284.0, 3.0, 0x68e5ffff);
        let detail = match game.mode {
            0 => "NINE LEVELS THEN ENDLESS PLAY",
            2 => "DOWN TO RESUME",
            3 => "UP FOR NEXT LEVEL",
            5 => "UP FOR ENDLESS MODE",
            _ => "UP TO PLAY AGAIN",
        };
        text(detail, 100.0, 323.0, 2.0, 0xe5f3ffff);
        if game.mode == 0 {
            text("UP TO START / E EDITOR", 100.0, 356.0, 2.0, 0xffdb70ff);
        }
        text(
            "L LONG   B BALL   G GHOST   R ROCKET",
            100.0,
            386.0,
            1.0,
            0x8ba2c9ff,
        );
    }
}
pub fn draw_penguin(penguin:&crate::physics::Penguin) {
    for shape in crate::physics::penguin_rects(penguin) {
        gamepkg::draw_rect(shape.x as f32*0.5,60.0+shape.y as f32*0.5,
            shape.width as f32*0.5,shape.height as f32*0.5,shape.color);
    }
}
fn heart_sprite(x:i32,y:i32,fallback_x:i32,fallback_y:i32) {
    if !gamepkg::draw_asset_image_linear("heart.png",x as f32*0.5,60.0+y as f32*0.5,10.0,10.0) {
        let dots=crate::physics::heart_points(fallback_x,fallback_y,12);
        if let Some(image)=crate::physics::effect_image(&dots) {draw_effect_layer(&image);}
    }
}
pub fn draw_hearts(game:&Game) {
    for index in 0..game.lives {
        let x=1240-index as i32*25;
        heart_sprite(x,15,x,20);
    }
    if let Some(position)=game.stolen_heart {
        heart_sprite(position[0] as i32-10,position[1] as i32-10,position[0] as i32,position[1] as i32);
    }
}
pub fn draw_particles(particles: &[crate::physics::Particle]) {
    if let Some(image)=crate::physics::particle_image(particles) {draw_effect_layer(&image);}
}
pub fn draw_ball_effects(ball: &crate::physics::Ball,frame:u64) {
    let dots=crate::physics::ball_effect_points(ball,frame);
    if let Some(image)=crate::physics::effect_image(&dots) {draw_effect_layer(&image);}
}
fn draw_effect_layer(image: &crate::physics::ParticleImage) {

        // Preserve the full layer in at most six non-overlapping 512x512 tiles.
        for top in (0..image.height).step_by(512) {
            for left in (0..image.width).step_by(512) {
                let width = (image.width - left).min(512);
                let height = (image.height - top).min(512);
                let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);
                let mut visible = false;
                for row in top..top + height {
                    let start = (row as usize * image.width as usize + left as usize) * 4;
                    let source = &image.pixels[start..start + width as usize * 4];
                    pixels.extend_from_slice(source);
                    if !visible {
                        for pixel in source.chunks_exact(4) {
                            if pixel[3] > 0 { visible = true; break; }
                        }
                    }
                }
                if visible {
                    gamepkg::draw_image((image.left as f32 + left as f32) * 0.5,
                        60.0 + (image.top as f32 + top as f32) * 0.5,
                        width as f32 * 0.5, height as f32 * 0.5, width, height, &pixels);
                }
            }
        }
    }
pub fn sound(kind: u32, frame: u64, volume: u32, muted: bool) {
    if muted || volume == 0 {
        return;
    }
    let samples = crate::audio::effect_samples(kind, frame, volume, false);
    gamepkg::audio_submit(&samples);
}

fn editor_name(label: &str, x: f32, y: f32, editing: bool) {
    let mut shown = String::new();
    let skip = if editing {
        label.chars().count().saturating_sub(78)
    } else {
        0
    };
    for character in label.chars().skip(skip).take(78) {
        shown.push(if character.is_ascii() {
            character.to_ascii_uppercase()
        } else {
            '?'
        });
    }
    text(&shown, x, y, 1.0, 0xe5f3ffff);
}
fn draw_editor(game: &Game) {
    let edit = &game.editor;
    let backgrounds = [
        0x0b1020ff, 0x10201bff, 0x201420ff, 0x201a10ff, 0x101c28ff, 0x201018ff,
    ];
    gamepkg::draw_clear(backgrounds[(edit.background.saturating_sub(1) as usize) % 6]);
    text("LEVEL EDITOR", 5.0, 65.0, 2.0, 0x68e5ffff);
    text(
        if edit.naming {
            "NAME / ENTER TO FINISH"
        } else {
            "NAME / N TO EDIT"
        },
        5.0,
        80.0,
        1.0,
        0x8ba2c9ff,
    );
    editor_name(&edit.name, 5.0, 92.0, edit.naming);
    let colors = [
        0xff0000ff, 0xffa500ff, 0xffff00ff, 0x00ff00ff, 0x0000ffff, 0x8a2be2ff, 0xc8f0ffff,
        0xdc5000ff, 0x404550ff,
    ];
    for chosen in 0..9 {
        let x = 430.0 + chosen as f32 * 22.5;
        if usize::from(edit.selected) == chosen {
            gamepkg::draw_rect(x - 1.0, 69.0, 22.0, 17.0, 0xffffffff);
        }
        gamepkg::draw_rect(x, 70.0, 20.0, 15.0, colors[chosen]);
        number(chosen as u32, x + 5.0, 88.0);
    }
    for row in 0..=10 {
        gamepkg::draw_rect(20.0, 100.0 + row as f32 * 10.0, 600.0, 0.5, 0x505064c8);
    }
    for col in 0..=20 {
        gamepkg::draw_rect(20.0 + col as f32 * 30.0, 100.0, 0.5, 100.0, 0x505064c8);
    }
    for index in 0..200 {
        let cell = edit.cells[index];
        let x = 20.0 + (index % 20) as f32 * 30.0;
        let y = 100.0 + (index / 20) as f32 * 10.0;
        if cell < 6 {
            gamepkg::draw_rect(x, y, 30.0, 10.0, colors[cell as usize]);
        }
        if (6..=8).contains(&cell) {
            brick_picture(
                cell - 5,
                edit.health[index],
                edit.health[index],
                game.frame,
                x,
                y,
            );
        }
    }
    text(
        "LEFT DRAG PAINT / RIGHT DRAG ERASE",
        20.0,
        220.0,
        1.0,
        0xe5f3ffff,
    );
    text(
        "0-5 NORMAL / 6 ICE / 7 EXPLOSIVE / 8 METAL",
        20.0,
        235.0,
        1.0,
        0x8ba2c9ff,
    );
    if edit.message_time > 0 {
        text(&edit.message, 20.0, 345.0, 1.0, 0xffdb70ff);
    }
    gamepkg::draw_rect(510.0, 365.0, 60.0, 17.5, 0x284260ff);
    text("BG PREV", 514.0, 370.0, 1.0, 0xffffffff);
    gamepkg::draw_rect(575.0, 365.0, 60.0, 17.5, 0x284260ff);
    text("BG NEXT", 579.0, 370.0, 1.0, 0xffffffff);
    for (index, label) in ["SAVE S", "CLEAR C", "TEST T", "LOAD L", "EXIT ESC"]
        .iter()
        .enumerate()
    {
        let x = 112.5 + index as f32 * 85.0;
        gamepkg::draw_rect(x, 390.0, 75.0, 20.0, 0x284260ff);
        text(label, x + 5.0, 397.0, 1.0, 0xffffffff);
    }
    gamepkg::draw_rect(20.0, 425.0, 75.0, 20.0, 0x284260ff);
    text("IMPORT I", 25.0, 432.0, 1.0, 0xffffffff);
    gamepkg::draw_rect(105.0, 425.0, 75.0, 20.0, 0x284260ff);
    text("EXPORT X", 110.0, 432.0, 1.0, 0xffffffff);
    if edit.browser {
        gamepkg::draw_rect(170.0, 115.0, 300.0, 250.0, 0x101b35ff);
        text(
            "PATTERNS / UP DOWN ENTER / ESC",
            180.0,
            125.0,
            1.0,
            0x68e5ffff,
        );
        let first = (edit.selection / 10) * 10;
        for (index, pattern) in edit.patterns.iter().enumerate().skip(first).take(10) {
            let y = 145.0 + (index - first) as f32 * 17.5;
            if index == edit.selection {
                gamepkg::draw_rect(180.0, y, 280.0, 15.0, 0x284260ff);
            }
            editor_name(&pattern.name, 185.0, y + 4.0, false);
        }
        if edit.patterns.is_empty() {
            text("NO SAVED PATTERNS", 180.0, 148.0, 1.0, 0xe5f3ffff);
        }
    }
}

fn draw_menu(game: &Game) {
    gamepkg::draw_clear(0x101b35ff);
    let heading = if game.menu.settings {
        "SETTINGS"
    } else if game.mode == 2 {
        "PAUSED"
    } else {
        "ARKANOO"
    };
    text(
        heading,
        320.0 - heading.len() as f32 * 6.0,
        150.0,
        2.0,
        0x68e5ffff,
    );
    if game.menu.settings {
        for action in [10, 7, 8] {
            draw_menu_button(game, action);
        }
        gamepkg::draw_rect(270.0, 240.0, 100.0, 10.0, 0x505050c8);
        gamepkg::draw_rect(
            270.0,
            240.0,
            (game.menu.sfx_volume.min(128) * 200 / 128) as f32 * 0.5,
            10.0,
            0x64c864dc,
        );
        gamepkg::draw_rect(270.0, 240.0, 100.0, 0.5, 0xc8c8c8ff);
        gamepkg::draw_rect(270.0, 249.5, 100.0, 0.5, 0xc8c8c8ff);
        gamepkg::draw_rect(270.0, 240.0, 0.5, 10.0, 0xc8c8c8ff);
        gamepkg::draw_rect(369.5, 240.0, 0.5, 10.0, 0xc8c8c8ff);
        let percent = game.menu.sfx_volume.min(128) * 100 / 128;
        let mut shown = String::new();
        if percent == 100 {
            shown.push_str("100");
        } else {
            if percent >= 10 {
                shown.push(char::from(48 + (percent / 10) as u8));
            }
            shown.push(char::from(48 + (percent % 10) as u8));
        }
        shown.push('%');
        text(
            &shown,
            320.0 - shown.len() as f32 * 3.0,
            241.5,
            1.0,
            0xffffffff,
        );
        if !game.editor.readable {
            text("SAVE DAMAGED / PRESERVED", 251.0, 330.0, 1.0, 0xffdb70ff);
        } else if game.menu.save_failed {
            text("SAVING UNAVAILABLE", 266.0, 330.0, 1.0, 0xffdb70ff);
        }
    } else {
        for action in 1..=6 {
            draw_menu_button(game, action);
        }
    }
    if game.menu.settings {
        text("F11 ALSO TOGGLES FULLSCREEN", 242.0, 300.0, 1.0, 0x8ba2c9ff);
    } else {
        text(
            "UP START / DOWN PAUSE / E EDITOR",
            227.0,
            355.0,
            1.0,
            0x8ba2c9ff,
        );
        text(
            "V GRAVITY / R RESTART / Q QUIT",
            233.0,
            375.0,
            1.0,
            0x8ba2c9ff,
        );
    }
}

fn draw_menu_button(game: &Game, action: u32) {
    let [left, top, width, height] = crate::menu::Menu::bounds(action);
    let label = match action {
        1 => {
            if game.started {
                "RESUME"
            } else {
                "NEW GAME"
            }
        }
        2 => "RESTART",
        3 => "GRAVITY MODE",
        4 => "LEVEL EDITOR",
        5 => "SETTINGS",
        6 => "QUIT",
        7 => "FULLSCREEN",
        10 => {
            if game.menu.sfx_muted {
                "SFX OFF"
            } else {
                "SFX ON"
            }
        }
        _ => "BACK",
    };
    let hovered = game.menu.hover == action;
    gamepkg::draw_rect(
        left,
        top,
        width,
        height,
        if hovered { 0x646496c8 } else { 0x3c3c64b4 },
    );
    gamepkg::draw_rect(left, top, width, 0.5, 0xc8c8c8ff);
    gamepkg::draw_rect(left, top + height - 0.5, width, 0.5, 0xc8c8c8ff);
    gamepkg::draw_rect(left, top, 0.5, height, 0xc8c8c8ff);
    gamepkg::draw_rect(left + width - 0.5, top, 0.5, height, 0xc8c8c8ff);
    if action == 3 && game.gravity {
        gamepkg::draw_rect(left + 2.0, top + 2.0, 2.0, height - 4.0, 0x64c864ff);
    }
    text(
        label,
        left + (width - label.len() as f32 * 7.5) * 0.5,
        top + 5.5,
        1.25,
        0xffffffff,
    );
}
