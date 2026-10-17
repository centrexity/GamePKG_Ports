use crate::gamepkg;
use crate::levels;
// Original 1280x720 world is uniformly scaled by 1/2 and letterboxed at y=60.
// Ball positions are centers; other positions are rectangle top-lefts.
pub const TOP: f32 = 60.0;
pub const BOTTOM: f32 = 420.0;
pub const PADDLE_Y: f32 = 395.0;
pub const BALL_RADIUS: f32 = 3.0;
pub const BLOCK_WIDTH: f32 = 30.0;
pub const BLOCK_HEIGHT: f32 = 10.0;

// SDL constructs integer rectangles in original coordinates before intersecting.
pub fn overlap(first: [f32; 4], second: [f32; 4]) -> (f32, f32) {
    let [x, y, width, height] = first;
    let [other_x, other_y, other_width, other_height] = second;
    let left = (x * 2.0) as i32;
    let top = ((y - TOP) * 2.0) as i32;
    let right = left + (width * 2.0) as i32;
    let bottom = top + (height * 2.0) as i32;
    let other_left = (other_x * 2.0) as i32;
    let other_top = ((other_y - TOP) * 2.0) as i32;
    let w = right.min(other_left + (other_width * 2.0) as i32) - left.max(other_left);
    let h = bottom.min(other_top + (other_height * 2.0) as i32) - top.max(other_top);
    if w > 0 && h > 0 {
        (w as f32 * 0.5, h as f32 * 0.5)
    } else {
        (0.0, 0.0)
    }
}
pub fn below(bound: u64) -> u64 {
    let threshold = bound.wrapping_neg() % bound;
    loop {
        let value = gamepkg::random_u64();
        if value >= threshold {
            return value % bound;
        }
    }
}
pub struct Ball {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub spin: f32,
    pub attached: bool,
    pub launch_timer: u32,
}
impl Ball {
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            spin: 0.0,
            attached: true,
            launch_timer: 30,
        }
    }
    pub fn launch(&mut self) {
        if self.attached {
            let direction = below(3);
            self.vx = if direction == 0 {
                -2.0
            } else if direction == 1 {
                0.0
            } else {
                2.0
            };
            self.vy = -2.0;
            self.attached = false;
        }
    }

    pub fn advance(&mut self, gravity: bool) {
        self.apply_forces(gravity);
        self.x += self.vx;
        self.y += self.vy;
        if self.x <= BALL_RADIUS {
            self.x = BALL_RADIUS;
            self.vx = self.vx.abs();
        } else if self.x >= 640.0 - BALL_RADIUS {
            self.x = 640.0 - BALL_RADIUS;
            self.vx = -self.vx.abs();
        }
        if self.y <= TOP + BALL_RADIUS {
            self.y = TOP + BALL_RADIUS;
            self.vy = self.vy.abs();
        }
        if self.vx.abs() < 1.0 {
            self.vx = if self.vx >= 0.0 { 1.0 } else { -1.0 };
        }
    }
    pub fn active(&self) -> bool {
        self.y < BOTTOM + BALL_RADIUS
    }
    pub fn intersects(&self, x: f32, y: f32, width: f32, height: f32) -> (f32, f32) {
        overlap(
            [
                self.x - BALL_RADIUS,
                self.y - BALL_RADIUS,
                BALL_RADIUS * 2.0,
                BALL_RADIUS * 2.0,
            ],
            [x, y, width, height],
        )
    }
    pub fn resolve_block(&mut self, x: f32, y: f32, ghost: bool) -> bool {
        let (width, height) = self.intersects(x, y, BLOCK_WIDTH, BLOCK_HEIGHT);
        if width == 0.0 {
            return false;
        }
        if !ghost {
            if width < height {
                if self.x < x + BLOCK_WIDTH / 2.0 {
                    self.x -= width;
                } else {
                    self.x += width;
                }
                self.vx = -self.vx;
            } else {
                if self.y < y + BLOCK_HEIGHT / 2.0 {
                    self.y -= height;
                } else {
                    self.y += height;
                }
                self.vy = -self.vy;
            }
        }
        true
    }
    // Velocities/spin use half of the original 1280-wide world's units.
    pub fn apply_forces(&mut self, gravity: bool) {
        if gravity {
            if self.vy > 0.0 && self.vy < 4.5 {
                self.vy = (self.vy + 0.15).min(4.5);
            }
            self.spin = 0.0;
        } else {
            self.vx += self.spin * 0.05;
            self.spin *= 0.98;
        }
    }
}
pub struct Drop {
    pub x: f32,
    pub y: f32,
    pub kind: u32,
}
pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: u32,
    pub color: u32,
}
pub struct Game {
    pub mode: u32,
    pub level: u32,
    pub unlocked: u32,
    pub score: u32,
    pub best: u32,
    pub lives: u32,
    pub paddle: f32,
    pub last_paddle: f32,
    pub width: f32,
    pub long: u32,
    pub ghost: u32,
    pub ammo: u32,
    pub balls: Vec<Ball>,
    pub drops: Vec<Drop>,
    pub rockets: Vec<(f32, f32)>,
    pub particles: Vec<Particle>,
    pub blocks: [u8; 200],
    pub block_type: [u8; 200],
    pub block_health: [u8; 200],
    pub block_x: [f32; 200],
    pub block_y: [f32; 200],
    pub gravity: bool,
    pub portal: bool,
    pub portal_timer: u32,
    pub max_speed: f32,
    pub frame: u32,
    pub lost_life: bool,
    pub cooldown: u32,
    pub sound: u32,
    pub dirty: bool,
}
impl Game {
    pub fn new() -> Self {
        let mut game = Self {
            mode: 0,
            level: 1,
            unlocked: 1,
            score: 0,
            best: 0,
            lives: 3,
            paddle: 285.0,
            last_paddle: 285.0,
            width: 70.0,
            long: 0,
            ghost: 0,
            ammo: 0,
            balls: Vec::new(),
            drops: Vec::new(),
            rockets: Vec::new(),
            particles: Vec::new(),
            blocks: [0; 200],
            block_type: [0; 200],
            block_health: [1; 200],
            block_x: [0.0; 200],
            block_y: [0.0; 200],
            gravity: false,
            portal: false,
            portal_timer: 0,
            max_speed: 0.0,
            frame: 0,
            lost_life: false,
            cooldown: 0,
            sound: 0,
            dirty: false,
        };
        game.populate();
        game.attach();
        game
    }
    pub fn populate(&mut self) {
        self.portal = false;
        self.portal_timer = 0;
        self.max_speed = 0.0;
        for index in 0..200 {
            self.block_type[index] = 0;
            self.block_health[index] = 1;
            self.block_x[index] = 20.0 + (index % 20) as f32 * 30.0;
            self.block_y[index] = 100.0 + (index / 20) as f32 * BLOCK_HEIGHT;
        }
        self.blocks = levels::generate(self.level);
    }
    pub fn attach(&mut self) {
        self.long = 0;
        self.ghost = 0;
        self.ammo = 0;
        self.width = 70.0;
        self.paddle = 285.0;
        self.last_paddle = self.paddle;
        self.lost_life = false;
        self.balls.clear();
        self.respawn();
        self.drops.clear();
        self.rockets.clear();
        self.particles.clear();
    }
    pub fn respawn(&mut self) {
        self.balls.push(Ball::new(
            self.paddle + self.width / 2.0,
            PADDLE_Y - BALL_RADIUS,
        ));
    }
    pub fn finish_level(&mut self) {
        if self.lost_life && self.lives < 3 {
            self.lives += 1;
        }
        self.mode = if self.level == 9 { 5 } else { 3 };
        self.unlocked = self.unlocked.max(self.level.saturating_add(1));
        self.dirty = true;
        self.sound = 3;
    }
    pub fn burst(&mut self, x: f32, y: f32) {
        for n in 0..8 {
            if self.particles.len() >= 64 {
                break;
            }
            let angle = n as f32 * core::f32::consts::FRAC_PI_4;
            self.particles.push(Particle {
                x,
                y,
                vx: angle.cos() * 2.0,
                vy: angle.sin() * 2.0,
                life: 22,
                color: 0xffcc70ff,
            });
        }
    }
    pub fn collide_balls(&mut self) {
        if self.portal {
            return;
        }
        // Snapshot contacts before responses, as the original does. Coordinates
        // are ball centers and distances/velocities are half original units.
        let mut contacts = Vec::new();
        for i in 0..self.balls.len() {
            for j in i + 1..self.balls.len() {
                let first = &self.balls[i];
                let second = &self.balls[j];
                if !first.active() || !second.active() {
                    continue;
                }
                let dx = second.x - first.x;
                let dy = second.y - first.y;
                if (dx * dx + dy * dy).sqrt() < 6.0 {
                    contacts.push((i, j, first.x + dx / 2.0, first.y + dy / 2.0));
                }
            }
        }
        for (i, j, x, y) in contacts {
            self.balls[i].vy = -self.balls[i].vy.abs().max(4.0);
            self.balls[i].vx = if self.balls[i].x < x { -2.5 } else { 2.5 };
            self.balls[j].vy = -self.balls[j].vy.abs().max(4.0);
            self.balls[j].vx = if self.balls[j].x < x { -2.5 } else { 2.5 };
            for k in 0..36 {
                let angle = (k as f32 * 10.0).to_radians();
                self.particles.push(Particle {
                    x,
                    y,
                    vx: angle.cos() * 3.0,
                    vy: angle.sin() * 3.0,
                    life: 30,
                    color: 0xc8ffffff,
                });
            }
            self.sound = 1;
        }
    }
    pub fn award(&mut self, points: u32) {
        self.score = self.score.saturating_add(points);
        // Persist only completed-level/lost-life checkpoints, never every frame.
        if self.score > self.best {
            self.best = self.score;
        }
    }
    pub fn set_block(&mut self, index: usize, kind: u8) -> bool {
        if index >= 200 || kind > 3 {
            return false;
        }
        self.blocks[index] = 1;
        self.block_type[index] = kind;
        self.block_health[index] = if kind == 1 {
            2
        } else if kind == 3 {
            255
        } else {
            1
        };
        true
    }
    pub fn destroy(&mut self, index: usize) {
        if self.blocks[index] == 0 {
            return;
        }
        self.blocks[index] = 0;
        self.award(10);
        self.sound = 2;
        self.burst(
            self.block_x[index] + BLOCK_WIDTH / 2.0,
            self.block_y[index] + BLOCK_HEIGHT / 2.0,
        );
    }
    pub fn hit(&mut self, index: usize) -> bool {
        if self.blocks[index] == 0 {
            return false;
        }
        if self.block_type[index] == 3 {
            self.sound = 1;
            return false;
        }
        if self.block_type[index] == 1 {
            self.block_health[index] = self.block_health[index].saturating_sub(1);
            if self.block_health[index] != 0 {
                self.sound = 2;
                return false;
            }
        }
        self.destroy(index);
        let x = self.block_x[index];
        let y = self.block_y[index];
        let roll = below(100);
        if roll < 15 && self.cooldown >= 60 {
            let weight = below(100);
            let kind = if weight < 50 {
                0
            } else if weight < 75 {
                1
            } else if weight < 90 {
                2
            } else {
                3
            };
            self.drops.push(Drop {
                x: x + BLOCK_WIDTH / 2.0,
                y,
                kind,
            });
            self.cooldown = 0;
        }
        true
    }
    pub fn explode(&mut self, x: f32, y: f32, rocket: bool) {
        for index in 0..200 {
            if self.blocks[index] == 0 {
                continue;
            }
            let dx = self.block_x[index] + BLOCK_WIDTH / 2.0 - x;
            let dy = self.block_y[index] + BLOCK_HEIGHT / 2.0 - y;
            let squared = dx * dx + dy * dy;
            // These different radii and sqrt/squared comparisons are deliberate:
            // rocket code uses 120 original pixels; explosive bricks use 60.
            let inside = if rocket {
                squared.sqrt() <= 60.0
            } else {
                squared <= 30.0 * 30.0
            };
            if inside {
                self.destroy(index);
            }
        }
        // Blast damage bypasses health/metal armor and never chains or drops bonuses.
        self.sound = 4;
    }
    pub fn collide_blocks(&mut self, ball: usize) {
        if ball >= self.balls.len() || !self.balls[ball].active() {
            return;
        }
        let mut explosions = Vec::new();
        for index in 0..200 {
            if self.blocks[index] == 0 {
                continue;
            }
            let x = self.block_x[index];
            let y = self.block_y[index];
            if !self.balls[ball].resolve_block(x, y, self.ghost > 0) {
                continue;
            }
            if self.hit(index) && self.block_type[index] == 2 {
                explosions.push((x + BLOCK_WIDTH / 2.0, y + BLOCK_HEIGHT / 2.0));
            }
            if self.ghost == 0 {
                break;
            }
        }
        for (x, y) in explosions {
            self.explode(x, y, false);
        }
    }
    pub fn update_rockets(&mut self) {
        let mut i = 0;
        while i < self.rockets.len() {
            self.rockets[i].1 -= 4.0;
            let (rx, ry) = self.rockets[i];
            let mut struck = 200;
            for index in 0..200 {
                let x = self.block_x[index];
                let y = self.block_y[index];
                if self.blocks[index] > 0
                    && ry >= TOP
                    && overlap([rx, ry, 5.0, 10.0], [x, y, BLOCK_WIDTH, BLOCK_HEIGHT]).0 > 0.0
                {
                    struck = index;
                    break;
                }
            }
            if struck < 200 {
                let x = self.block_x[struck] + BLOCK_WIDTH / 2.0;
                let y = self.block_y[struck] + BLOCK_HEIGHT / 2.0;
                self.destroy(struck);
                self.explode(x, y, true);
                self.rockets.remove(i);
                self.sound = 4;
            } else if ry < TOP {
                self.rockets.remove(i);
            } else {
                i += 1;
            }
        }
    }
    pub fn collect(&mut self, kind: u32) {
        self.award(2);
        self.sound = 3;
        match kind {
            0 => {
                if self.width == 70.0 {
                    let center = self.paddle + self.width / 2.0;
                    self.width = 90.0;
                    self.paddle = center - self.width / 2.0;
                }
                self.long = 300;
                self.paddle = self.paddle.clamp(0.0, 640.0 - self.width);
            }
            1 => {
                // Original bonus initially spawns offset from the paddle; on the
                // following attached update it aligns with the normal launch point.
                self.balls.push(Ball::new(
                    self.paddle + self.width / 2.0 + BALL_RADIUS,
                    PADDLE_Y - 7.0,
                ));
            }
            2 => self.ghost = 600,
            _ => self.ammo = self.ammo.saturating_add(1),
        }
    }
    pub fn update_paddle(&mut self, input: u32) -> f32 {
        if input & 257 != 0 {
            self.paddle = (self.paddle - 9.0).max(0.0);
        }
        if input & 514 != 0 {
            self.paddle = (self.paddle + 9.0).min(640.0 - self.width);
        }
        let movement = self.paddle - self.last_paddle;
        self.last_paddle = self.paddle;
        if self.long > 0 {
            self.long -= 1;
            if self.long == 0 {
                let center = self.paddle + self.width / 2.0;
                self.width = 70.0;
                self.paddle = (center - self.width / 2.0).clamp(0.0, 640.0 - self.width);
            }
        }
        if self.ghost > 0 {
            self.ghost -= 1;
        }
        movement
    }
    pub fn fire_or_launch(&mut self) {
        let mut launched = false;
        for ball in &mut self.balls {
            if ball.attached {
                ball.launch();
                launched = true;
                self.sound = 1;
            }
        }
        if !launched && self.ammo > 0 {
            self.ammo -= 1;
            self.rockets
                .push((self.paddle + self.width / 2.0 - 2.5, PADDLE_Y - 10.0));
            self.sound = 3;
        }
    }
    pub fn source_input(&mut self, kind: u32, x: f32, y: f32, code: u32) {
        if kind == 1 && self.mode == 1 && x.is_finite() && y.is_finite() {
            let original_x = (x.clamp(0.0, 640.0) * 2.0) as i32;
            let half_width = (self.width * 2.0) as i32 / 2;
            self.paddle = ((original_x - half_width) as f32 * 0.5).clamp(0.0, 640.0 - self.width);
        }
        if kind == 2 && code == 1 && (self.mode == 3 || self.mode == 5) {
            self.update(4);
        }
        if kind == 5 {
            let key = code & 65535;
            if key == 44 && self.mode == 1 {
                self.fire_or_launch();
            }
            if key == 41 && (self.mode == 1 || self.mode == 2) {
                self.mode = if self.mode == 1 { 2 } else { 1 };
            }
            if key == 40 && (self.mode == 0 || self.mode == 3 || self.mode == 5) {
                self.update(4);
            }
        }
        if kind == 9 {
            let key = code & 0x7fff_ffff;
            if key == 113 && self.mode == 1 {
                self.blocks = [0; 200];
            }
            if key == 114 && (self.mode == 2 || self.mode == 4 || self.mode == 5) {
                let best = self.best;
                let unlocked = self.unlocked;
                *self = Self::new();
                self.best = best;
                self.unlocked = unlocked;
                self.dirty = true;
            }
        }
    }
    pub fn update(&mut self, input: u32) {
        if input & 131072 != 0 {
            self.gravity = !self.gravity;
            self.dirty = true;
        }
        if input & 8 != 0 && (self.mode == 1 || self.mode == 2) {
            self.mode = if self.mode == 1 { 2 } else { 1 };
            if self.mode == 2 {
                return;
            }
        }
        if self.mode != 1 {
            if input & 4 != 0 {
                if self.mode == 0 {
                    self.level = self.unlocked;
                    self.populate();
                    self.attach();
                }
                if self.mode == 3 || self.mode == 5 {
                    self.level = self.level.saturating_add(1);
                    self.populate();
                    self.attach();
                }
                if self.mode == 4 {
                    let best = self.best;
                    let unlocked = self.unlocked;
                    *self = Self::new();
                    self.best = best;
                    self.unlocked = unlocked;
                    self.dirty = true;
                }
                self.mode = 1;
            }
            return;
        }
        self.frame = self.frame.wrapping_add(1);
        // Launch/fire are input events before the original per-frame update.
        if input & 4 != 0 {
            self.fire_or_launch();
        }
        let movement = self.update_paddle(input);
        self.cooldown = self.cooldown.saturating_add(1);
        for i in 0..self.balls.len() {
            let orbiting = self.portal;
            if !orbiting {
                if self.balls[i].attached {
                    self.balls[i].x = self.paddle + self.width / 2.0;
                    self.balls[i].y = PADDLE_Y - BALL_RADIUS;
                    if self.balls[i].launch_timer > 0 {
                        self.balls[i].launch_timer -= 1;
                    } else {
                        self.balls[i].launch();
                    }
                } else {
                    self.balls[i].advance(self.gravity);
                }
                let ball = &self.balls[i];
                let speed = (ball.vx * ball.vx + ball.vy * ball.vy).sqrt() * 120.0;
                self.max_speed = self.max_speed.max(speed);
                if self.max_speed >= 3600.0 {
                    self.portal = true;
                    self.award(5000);
                    self.burst(320.0, 240.0);
                    self.sound = 4;
                }
            }
            if orbiting {
                let angle = self.frame as f32 * 0.1 + i as f32 * (core::f32::consts::TAU / 3.0);
                self.balls[i].x = 320.0 + angle.cos() * 75.0;
                self.balls[i].y = 240.0 + angle.sin() * 75.0;
            }
        }
        self.collide_balls();
        for i in 0..self.balls.len() {
            if !self.balls[i].active() {
                continue;
            }
            let paddle_overlap = self.balls[i].intersects(self.paddle, PADDLE_Y, self.width, 11.0);
            if paddle_overlap.0 > 0.0 {
                let ball = &mut self.balls[i];
                ball.vy = -ball.vy.abs();
                let original_center = ((ball.x - BALL_RADIUS) * 2.0) as i32 + 6;
                let paddle_center = (self.paddle * 2.0) as i32 + (self.width * 2.0) as i32 / 2;
                let offset = (original_center - paddle_center) as f32 * 0.5;
                ball.vx += offset * 0.1;
                ball.spin = if movement.abs() > 1.0 {
                    movement * 0.3 + offset * 0.05
                } else {
                    offset * 0.02
                };
                self.award(5);
                self.sound = 1;
            }
            self.collide_blocks(i);
        }
        self.update_rockets();
        let mut i = 0;
        while i < self.drops.len() {
            self.drops[i].y += 1.0;
            let drop = &self.drops[i];
            if overlap(
                [drop.x, drop.y, 20.0, 20.0],
                [self.paddle, PADDLE_Y, self.width, 11.0],
            )
            .0 > 0.0
                && drop.y <= BOTTOM
            {
                let kind = drop.kind;
                self.drops.remove(i);
                self.collect(kind);
            } else if drop.y > BOTTOM {
                self.drops.remove(i);
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        if self.portal {
            self.pull_blocks();
        }
        while i < self.particles.len() {
            let particle = &mut self.particles[i];
            particle.x += particle.vx;
            particle.y += particle.vy;
            particle.vy += 0.06;
            particle.life -= 1;
            if particle.life == 0 {
                self.particles.remove(i);
            } else {
                i += 1;
            }
        }
        let mut i = 0;
        while i < self.balls.len() {
            if !self.balls[i].active() {
                self.balls.remove(i);
            } else {
                i += 1;
            }
        }
        if self.balls.is_empty() && !self.portal {
            self.lives = self.lives.saturating_sub(1);
            self.lost_life = true;
            self.score = self.score.saturating_sub(20);
            self.dirty = true;
            self.sound = 4;
            if self.lives == 0 {
                self.mode = 4;
            } else {
                self.respawn();
            }
        }
        let mut remaining = 0;
        for index in 0..200 {
            if self.block_type[index] != 3 {
                remaining += self.blocks[index] as u32;
            }
        }
        if remaining == 0 && !self.portal {
            self.finish_level();
        }
    }
    pub fn pull_blocks(&mut self) {
        let mut empty = true;
        for index in 0..200 {
            if self.blocks[index] == 0 {
                continue;
            }
            empty = false;
            let dx = 320.0 - (self.block_x[index] + BLOCK_WIDTH / 2.0);
            let dy = 240.0 - (self.block_y[index] + BLOCK_HEIGHT / 2.0);
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 2.5 {
                self.block_x[index] += (dx / distance * 8.0) as i32 as f32 * 0.5;
                self.block_y[index] += (dy / distance * 8.0) as i32 as f32 * 0.5;
            } else {
                self.blocks[index] = 0;
                self.burst(320.0, 240.0);
            }
        }
        if empty {
            self.portal_timer += 1;
            if self.portal_timer >= 270 {
                self.finish_level();
            }
        }
    }
}
