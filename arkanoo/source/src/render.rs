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

fn brick_mask_line(mask: &mut [u8], size: [i32; 2], points: [i32; 4], opacity: u8) {
    let [mut x, mut y, end_x, end_y] = points;
    let dx = (end_x - x).abs();
    let dy = -(end_y - y).abs();
    let sx = if x < end_x { 1 } else { -1 };
    let sy = if y < end_y { 1 } else { -1 };
    let mut error = dx + dy;
    loop {
        if x >= 0 && x < size[0] && y >= 0 && y < size[1] {
            let offset = (y * size[0] + x) as usize;
            let previous = u32::from(mask[offset]);
            mask[offset] =
                (u32::from(opacity) + (previous * (255 - u32::from(opacity))) / 255) as u8;
        }
        if x == end_x && y == end_y {
            break;
        }
        let twice = error * 2;
        if twice >= dy {
            error += dy;
            x += sx;
        }
        if twice <= dx {
            error += dx;
            y += sy;
        }
    }
}
fn brick_mask_pixels(mask: &[u8]) -> Vec<u8> {
    let mut pixels = Vec::new();
    for opacity in mask {
        pixels.extend_from_slice(&[255, 255, 255, *opacity]);
    }
    pixels
}
pub fn draw_ice_brick(health: u8, max_health: u8, frame: u64, x: f32, y: f32) -> bool {
    // Original integer world rectangles, projected at half scale.
    let bx = (x * 2.0) as i32;
    let by = ((y - 60.0) * 2.0) as i32;
    let x = bx as f32 * 0.5;
    let y = 60.0 + by as f32 * 0.5;
    let pulse = (frame as f32 * 0.05).sin() * 20.0;
    let opacity = (180.0 + pulse) as u8;
    let base = gamepkg::draw_rect(x, y, 30.0, 10.0, 0xc8f0ff00 | u32::from(opacity));
    // Glints extend through the original inclusive right/bottom endpoints.
    let mut overlay = Vec::new();
    overlay.resize(61 * 21, 0u8);
    brick_mask_line(&mut overlay, [61, 21], [0, 0, 60, 0], 150);
    brick_mask_line(&mut overlay, [61, 21], [0, 0, 0, 20], 150);
    let mut rng = crate::pattern_rng::Rng::new((bx * by) as u64);
    for _ in 0..3 {
        let x1 = rng.range_i32(5, 55);
        let y1 = rng.range_i32(5, 15);
        let x2 = rng.range_i32(5, 55);
        let y2 = rng.range_i32(5, 15);
        brick_mask_line(&mut overlay, [61, 21], [x1, y1, x2, y2], 80);
    }
    if health < max_health {
        brick_mask_line(&mut overlay, [61, 21], [30, 10, 5, 5], 220);
        brick_mask_line(&mut overlay, [61, 21], [30, 10, 55, 12], 220);
        brick_mask_line(&mut overlay, [61, 21], [30, 10, 10, 18], 220);
        brick_mask_line(&mut overlay, [61, 21], [5, 5, 0, 15], 220);
        brick_mask_line(&mut overlay, [61, 21], [55, 12, 60, 5], 220);
    }
    let pixels = brick_mask_pixels(&overlay);
    let glints = gamepkg::draw_image(x, y, 30.5, 10.5, 61, 21, &pixels);
    let mut shine = Vec::new();
    shine.resize(60 * 20, 0u8);
    let offset = (frame as f32 * 2.0) as i32 % 120 - 20;
    for i in 0..10 {
        brick_mask_line(
            &mut shine,
            [60, 20],
            [offset + i, 20, offset + 20 + i, 0],
            150,
        );
    }
    let pixels = brick_mask_pixels(&shine);
    let band = gamepkg::draw_image(x, y, 30.0, 10.0, 60, 20, &pixels);
    base && glints && band
}

pub fn draw_fire_brick(frame: u64, x: f32, y: f32) -> bool {
    let bx = (x * 2.0) as i32;
    let by = ((y - 60.0) * 2.0) as i32;
    let x = bx as f32 * 0.5;
    let y = 60.0 + by as f32 * 0.5;
    let time = frame as f64 * 0.05;
    let red = 220u8.saturating_add((time.sin() * 20.0) as u8);
    let green = 80u8.saturating_add((time.cos() * 30.0) as u8);
    let color = (u32::from(red) << 24) | (u32::from(green) << 16) | 255;
    let base = gamepkg::draw_rect(x, y, 30.0, 10.0, color);
    // rust-sdl2 0.37 Rect::new clamps the original zero height to one pixel.
    let blob_offset_y = ((time * 1.5).sin() * 5.0) as i32;
    let blob = gamepkg::draw_rect(
        x + 5.0,
        y + 5.0 + blob_offset_y as f32 * 0.5,
        20.0,
        0.5,
        0xff9632ff,
    );
    let mut overlay = Vec::new();
    overlay.resize(61 * 21, 0u8);
    for row in 0..10 {
        for col in 0..60 {
            overlay[row * 61 + col] = 40;
        }
    }
    brick_mask_line(&mut overlay, [61, 21], [0, 0, 60, 0], 180);
    brick_mask_line(&mut overlay, [61, 21], [0, 0, 0, 20], 180);
    brick_mask_line(&mut overlay, [61, 21], [0, 20, 20, 0], 80);
    brick_mask_line(&mut overlay, [61, 21], [5, 20, 25, 0], 80);
    let pixels = brick_mask_pixels(&overlay);
    let glass = gamepkg::draw_image(x, y, 30.5, 10.5, 61, 21, &pixels);
    let mut shine = Vec::new();
    shine.resize(60 * 20, 0u8);
    let offset = (frame as f32 * 1.5) as i32 % 120 - 20;
    for i in 0..8 {
        brick_mask_line(
            &mut shine,
            [60, 20],
            [offset + i, 20, offset + 20 + i, 0],
            100,
        );
    }
    let pixels = brick_mask_pixels(&shine);
    let band = gamepkg::draw_image(x, y, 30.0, 10.0, 60, 20, &pixels);
    base && blob && glass && band
}

pub fn draw_bonus_item(kind: u32, x: f32, y: f32) -> bool {
    if kind >= 4 {
        return false;
    }
    let bodies = [
        "bonus-body-long.png",
        "bonus-body-ball.png",
        "bonus-body-ghost.png",
        "bonus-body-rocket.png",
    ];
    let symbols = [
        "bonus-symbol-long.png",
        "bonus-symbol-ball.png",
        "bonus-symbol-ghost.png",
        "bonus-symbol-rocket.png",
    ];
    let x = (x * 2.0) as i32 as f32 * 0.5;
    let y = 60.0 + ((y - 60.0) * 2.0) as i32 as f32 * 0.5;
    // Inclusive point endpoints extend one pixel beyond the 40x40 collision box.
    let body = gamepkg::draw_asset_image(bodies[kind as usize], x, y, 20.5, 20.5);
    let highlight = gamepkg::draw_asset_image("bonus-highlight.png", x+2.0, y+2.0, 10.5, 10.5);
    let outline = gamepkg::draw_asset_image("bonus-rings.png", x, y, 20.5, 20.5);
    let symbol_rect=[[12.0f32,18.0,16.0,6.0],[15.0,15.0,12.0,12.0],[14.0,14.0,12.0,13.0],[17.0,14.0,6.0,13.0]][kind as usize];
    let symbol = gamepkg::draw_asset_image(symbols[kind as usize],x+symbol_rect[0]*0.5,y+symbol_rect[1]*0.5,symbol_rect[2]*0.5,symbol_rect[3]*0.5);
    body && highlight && outline && symbol
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
    if kind == 1 {
        draw_ice_brick(health, max_health, frame, x, y);
        return;
    }
    if kind == 2 {
        draw_fire_brick(frame, x, y);
        return;
    }
    if kind == 3 {
        draw_metal_brick(x, y);
        return;
    }
    let rgba = [30, 35, 40, 255];
    let mut pixels = Vec::new();
    pixels.resize(60 * 20 * 4, 0u8);
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&rgba);
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
    if !draw_background(game.level, game.frame) {
    for n in 0..11 {
        gamepkg::draw_rect(0.0, 60.0 + n as f32 * 32.0, 640.0, 1.0, 0x17233aff);
    }
    gamepkg::draw_rect(0.0, 60.0, 1.0, 360.0, 0x345476ff);
    gamepkg::draw_rect(639.0, 60.0, 1.0, 360.0, 0x345476ff);
    }
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
    gamepkg::draw_clip(0.0, 60.0, 640.0, 360.0);
    for index in 0..200 {
        if game.blocks[index] > 0 {
            let x = game.block_x[index];
            let y = game.block_y[index];
            if game.block_type[index] == 0 {
                draw_normal_brick(game.block_color[index], x, y);
            } else {
                special_brick(game, index, x, y);
            }
        }
    }
    draw_paddle(game);
    for ball in &game.balls {
        draw_ball_effects(ball,game.frame);
        draw_ball(ball);
        draw_ball_speed(ball);
    }
    for drop in &game.drops {
        draw_bonus_item(drop.kind, drop.x, drop.y);
    }
    for (x, y) in &game.rockets {
        draw_rocket(*x, *y);
    }
    draw_particles(&game.particles);
    if let Some(penguin)=&game.penguin {draw_penguin(penguin);}
    if game.portal {
        draw_portal(game.frame, u64::from(game.portal_timer));
    }
    draw_hearts(game);
    gamepkg::draw_clip_reset();
    if game.gravity {
        text("GRAVITY", 266.0, 452.0, 1.0, 0xffcd60ff);
    }

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
pub fn draw_rocket(x: f32, y: f32) -> bool {
    let original_x = (x * 2.0) as i32;
    let original_y = ((y - 60.0) * 2.0) as i32;
    gamepkg::draw_asset_image(
        "rocket.png",
        original_x as f32 * 0.5,
        original_y as f32 * 0.5 + 60.0,
        5.0,
        10.0,
    )
}
pub fn draw_ball(ball: &crate::physics::Ball) -> bool {
    let x = ((ball.x - 3.0) * 2.0) as i32;
    let y = ((ball.y - 60.0 - 3.0) * 2.0) as i32;
    if gamepkg::draw_asset_image_linear("ball.png", x as f32 * 0.5, 60.0 + y as f32 * 0.5, 6.0, 6.0) {
        true
    } else {
        gamepkg::draw_circle(ball.x, ball.y, 3.0, 0xffffffff);
        false
    }
}
pub fn draw_ball_speed(ball: &crate::physics::Ball) -> bool {
    let vx=ball.vx*2.0;let vy=ball.vy*2.0;
    let speed=((vx*vx+vy*vy).sqrt()*60.0) as i32;
    let label=crate::speed_font::raster(speed as u32);
    let width=(label.width as f32*0.6) as u32;
    let height=(31.0f32*0.6) as u32;
    let cx=((ball.x-3.0)*2.0) as i32+6;
    let cy=((ball.y-63.0)*2.0) as i32+6;
    gamepkg::draw_image_linear((cx-width as i32/2) as f32*0.5,60.0+(cy-6-height as i32-5) as f32*0.5,width as f32*0.5,height as f32*0.5,label.width,31,&label.pixels)
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
pub fn draw_image_background(level:u32,darkness:u8)->bool {
    let paths=match level {
        1 => ["background1-0.png","background1-1.png","background1-2.png","background1-3.png","background1-4.png","background1-5.png"],
        2 => ["background2-0.png","background2-1.png","background2-2.png","background2-3.png","background2-4.png","background2-5.png"],
        3 => ["background3-0.png","background3-1.png","background3-2.png","background3-3.png","background3-4.png","background3-5.png"],
        4 => ["background4-0.png","background4-1.png","background4-2.png","background4-3.png","background4-4.png","background4-5.png"],
        5 => ["background5-0.png","background5-1.png","background5-2.png","background5-3.png","background5-4.png","background5-5.png"],
        6 => ["background6-0.png","background6-1.png","background6-2.png","background6-3.png","background6-4.png","background6-5.png"],
        _ => return false,
    };
    let mut accepted=true;
    for index in 0..6 {
        let col=index%3;let row=index/3;
        let width=if col==2 {128.0} else {256.0};
        let height=if row==1 {104.0} else {256.0};
        if !gamepkg::draw_asset_image(paths[index],col as f32*256.0,60.0+row as f32*256.0,width,height){accepted=false;}
    }
    if darkness>0 && !gamepkg::draw_rect(0.0,60.0,640.0,360.0,u32::from(darkness)){accepted=false;}
    accepted
}
pub fn draw_background(level: u32, frame: u64) -> bool {
    if level>=1 && level<=6 {return draw_image_background(level,191);}
    if level > 9 {
        let theme = crate::procedural::procedural_theme(level);
        if [2,8,10,12,15].contains(&theme) {
            let (color, commands) = match theme {
                2 => crate::procedural::hex_commands(level, frame),
                8 => crate::procedural::night_commands(level, frame),
                10 => crate::procedural::bubble_commands(level, frame),
                12 => crate::procedural::circuit_commands(level, frame),
                _ => crate::procedural::maze_commands(level, frame),
            };
            gamepkg::draw_rect(0.0, 60.0, 640.0, 360.0, color);
            if let Some(image) = crate::background::composited_commands(&commands, color) {
                draw_effect_tiles(&image, 128);
            }
            return true;
        }
        if [4,5,9].contains(&theme) {
            let (color, rectangles) = match theme {
                4 => crate::procedural::matrix_rectangles(level, frame),
                5 => crate::procedural::nebula_rectangles(level, frame),
                _ => crate::procedural::smoke_rectangles(level, frame),
            };
            gamepkg::draw_rect(0.0, 60.0, 640.0, 360.0, color);
            if let Some(image) = crate::background::composited_rectangles(&rectangles, color) {
                draw_effect_tiles(&image, 128);
            }
            return true;
        }
    }
    let background = if level <= 9 { crate::background::campaign(level, frame) }
        else { crate::procedural::procedural_background(level, frame) };
    if let Some(scene) = background {
        gamepkg::draw_rect(0.0, 60.0, 640.0, 360.0, scene.color);
        let theme = if level > 9 { crate::procedural::procedural_theme(level) } else { u32::MAX };
        let image = crate::background::composited_background(&scene);
        if let Some(image) = image {
            let tile = if theme == 6 || (level > 9 && scene.points.len() > 10000) { 128 } else { 64 };
            draw_effect_tiles(&image, tile);
        }
        true
    } else { false }
}
pub fn draw_portal(frame: u64, timer: u64) {
    let dots = crate::physics::portal_points(frame, timer);
    if let Some(image) = crate::physics::effect_image(&dots) { draw_effect_layer(&image); }
}
pub fn draw_ball_effects(ball: &crate::physics::Ball,frame:u64) {
    let dots=crate::physics::ball_effect_points(ball,frame);
    if let Some(image)=crate::physics::effect_image(&dots) {draw_effect_layer(&image);}
}
fn draw_effect_layer(image: &crate::physics::ParticleImage) { draw_effect_tiles(image, 512); }
fn draw_effect_tiles(image: &crate::physics::ParticleImage, tile: u32) {

        // Non-overlapping tiles retain every visible pixel; sparse backgrounds use smaller tiles.
        for top in (0..image.height).step_by(tile as usize) {
            for left in (0..image.width).step_by(tile as usize) {
                let width = (image.width - left).min(tile);
                let height = (image.height - top).min(tile);
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

pub fn draw_dynamic_label(label:&str,original_x:i32,original_y:i32,color:[u8;3],centered:bool,row_height:i32)->bool {
    draw_viewport_label(label,original_x,original_y,color,centered,row_height,720)
}
fn draw_viewport_label(label:&str,original_x:i32,original_y:i32,color:[u8;3],centered:bool,row_height:i32,bottom:i32)->bool {
    let Some(layout_data)=crate::dynamic_font::unicode_layout(label) else {return false;};
    let left=if centered {original_x-layout_data.width/2} else {original_x};
    let top=if row_height>=0 {original_y+(row_height-layout_data.height)/2} else {original_y};
    let visible_left=(-left).clamp(0,layout_data.width);let visible_right=(1280-left).clamp(0,layout_data.width);
    let clip_top=top.max(0);let clip_bottom=(top+layout_data.height).min(bottom);
    if visible_left>=visible_right || clip_top>=clip_bottom {return true;}
    let mut column=visible_left;let mut accepted=true;
    while column<visible_right {
        let core_right=(column+256).min(visible_right);let source_left=column.saturating_sub(1).max(0);let source_right=(core_right+1).min(layout_data.width);let tile_width=source_right-source_left;
        let pixels=crate::dynamic_font::unicode_raster_slice(&layout_data,color,source_left,tile_width);
        if !gamepkg::draw_clip((left+column) as f32*0.5,60.0+clip_top as f32*0.5,(core_right-column) as f32*0.5,(clip_bottom-clip_top) as f32*0.5){accepted=false;}
        if !gamepkg::draw_image_linear((left+source_left) as f32*0.5,60.0+top as f32*0.5,tile_width as f32*0.5,layout_data.height as f32*0.5,tile_width as u32,layout_data.height as u32,&pixels){accepted=false;}
        if !gamepkg::draw_clip_reset(){accepted=false;}
        column=core_right;
    }
    accepted
}
pub fn draw_editor_grid()->bool {
    // Original SDL grid disables blending and includes both line endpoints.
    let grid_pixel=[80u8,80,100,255];let mut accepted=true;
    for row in 0..=10 {
        if !gamepkg::draw_image(20.0,100.0+row as f32*10.0,600.5,0.5,1,1,&grid_pixel){accepted=false;}
    }
    for col in 0..=20 {
        if !gamepkg::draw_image(20.0+col as f32*30.0,100.0,0.5,100.5,1,1,&grid_pixel){accepted=false;}
    }
    accepted
}
pub fn draw_editor_palette(selected:u8,hovered:u8)->bool {
    let palette_colors=[[255u8,0,0,255],[255,165,0,255],[255,255,0,255],[0,255,0,255],[0,0,255,255],[138,43,226,255],[200,240,255,255],[255,100,0,255],[30,35,40,255]];
    let mut accepted=true;
    for index in 0..9 {
        let px=860+index as i32*45;
        let border_shade=if selected==index as u8 {255} else if hovered==index as u8 {150} else {100};
        let border_pixel=[border_shade,border_shade,border_shade,255];
        let expansion=if selected==index as u8 {2} else {0};
        if !gamepkg::draw_image((px-expansion) as f32*0.5,60.0+(20-expansion) as f32*0.5,(40+expansion*2) as f32*0.5,(30+expansion*2) as f32*0.5,1,1,&border_pixel){accepted=false;}
        if !gamepkg::draw_image((px+1) as f32*0.5,70.5,19.0,14.0,1,1,&palette_colors[index]){accepted=false;}
        let label=crate::speed_font::palette_number(index as u8);
        if !gamepkg::draw_image_linear((px+12) as f32*0.5,70.0,7.5,15.5,15,31,&label.pixels){accepted=false;}
    }
    accepted
}
pub fn draw_editor_label(which:u32,original_x:i32,original_y:i32)->bool {
    let lettering=crate::editor_font::editor_raster(which);
    let full_width=lettering.width as usize;let mut accepted=true;let mut column=0;
    while column<full_width {
        let core_right=(column+256).min(full_width);let source_left=column.saturating_sub(1);let source_right=(core_right+1).min(full_width);let tile_width=source_right-source_left;
        let mut tile_pixels=Vec::new();tile_pixels.reserve(tile_width*31*4);
        for row in 0..31 {tile_pixels.extend_from_slice(&lettering.pixels[(row*full_width+source_left)*4..(row*full_width+source_right)*4]);}
        if !gamepkg::draw_clip((original_x as f32+column as f32)*0.5,60.0+original_y as f32*0.5,(core_right-column) as f32*0.5,15.5) {accepted=false;}
        if !gamepkg::draw_image_linear((original_x as f32+source_left as f32)*0.5,60.0+original_y as f32*0.5,tile_width as f32*0.5,15.5,tile_width as u32,31,&tile_pixels){accepted=false;}
        if !gamepkg::draw_clip_reset(){accepted=false;}
        column=core_right;
    }
    accepted
}
pub fn draw_editor_button(which:u32,hovered:bool)->bool {
    let box_rect=crate::editor::Editor::toolbar_bounds(which);
    let x=box_rect[0] as f32*0.5;let y=60.0+box_rect[1] as f32*0.5;let w=box_rect[2] as f32*0.5;let h=box_rect[3] as f32*0.5;
    let fill=if hovered {[100u8,100,150,200]} else {[60u8,60,100,180]};let border=[200u8,200,200,255];let mut accepted=true;
    if !gamepkg::draw_image(x,y,w,h,1,1,&fill){accepted=false;}
    for edge in [[x,y,w,0.5],[x,y+h-0.5,w,0.5],[x,y,0.5,h],[x+w-0.5,y,0.5,h]] {if !gamepkg::draw_image(edge[0],edge[1],edge[2],edge[3],1,1,&border){accepted=false;}}
    let which_label=which+3;let lettering=crate::editor_font::editor_raster(which_label);
    if !draw_editor_label(which_label,box_rect[0]+(box_rect[2]-lettering.width as i32)/2,box_rect[1]+(box_rect[3]-31)/2){accepted=false;}
    accepted
}
pub fn draw_pattern_browser(edit:&crate::editor::Editor) {
    gamepkg::draw_image(0.0,60.0,640.0,360.0,1,1,&[0,0,0,200]);
    gamepkg::draw_image(170.0,115.0,300.0,250.0,1,1,&[40,45,52,255]);
    for edge in [[170.0,115.0,300.0,0.5],[170.0,364.5,300.0,0.5],[170.0,115.0,0.5,250.0],[469.5,115.0,0.5,250.0]] {gamepkg::draw_image(edge[0],edge[1],edge[2],edge[3],1,1,&[100,110,120,255]);}
    draw_editor_label(16,565,125);
    let first=(edit.selection/10)*10;
    for (index,pattern) in edit.patterns.iter().enumerate().skip(first).take(10) {
        let row_y=145.0+(index-first) as f32*17.5;
        let row_color=if index==edit.selection {[70u8,130,180,255]} else {[55u8,60,68,255]};
        gamepkg::draw_image(180.0,row_y,280.0,15.0,1,1,&row_color);
        draw_dynamic_label(&pattern.name,370,170+(index-first) as i32*35,[255,255,255],false,30);
    }
    draw_editor_label(17,432,570);
}
fn draw_editor(game: &Game) {
    let edit = &game.editor;
    gamepkg::draw_clear(0x0b1020ff);
    if !draw_image_background(edit.background,153) {
        gamepkg::draw_rect(0.0,60.0,640.0,360.0,0x000000ff);
    }
    draw_editor_grid();
    for index in 0..200 {
        let cell = edit.cells[index];
        let x = 20.0 + (index % 20) as f32 * 30.0;
        let y = 100.0 + (index / 20) as f32 * 10.0;
        if cell < 6 {
            draw_normal_brick(cell, x, y);
        }
        if (6..=8).contains(&cell) {
            brick_picture(
                cell - 5,
                edit.health[index],
                edit.health[index],
                edit.frame_count,
                x,
                y,
            );
        }
    }
    draw_editor_label(0,10,10);
    draw_editor_label(1,760,25);
    let mut pattern_label=String::from("Name: ");pattern_label.push_str(&edit.name);
    pattern_label.push_str(if edit.naming {"_ (ESC to finish)"} else {" (N to edit)"});
    draw_dynamic_label(&pattern_label,10,40,if edit.naming {[100,255,100]} else {[200,200,200]},false,-1);
    draw_editor_palette(edit.selected,edit.hovered_color);
    for which in 0..7 {draw_editor_button(which,edit.hovered_button==which as u8);}
    gamepkg::draw_rect(20.0, 425.0, 75.0, 20.0, 0x284260ff);
    text("IMPORT I", 25.0, 432.0, 1.0, 0xffffffff);
    gamepkg::draw_rect(105.0, 425.0, 75.0, 20.0, 0x284260ff);
    text("EXPORT X", 110.0, 432.0, 1.0, 0xffffffff);
    if edit.browser {draw_pattern_browser(edit);}
    draw_editor_label(2,296,620);
    draw_editor_label(9+edit.background.clamp(1,6),1084,570);
    if edit.message_time>0 {draw_dynamic_label(&edit.message,640,590,[100,255,100],true,-1);}
}

pub fn draw_menu_background()->bool {
    let paths=["menu-background1-0.png","menu-background1-1.png","menu-background1-2.png","menu-background1-3.png","menu-background1-4.png","menu-background1-5.png"];let mut accepted=true;
    for index in 0..6 {
        let core_x=(index%3)*510usize;let core_y=(index/3)*510usize;let core_w=(1280-core_x).min(510);let core_h=(720-core_y).min(510);let source_left=core_x.saturating_sub(1);let source_top=core_y.saturating_sub(1);let source_right=(core_x+core_w+1).min(1280);let source_bottom=(core_y+core_h+1).min(720);
        if !gamepkg::draw_clip(core_x as f32*0.5,60.0+core_y as f32*0.5,core_w as f32*0.5,core_h as f32*0.5){accepted=false;}
        if !gamepkg::draw_asset_image_linear(paths[index],source_left as f32*0.5,60.0+source_top as f32*0.5,(source_right-source_left) as f32*0.5,(source_bottom-source_top) as f32*0.5){accepted=false;}
        if !gamepkg::draw_clip_reset(){accepted=false;}
    }
    if !gamepkg::draw_rect(0.0,60.0,640.0,360.0,150){accepted=false;}accepted
}
pub fn draw_menu(game:&Game) {
    gamepkg::draw_clear(0x101b35ff);
    draw_menu_background();
    let heading=if game.menu.settings {"SETTINGS"} else if game.started {"PAUSED"} else {"ARKANOO"};
    draw_dynamic_label(heading,640,if game.menu.settings {160} else {180},[255,255,255],true,-1);
    if game.menu.settings {
        for action in [10,7,8] {draw_menu_button(game,action);}
        draw_sfx_slider(game.menu.sfx_volume);
        if !game.editor.readable {text("SAVE DAMAGED / PRESERVED",251.0,330.0,1.0,0xffdb70ff);} else if game.menu.save_failed {text("SAVING UNAVAILABLE",266.0,330.0,1.0,0xffdb70ff);}
    } else {
        for action in 1..=6 {draw_menu_button(game,action);}
    }
}
pub fn draw_sfx_slider(volume:u32) {
    let width=volume.min(128)*200/128;
    gamepkg::draw_image(270.0,240.0,100.0,10.0,1,1,&[80,80,80,200]);
    gamepkg::draw_image(270.0,240.0,width as f32*0.5,10.0,1,1,&[100,200,100,220]);
    for edge in [[270.0,240.0,100.0,0.5],[270.0,249.5,100.0,0.5],[270.0,240.0,0.5,10.0],[369.5,240.0,0.5,10.0]] {gamepkg::draw_image(edge[0],edge[1],edge[2],edge[3],1,1,&[200,200,200,255]);}
    let percent=volume.min(128)*100/128;let mut shown=String::new();
    if percent==100 {shown.push_str("100");} else {if percent>=10 {shown.push(char::from(48+(percent/10) as u8));}shown.push(char::from(48+(percent%10) as u8));}shown.push('%');
    if let Some(volume_label)=crate::dynamic_font::unicode_layout(&shown) {draw_dynamic_label(&shown,540+(200-volume_label.width)/2,360,[255,255,255],false,20);}
}
pub fn draw_menu_button(game:&Game,action:u32) {
    let [left,top,width,height]=crate::menu::Menu::bounds(action);
    let caption=match action {1=>if game.started {"Resume"} else {"New Game"},2=>"Restart",3=>"Gravity Mode",4=>"Level Editor",5=>"Settings",6=>"Quit",7=>if game.menu.fullscreen {"Fullscreen"} else {"Windowed"},10=>if game.menu.sfx_muted {"SFX: OFF"} else {"SFX: ON"},_=>"Back"};
    let fill=if game.menu.hover==action {[100u8,100,150,200]} else {[60u8,60,100,180]};
    gamepkg::draw_image(left,top,width,height,1,1,&fill);
    for edge in [[left,top,width,0.5],[left,top+height-0.5,width,0.5],[left,top,0.5,height],[left+width-0.5,top,0.5,height]] {gamepkg::draw_image(edge[0],edge[1],edge[2],edge[3],1,1,&[200,200,200,255]);}
    if let Some(caption_layout)=crate::dynamic_font::unicode_layout(caption) {draw_viewport_label(caption,(left*2.0) as i32+((width*2.0) as i32-caption_layout.width)/2,((top-60.0)*2.0) as i32,[255,255,255],false,(height*2.0) as i32,840);}
}

// Body pixels come from the unchanged original SDL procedure. Overlay rectangles
// preserve the original integer geometry before mapping the world to half size.
pub fn draw_paddle(game: &Game) -> bool {
    let x = (game.paddle * 2.0) as i32;
    let w = (game.width * 2.0) as i32;
    let left = x as f32 * 0.5;
    let path = if w > 140 {
        "paddle-long.png"
    } else {
        "paddle.png"
    };
    let loaded = gamepkg::draw_asset_image_linear(path, left, PADDLE_Y, w as f32 * 0.5, 11.0);
    if !loaded {
        gamepkg::draw_rect(left, PADDLE_Y, w as f32 * 0.5, 11.0, 0x283246ff);
    }
    let spin = game.paddle_glow;
    if spin > 0.1 {
        let glow_alpha = (spin * 255.0) as u32;
        gamepkg::draw_rect(
            left + 6.0,
            PADDLE_Y + 4.0,
            (w - 24) as f32 * 0.5,
            2.0,
            0xc864ff00 | glow_alpha,
        );
        gamepkg::draw_rect(
            left + 6.0,
            PADDLE_Y + 4.5,
            (w - 24) as f32 * 0.5,
            1.0,
            0xffffff00 | glow_alpha,
        );
    } else {
        gamepkg::draw_rect(
            left + 6.0,
            PADDLE_Y + 4.0,
            (w - 24) as f32 * 0.5,
            2.0,
            0x00c8ffc8,
        );
    }
    let light = if spin > 0.1 { 0xff32ffff } else { 0xff6432c8 };
    gamepkg::draw_rect(left + 1.0, PADDLE_Y + 2.0, 2.0, 6.0, light);
    gamepkg::draw_rect(left + (w - 6) as f32 * 0.5, PADDLE_Y + 2.0, 2.0, 6.0, light);
    if spin > 0.2 {
        let color = 0xc864ff00 | (spin * 100.0) as u32;
        // SDL draw_rect includes both outer endpoints of its integer rectangle.
        gamepkg::draw_rect(left - 1.0, PADDLE_Y - 1.0, (w + 4) as f32 * 0.5, 0.5, color);
        gamepkg::draw_rect(
            left - 1.0,
            PADDLE_Y + 10.5,
            (w + 4) as f32 * 0.5,
            0.5,
            color,
        );
        gamepkg::draw_rect(left - 1.0, PADDLE_Y - 0.5, 0.5, 11.0, color);
        gamepkg::draw_rect(
            left + (w + 1) as f32 * 0.5,
            PADDLE_Y - 0.5,
            0.5,
            11.0,
            color,
        );
    }
    loaded
}

// The original caches six 60x20 textures, then copies integer rectangles.
pub fn draw_normal_brick(color_index: u8, x: f32, y: f32) -> bool {
    let filenames = [
        "brick-red.png",
        "brick-orange.png",
        "brick-yellow.png",
        "brick-green.png",
        "brick-blue.png",
        "brick-violet.png",
    ];
    let selected = usize::from(color_index % 6);
    let left = (x * 2.0) as i32 as f32 * 0.5;
    let top = 60.0 + ((y - 60.0) * 2.0) as i32 as f32 * 0.5;
    let loaded =
        gamepkg::draw_asset_image_linear(filenames[selected], left, top, BLOCK_WIDTH, BLOCK_HEIGHT);
    if !loaded {
        let colors = [
            0xff0000ff, 0xffa500ff, 0xffff00ff, 0x00ff00ff, 0x0000ffff, 0x8a2be2ff,
        ];
        gamepkg::draw_rect(left, top, BLOCK_WIDTH, BLOCK_HEIGHT, colors[selected]);
    }
    loaded
}

// Nearest sampling retains the original primitive pixel grid. The outside
// stripes and bevel stay separate so their alpha and draw order are preserved.
pub fn draw_metal_brick(x: f32, y: f32) -> bool {
    let left = (x * 2.0) as i32 as f32 * 0.5;
    let top = 60.0 + ((y - 60.0) * 2.0) as i32 as f32 * 0.5;
    let body = gamepkg::draw_asset_image("metal-body.png", left, top, 30.0, 10.0);
    let stripes = gamepkg::draw_asset_image("metal-stripes.png", left - 10.0, top, 45.5, 10.5);
    let bevel = gamepkg::draw_asset_image("metal-bevel.png", left - 10.0, top, 45.5, 10.5);
    body && stripes && bevel
}
