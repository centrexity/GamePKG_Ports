mod save_io;
mod procedural;
mod background;
mod portable_math;
// Adapted from c0m4r/arkanoo; see CREDITS.md for revision and changes.
mod audio;
mod music;
mod editor;
mod levels;
mod menu;
mod pattern_rng;
mod physics;
mod render;
mod speed_font;
mod editor_font;
mod dynamic_font;
mod dynamic_font_data;
use std::sync::Mutex;
static GAME: Mutex<Option<physics::Game>> = Mutex::new(None);
static MUSIC: Mutex<Option<music::Music>> = Mutex::new(None);

pub fn game_tick(input: u32, state: u64) -> u64 {
    if let Ok(mut slot) = GAME.lock() {
        let Ok(mut music_slot) = MUSIC.lock() else { return state; };
        if let Some(manager) = music_slot.as_mut() { manager.begin_tick(); }
        if slot.is_none() || state == 0 {
            let mut manager = music::Music::from_package();
            let mut game = physics::Game::new_audio(manager.has_playlist());
            if let Some(size) = gamepkg::save_size() {
                if size > 0 && size <= 1048576 {
                    if let Some((loaded, metadata)) = save_io::load_saved_editor(size) {
                        game.editor = loaded;
                        game.best = metadata[0];
                        game.unlocked = metadata[1].max(1);
                        game.gravity = metadata[2] == 1;
                        game.menu.sfx_volume = game.editor.sfx_volume;
                            game.menu.sfx_muted = game.editor.sfx_muted;
                        game.menu.music_volume = game.editor.music_volume;
                        game.menu.music_muted = game.editor.music_muted;
                    } else {
                        game.editor.readable = false;
                        game.editor.editor_message("SAVE DAMAGED / PRESERVED");
                    }
                } else if size > 1048576 {
                    game.editor.readable = false;
                }
            }
            *music_slot = if game.menu.music_available {
                manager.begin_tick();
                manager.start_saved(game.menu.music_volume, game.menu.music_muted);
                Some(manager)
            } else { None };
            *slot = Some(game);
        }
        if let Some(game) = slot.as_mut() {
            game.menu.vsync = gamepkg::window_vsync_preference();
            game.menu.observe_resolution(gamepkg::window_size(), gamepkg::window_fullscreen(), gamepkg::window_preview_active());
            game.frame_clock(gamepkg::frame_time_us());
            if let Some(events) = gamepkg::input_events() {
                for event in events {
                    game.source_input(event.kind, event.x, event.y, event.code);
                    resolution_request(game);
                    game.capture_sound();
                }
            }
            pattern_files(game);
            game.update(input);
            if let Some(fullscreen) = gamepkg::window_fullscreen() { game.menu.fullscreen = fullscreen; }
            render::draw(game);
            game.capture_sound();
            if let Some(samples) = game.effects.effect_render(game.menu.sfx_volume, game.menu.sfx_muted) {
                gamepkg::audio_submit_stereo(&samples);
            }
            if game.menu.music_available { gamepkg::effect_volume(if game.menu.sfx_muted {0} else {game.menu.sfx_volume}); }
            if let Some(manager) = music_slot.as_mut() {
                if manager.get_music_volume() != game.menu.music_volume as i32 { manager.set_music_volume(game.menu.music_volume as i32); }
                if manager.is_music_muted() != game.menu.music_muted { manager.set_music_muted(game.menu.music_muted); }
                manager.update();
                if game.quit_requested { manager.stop_music(); }
            }
            if game.dirty && game.editor.readable {
                let saved = if game.menu.music_available || game.menu.music_volume != 64 || game.menu.music_muted {
                    save_io::save_stream_audio_settings(&game.editor, game.best, game.unlocked, game.gravity,
                        [game.menu.sfx_volume, game.menu.music_volume], [game.menu.sfx_muted, game.menu.music_muted])
                } else { save_io::save_stream_settings(
                    &game.editor,
                    game.best,
                    game.unlocked,
                    game.gravity,
                    game.menu.sfx_volume,
                    game.menu.sfx_muted,
                ) };
                if saved {
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
            if game.homepage_requested {
                gamepkg::window_control(4, 0);
                game.homepage_requested = false;
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

fn resolution_request(game: &mut physics::Game) {
    if game.menu.vsync_requested {
        game.menu.vsync_requested = false;
        if let Some(vsync) = game.menu.vsync {
            let accepted = gamepkg::window_set_vsync(!vsync);
            game.menu.vsync_failed = !accepted;
            if accepted { game.menu.vsync = Some(!vsync); }
        }
    }
    let action = game.menu.resolution_request;
    game.menu.resolution_request = 0;
    if action == 0 { return; }
    let accepted = if let Some(size) = menu::Menu::resolution(action) {
        gamepkg::window_preview_size(size[0], size[1])
    } else if action == 18 {
        gamepkg::window_keep_size()
    } else if action == 19 {
        gamepkg::window_revert_size()
    } else { false };
    game.menu.resolution_result(action, accepted);
}

fn pattern_filename(name: &str) -> String { pattern_filename_segments([name, ""]) }
fn pattern_filename_segments(pieces: [&str; 2]) -> String {
    let mut filename = String::new();
    for character in pieces[0].chars().chain(pieces[1].chars()) {
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
                let pattern = editor::Pattern::pattern_from_cells(game.editor.name.clone(), game.editor.cells).unwrap();
                let ascii = pattern.ascii_write();
                let filename = pattern_filename_segments(pattern.name.name_segments());
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
                            game.editor.cells = pattern.pattern_unpack();
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
