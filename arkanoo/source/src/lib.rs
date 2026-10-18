// Adapted from c0m4r/arkanoo; see CREDITS.md for revision and changes.
mod audio;
mod editor;
mod levels;
mod menu;
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
                    let mut offset = 0u32;
                    let mut read_ok = true;
                    while offset < size {
                        let length = (size - offset).min(65536);
                        if gamepkg::save_read(
                            offset,
                            &mut bytes[offset as usize..(offset + length) as usize],
                        ) != Some(length)
                        {
                            read_ok = false;
                            break;
                        }
                        offset += length;
                    }
                    if read_ok {
                        if let Some(metadata) = game.editor.save_decode(&bytes) {
                            game.best = metadata[0];
                            game.unlocked = metadata[1].max(1);
                            game.gravity = metadata[2] == 1;
                            game.menu.sfx_volume = game.editor.sfx_volume;
                            game.menu.sfx_muted = game.editor.sfx_muted;
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
            pattern_files(game);
            game.update(input);
            render::draw(game);
            if game.sound > 0 {
                render::sound(
                    game.sound,
                    game.frame,
                    game.menu.sfx_volume,
                    game.menu.sfx_muted,
                );
                game.sound = 0;
            }
            if game.dirty && game.editor.readable {
                let bytes = game.editor.save_encode_settings(
                    game.best,
                    game.unlocked,
                    game.gravity,
                    game.menu.sfx_volume,
                    game.menu.sfx_muted,
                );
                if gamepkg::save_write(&bytes) {
                    game.dirty = false;
                    game.menu.save_failed = false;
                    game.editor.editor_commit(true);
                } else {
                    game.menu.save_failed = true;
                    if game.editor.save_requested {
                        game.dirty = false;
                        game.editor.editor_commit(false);
                    }
                }
            }
            let confined = game.cursor_confined();
            gamepkg::window_control(0, u32::from(!confined));
            gamepkg::window_control(1, u32::from(confined));
            while game.fullscreen_requests > 0 {
                gamepkg::window_control(2, 0);
                game.fullscreen_requests -= 1;
            }
            if game.quit_requested {
                gamepkg::window_control(3, 0);
                game.quit_requested = false;
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

fn pattern_filename(name: &str) -> String {
    let mut filename = String::new();
    for character in name.chars() {
        if filename.len() + character.len_utf8() > 240 {
            break;
        }
        if character.is_control() || "<>:\"/\\|?*".contains(character) {
            filename.push('_');
        } else {
            filename.push(character);
        }
    }
    let stem = filename
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    let tail = if let Some(digit) = stem.strip_prefix("COM") {
        digit
    } else if let Some(digit) = stem.strip_prefix("LPT") {
        digit
    } else {
        ""
    };
    let numbered = match tail {
        "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³" => true,
        _ => false,
    };
    if filename.is_empty()
        || stem == "CON"
        || stem == "PRN"
        || stem == "AUX"
        || stem == "NUL"
        || stem == "CONIN$"
        || stem == "CONOUT$"
        || numbered
    {
        filename = String::from("pattern");
    }
    filename.push_str(".txt");
    filename
}
fn pattern_files(game: &mut physics::Game) {
    if game.editor.file_requested > 0 {
        let operation = game.editor.file_requested;
        game.editor.file_requested = 0;
        if game.editor.file_handle.is_some() {
            game.editor.editor_message("FILE REQUEST STILL OPEN");
        } else {
            let handle = if operation == 1 {
                gamepkg::file_import(0)
            } else {
                let pattern = editor::Pattern {
                    name: game.editor.name.clone(),
                    cells: game.editor.cells,
                };
                let ascii = pattern.ascii_write();
                let filename = pattern_filename(&pattern.name);
                gamepkg::file_export(0, &filename, ascii.as_bytes())
            };
            if let Some(id) = handle {
                game.editor.file_handle = Some(id);
                game.editor.file_operation = operation;
                game.editor.editor_message("CHOOSE A FILE IN THE PICKER");
            } else {
                game.editor
                    .editor_message("FILE PICKER UNAVAILABLE / LIMIT");
            }
        }
    }
    if let Some(handle) = game.editor.file_handle {
        if let Some(result) = gamepkg::file_status(handle) {
            if result.status == 1 {
                return;
            }
            if result.status == 2
                && game.editor.file_operation == 1
                && game.mode == 6
                && result.size <= 1048576
                && result.name_bytes <= 256
            {
                let mut names = [0u8; 256];
                let mut bytes = Vec::new();
                bytes.resize(result.size as usize, 0u8);
                let mut offset = 0u32;
                let mut read_ok = true;
                while offset < result.size {
                    let length = (result.size - offset).min(65536);
                    if gamepkg::file_read(
                        handle,
                        offset,
                        &mut bytes[offset as usize..(offset + length) as usize],
                    ) != Some(length)
                    {
                        read_ok = false;
                        break;
                    }
                    offset += length;
                }
                let received = gamepkg::file_name(handle, &mut names);
                if read_ok && received == Some(result.name_bytes) {
                    if let Ok(filename) = str::from_utf8(&names[..result.name_bytes as usize]) {
                        if let Ok(content) = str::from_utf8(&bytes) {
                            let name = if let Some((stem, _)) = filename.rsplit_once('.') {
                                stem
                            } else {
                                filename
                            };
                            let pattern = editor::Pattern::ascii_read(name, content);
                            game.editor.cells = pattern.cells;
                            game.editor.health = [1; 200];
                            game.editor.name = pattern.name;
                            game.editor.editor_message("IMPORTED / S TO SAVE");
                        } else {
                            game.editor.editor_message("PATTERN MUST BE UTF-8 TEXT");
                        }
                    } else {
                        game.editor.editor_message("INVALID FILE NAME");
                    }
                } else {
                    game.editor.editor_message("UNABLE TO READ SELECTED FILE");
                }
            } else if result.status == 3 {
                game.editor.editor_message("PATTERN EXPORTED");
            } else if result.status == 4 {
                game.editor.editor_message("FILE SELECTION CANCELLED");
            } else {
                game.editor.editor_message("FILE REQUEST FAILED");
            }
            gamepkg::file_close(handle);
            game.editor.file_handle = None;
            game.editor.file_operation = 0;
        } else {
            game.editor.file_handle = None;
            game.editor.editor_message("FILE REQUEST EXPIRED");
        }
    }
}
