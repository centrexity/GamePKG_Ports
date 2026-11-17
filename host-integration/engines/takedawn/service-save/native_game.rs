// Synthetic admitted game fixture: no user media and no physical audio output.
mod save_adapter;
mod save_files;
mod save_history;
mod save_paths;
mod save_recovery;
mod save_state;
mod save_worker;
mod worker_fixture;
pub fn save_worker_entry(argument: u64) -> u64 {
    save_worker::run(argument)
}

fn checkpoint(point: i16) {
    let _ = gamepkg::audio(&[point], 1);
    loop {
        if let Ok((now, generation)) = gamepkg::monotonic() {
            let _ = gamepkg::wait_until(now.saturating_add(100000000), generation);
        }
    }
}
pub fn game_main() {
    let grants = match gamepkg::grants() {
        Ok(data) => data,
        Err(_) => return,
    };
    let mut save = 0u64;
    let mut assets = 0u64;
    for item in grants {
        if item.kind == 2 {
            save = item.id;
        }
        if item.kind == 1 {
            assets = item.id;
        }
    }
    let control = match gamepkg::file_open(assets, false, "phase") {
        Ok(id) => id,
        Err(_) => return,
    };
    let mut byte = [0u8; 1];
    if gamepkg::file_read(control, 0, &mut byte) != Ok(1) {
        return;
    }
    if gamepkg::file_close(control) != Ok(()) {
        return;
    }
    if byte[0] >= 10 {
        match worker_fixture::exercise(save, byte[0]) {
            Ok(()) => {
                let _ = gamepkg::audio(&[200i16 + i16::from(byte[0])], 1);
            }
            Err(error) => {
                let _ = gamepkg::audio(&[error as i16], 1);
            }
        }
        return;
    }
    let mut guard = save_adapter::SaveGuard::new(save);
    if byte[0] == 0 {
        let mut startup = match save_recovery::Recovery::begin(&mut guard, save, "12345678") {
            Ok(data) => data,
            Err(_) => return,
        };
        loop {
            match startup.step() {
                Ok(save_recovery::RecoveryStep::Progress {}) => {}
                Ok(save_recovery::RecoveryStep::Done { .. }) => {
                    let _ = gamepkg::audio(&[100i16], 1);
                    return;
                }
                _ => return,
            }
        }
    }
    let handle = match guard.open_write("UDATA/12345678/profile") {
        Ok(id) => id,
        Err(_) => return,
    };
    if byte[0] == 1 {
        checkpoint(1);
    }
    if guard.write_at(handle, 0, b"new-save") != Ok(()) {
        return;
    }
    if byte[0] == 2 {
        checkpoint(2);
    }
    if guard.save_guard_close(handle) != Ok(()) {
        return;
    }
    if byte[0] == 3 {
        checkpoint(3);
    }
    if byte[0] == 4 {
        guard.set_history(true);
    }
    let mut job = match guard.begin_job() {
        Ok(Some(data)) => data,
        _ => return,
    };
    match job.step() {
        Ok(save_adapter::Step::History { .. }) => {
            if byte[0] == 4 {
                checkpoint(4);
            }
        }
        Ok(save_adapter::Step::Complete { finished }) => {
            let cleanup = match guard.finish(finished) {
                Ok(data) => data,
                Err(_) => return,
            };
            if save_adapter::clean_obsolete(save, cleanup) != Ok(()) {
                return;
            }
            if byte[0] == 5 {
                checkpoint(5);
            }
        }
        _ => {}
    }
}
