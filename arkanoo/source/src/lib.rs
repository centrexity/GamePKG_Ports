// Adapted from c0m4r/arkanoo; see CREDITS.md for revision and changes.
mod editor;
mod levels;
mod pattern_rng;
mod physics;
mod render;
use std::sync::Mutex;
static GAME: Mutex<Option<physics::Game>> = Mutex::new(None);

pub fn game_tick(input: u32, state: u64) -> u64 {
    if let Ok(mut slot) = GAME.lock() {
        if slot.is_none() || state == 0 {
            let mut game = physics::Game::new();
            if let Some(size) = gamepkg::save_size() {
                if size > 0 && size <= 1048576 {
                    let mut bytes = Vec::new();
                    bytes.resize(size as usize, 0);
                    if gamepkg::save_read(0, &mut bytes) == Some(size) {
                        if let Some(metadata) = game.editor.save_decode(&bytes) {
                            game.best = metadata[0];
                            game.unlocked = metadata[1].max(1);
                            game.gravity = metadata[2] == 1;
                        } else {
                            game.editor.readable = false;
                            game.editor.editor_message("SAVE DAMAGED / PRESERVED");
                        }
                    } else {
                        game.editor.readable = false;
                    }
                } else if size > 1048576 {
                    game.editor.readable = false;
                }
            }
            *slot = Some(game);
        }
        if let Some(game) = slot.as_mut() {
            if let Some(events) = gamepkg::input_events() {
                for event in events {
                    game.source_input(event.kind, event.x, event.y, event.code);
                }
            }
            game.update(input);
            render::draw(game);
            if game.sound > 0 {
                render::sound(game.sound, game.frame);
                game.sound = 0;
            }
            if game.dirty && game.editor.readable {
                let bytes = game
                    .editor
                    .save_encode(game.best, game.unlocked, game.gravity);
                if gamepkg::save_write(&bytes) {
                    game.dirty = false;
                    game.editor.editor_commit(true);
                } else if game.editor.save_requested {
                    game.dirty = false;
                    game.editor.editor_commit(false);
                }
            }
            if game.score >= 10 {
                gamepkg::achievement_unlock(0);
            }
            if game.unlocked >= 2 {
                gamepkg::achievement_unlock(1);
            }
            if game.unlocked >= 10 {
                gamepkg::achievement_unlock(2);
            }
        }
    }
    state.wrapping_add(1)
}
