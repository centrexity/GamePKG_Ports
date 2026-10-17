// 2048-rs adaptation. Upstream MIT notice: assets/LICENSE-2048.txt.
mod model;
mod render;
use std::sync::Mutex;
struct Session {
    game: model::Game,
    damaged: bool,
    save_failed: bool,
}
static GAME: Mutex<Option<Session>> = Mutex::new(None);
pub fn game_tick(input: u32, state: u64) -> u64 {
    if let Ok(mut slot) = GAME.lock() {
        if slot.is_none() {
            let size = gamepkg::save_size().unwrap_or(0);
            let mut loaded = None;
            if (52..=3636).contains(&size) {
                let mut bytes = Vec::new();
                bytes.resize(size as usize, 0u8);
                if gamepkg::save_read(0, &mut bytes) == Some(size) {
                    loaded = model::Game::decode(&bytes);
                }
            }
            let damaged = size != 0 && loaded.is_none();
            *slot = Some(Session {
                game: match loaded {
                    Some(game) => game,
                    None => model::Game::new(0),
                },
                damaged,
                save_failed: false,
            });
        } else if state == 0 {
            if let Some(session) = slot.as_mut() {
                session.game = model::Game::new(session.game.best);
                session.damaged = false;
            }
        }
        if let Some(session) = slot.as_mut() {
            if !session.damaged || input & 64 != 0 {
                session.game.update(input);
                if input & 64 != 0 {
                    session.damaged = false;
                }
            }
            if session.game.dirty && !session.damaged {
                session.save_failed = !gamepkg::save_write(&session.game.encode());
                if !session.save_failed {
                    session.game.dirty = false;
                }
            }
            render::draw(&session.game, session.damaged, session.save_failed);
            return (session.game.position.score.min(u64::from(u32::MAX)) << 32)
                | u64::from(session.game.revision);
        }
    }
    state
}
