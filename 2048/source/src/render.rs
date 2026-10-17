use super::gamepkg;
use super::model::Game;
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
        82 => 18879563326,
        83 => 32247333391,
        84 => 4433514655,
        85 => 15621211697,
        86 => 4648912433,
        87 => 11464787505,
        90 => 33831389247,
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
fn text(label: &str, x: f32, y: f32, scale: f32, color: u32) {
    letters(label.as_bytes(), x, y, scale, color);
}
fn number(value: u64, x: f32, y: f32, scale: f32, color: u32) {
    let mut digits = [48u8; 20];
    let mut value = value;
    let mut start = 19;
    digits[start] = 48 + (value % 10) as u8;
    value /= 10;
    while value > 0 {
        start -= 1;
        digits[start] = 48 + (value % 10) as u8;
        value /= 10;
    }
    letters(&digits[start..], x, y, scale, color);
}
pub fn draw(game: &Game, damaged: bool, save_failed: bool) {
    gamepkg::draw_clear(0xfaf8efff);
    text("2048", 20.0, 12.0, 3.0, 0x776e65ff);
    text("SCORE", 180.0, 8.0, 1.0, 0x776e65ff);
    number(game.position.score, 180.0, 24.0, 1.0, 0x776e65ff);
    text("BEST", 350.0, 8.0, 1.0, 0x776e65ff);
    number(game.best, 350.0, 24.0, 1.0, 0x776e65ff);
    text(
        "Z UNDO  ENTER NEW  C CONTINUE",
        20.0,
        451.0,
        1.0,
        0x776e65ff,
    );
    if save_failed {
        text("SAVE UNAVAILABLE", 430.0, 35.0, 1.0, 0xb03030ff);
    }
    gamepkg::draw_rect(125.0, 55.0, 390.0, 390.0, 0xbbada0ff);
    for index in 0..16 {
        let exponent = game.position.cells[index];
        let x = 133.0 + (index % 4) as f32 * 96.0;
        let y = 63.0 + (index / 4) as f32 * 96.0;
        let colors = [
            0xcdc1b4ffu32,
            0xeee4daff,
            0xede0c8ff,
            0xf2b179ff,
            0xf59563ff,
            0xf67c5fff,
            0xf65e3bff,
            0xedcf72ff,
            0xedcc61ff,
            0xedc850ff,
            0xedc53fff,
            0xedc22eff,
        ];
        gamepkg::draw_rect(x, y, 86.0, 86.0, colors[(exponent as usize).min(11)]);
        if exponent != 0 {
            let value = 1u64 << exponent;
            let mut digits = 1;
            let mut remaining = value / 10;
            while remaining > 0 {
                digits += 1;
                remaining /= 10;
            }
            let scale = (70.0 / (digits as f32 * 6.0)).min(3.0);
            number(
                value,
                x + (86.0 - digits as f32 * 6.0 * scale) / 2.0,
                y + (86.0 - 7.0 * scale) / 2.0,
                scale,
                if exponent <= 2 {
                    0x776e65ff
                } else {
                    0xf9f6f2ff
                },
            );
        }
    }
    if damaged || game.over() || (game.won() && !game.position.continued) {
        gamepkg::draw_rect(132.0, 213.0, 376.0, 72.0, 0xeee4daef);
        let label = if damaged {
            "SAVE DAMAGED"
        } else if game.over() {
            "GAME OVER"
        } else {
            "YOU WIN"
        };
        text(
            label,
            (640.0 - label.len() as f32 * 12.0) / 2.0,
            225.0,
            2.0,
            0x776e65ff,
        );
        let help = if damaged {
            "ENTER STARTS A NEW GAME"
        } else if game.over() {
            "Z UNDO OR ENTER NEW GAME"
        } else {
            "C CONTINUES  Z UNDO"
        };
        text(
            help,
            (640.0 - help.len() as f32 * 6.0) / 2.0,
            262.0,
            1.0,
            0x776e65ff,
        );
    }
}
