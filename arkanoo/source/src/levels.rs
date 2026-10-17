// Pinned Arkanoo layout predicates. See CREDITS.md and LICENSE.
use crate::pattern_rng::Rng;
pub struct Recipe {
    pub kind: u64,
    value: f32,
    rule: u64,
}
impl Recipe {
    pub fn new(level: u32, attempt: u32) -> Self {
        let level = u64::from(level);
        let seed = level
            .wrapping_mul(54321)
            .wrapping_add((level % 7).wrapping_mul(11111))
            .wrapping_add((level / 5).wrapping_mul(99999))
            .wrapping_add(u64::from(attempt).wrapping_mul(77777));
        let mut rng = Rng::new(seed);
        let kind = rng.range(0, 12);
        let mut recipe = Self {
            kind,
            value: 0.0,
            rule: 0,
        };
        if kind == 0 {
            recipe.value = rng.float(0.6, 0.8);
        }
        if kind == 1 {
            recipe.value = rng.float(-0.3, 0.3);
        }
        if kind == 2 {
            recipe.rule = rng.range(2, 5);
        }
        if kind == 3 {
            recipe.value = rng.float(1.5, 3.0);
        }
        if kind == 4 {
            recipe.rule = rng.range(0, 3);
        }
        if kind == 5 {
            recipe.rule = rng.range(2, 6);
        }
        recipe
    }
    pub fn includes(&self, level: u32, row: usize, col: usize, attempt: u32) -> bool {
        let seed = u64::from(level) * 1000 + row as u64 * 100 + col as u64 + u64::from(attempt);
        match self.kind {
            0 => Rng::new(seed).unit() < self.value,
            1 => (col as f32 * 0.5 + row as f32 * 0.3).sin() > self.value,
            2 => ((row as u64 + col as u64) / self.rule).is_multiple_of(2),
            3 => {
                let dx = col as f32 - 10.0;
                let dy = row as f32 - 5.0;
                ((dx * dx + dy * dy).sqrt() / self.value) as i32 % 2 == 0
            }
            4 => (row as u64 + col as u64 + self.rule).is_multiple_of(2),
            5 => {
                let sum = (row % 3 + col % 3) as u64;
                sum == self.rule || sum == self.rule + 1
            }
            6 => row.is_multiple_of(2) == col.is_multiple_of(2),
            7 => {
                let mirror = if col < 10 { col } else { 19 - col };
                let seed =
                    u64::from(level) * 1000 + row as u64 * 100 + mirror as u64 + u64::from(attempt);
                Rng::new(seed).sixty_percent()
            }
            8 => (col ^ row).is_multiple_of(3) || (col & row).is_multiple_of(5),
            9 => {
                let cx = (col / 4) * 4 + 2;
                let cy = (row / 4) * 4 + 2;
                (col as i32 - cx as i32).abs() + (row as i32 - cy as i32).abs() <= 2
            }
            10 => match row % 5 {
                0 | 4 => col % 6 == 2 || col % 6 == 3,
                1 | 3 => col % 6 > 0 && col % 6 < 5,
                _ => col % 6 != 2 && col % 6 != 3,
            },
            _ => {
                let phase = row as f32 * 0.5;
                let first = phase.sin() * 5.0 + 10.0;
                let second = (phase + core::f32::consts::PI).sin() * 5.0 + 10.0;
                (col as f32 - first).abs() < 2.5 || (col as f32 - second).abs() < 2.5
            }
        }
    }
}
pub fn generate(level: u32) -> [u8; 200] {
    let mut attempt = 0;
    loop {
        let mut blocks = [0u8; 200];
        let mut count = 0;
        let recipe = Recipe::new(level, attempt);
        for row in 0..10 {
            for col in 0..20 {
                if (level <= 9 && campaign(level, row, col))
                    || (level > 9 && recipe.includes(level, row, col, attempt))
                {
                    blocks[row * 20 + col] = 1;
                    count += 1;
                }
            }
        }
        if level <= 9 || count >= 20 || attempt >= 10 {
            return blocks;
        }
        attempt += 1;
    }
}
// Upstream's nine handcrafted campaign layout predicates, scaled to our canvas.
pub fn campaign(level: u32, row: usize, col: usize) -> bool {
    match level {
        1 => true,
        2 => (row + col).is_multiple_of(2),
        3 => row.is_multiple_of(2),
        4 => col.is_multiple_of(2) || !col.is_multiple_of(2) && row == 0 || row == 9,
        5 => (col as i32 - 10).abs() <= row as i32,
        6 => (col as i32 - 10).abs() + (row as i32 - 5).abs() <= 7,
        7 => {
            let dx = col as f32 - 10.0;
            let dy = row as f32 - 5.0;
            (dy.atan2(dx) * 2.0 + (dx * dx + dy * dy).sqrt() * 0.5).sin() > 0.0
        }
        8 => {
            let dx = col as f32 - 10.0;
            let dy = row as f32 - 5.0;
            ((dx * dx + dy * dy).sqrt() as i32) % 3 != 1
        }
        9 => {
            !((col + row).is_multiple_of(7)
                || col % 4 == 1 && row % 4 == 1
                || col % 4 == 2 && row % 4 == 2)
        }
        _ => true,
    }
}
