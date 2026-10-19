// Playlist control adapted from c0m4r/arkanoo audio.rs at
// 9d613e78c9ed5657ae7fda9206cd4a82a893b7c8. See CREDITS.md; no guest decoder.
// Tracks remain virtual package assets and advance on the host output clock.
pub struct Music {
    songs: Vec<String>,
    current: usize,
    volume: i32,
    muted: bool,
    should_play: bool,
    requested: bool,
    staged_play: bool,
}
impl Music {
    pub fn new(mut songs: Vec<String>) -> Self {
        songs.sort();
        let current = if songs.is_empty() {
            0
        } else {
            crate::physics::below(songs.len() as u64) as usize
        };
        Self {
            songs,
            current,
            volume: 64,
            muted: false,
            should_play: false,
            requested: false,
            staged_play: false,
        }
    }
    pub fn from_package() -> Self {
        let mut bytes = Vec::new();
        let mut block = [0u8; 1024];
        loop {
            let Some(count) =
                crate::gamepkg::asset_read("audio/music-list.txt", bytes.len() as u32, &mut block)
            else {
                return Self::new(Vec::new());
            };
            if count == 0 {
                break;
            }
            if count > 1024 || bytes.len() + count as usize > 1048576 {
                return Self::new(Vec::new());
            }
            bytes.extend_from_slice(&block[..count as usize]);
        }
        let Ok(playlist_text) = String::from_utf8(bytes) else {
            return Self::new(Vec::new());
        };
        let mut songs = Vec::new();
        for path in playlist_text.lines() {
            let mut portable =
                path.starts_with("music/") && path.ends_with(".mp3") && path.len() <= 1024;
            for part in path.split('/') {
                if part.is_empty()
                    || part == "."
                    || part == ".."
                    || part.len() > 255
                    || part.ends_with('.')
                    || part.ends_with(' ')
                {
                    portable = false;
                }
            }
            for character in path.chars() {
                if character.is_control()
                    || character == '\\'
                    || character == ':'
                    || character == '<'
                    || character == '>'
                    || character == '"'
                    || character == '|'
                    || character == '?'
                    || character == '*'
                {
                    portable = false;
                }
            }
            if portable {
                songs.push(path.to_string());
            }
        }
        Self::new(songs)
    }
    pub fn begin_tick(&mut self) {
        self.staged_play = false;
    }
    fn active(&self) -> bool {
        self.staged_play
            || (self.requested
                && !crate::gamepkg::music_finished()
                && !crate::gamepkg::music_failed())
    }
    pub fn play_music(&mut self) {
        if self.songs.is_empty() || self.muted {
            return;
        }
        self.should_play = true;
        crate::gamepkg::music_volume(self.volume as u32);
        self.requested = crate::gamepkg::music_play(&self.songs[self.current]);
        self.staged_play = self.requested;
    }
    pub fn update(&mut self) {
        if !self.muted && self.should_play && !self.songs.is_empty() && !self.active() {
            self.current = crate::physics::below(self.songs.len() as u64) as usize;
            self.play_music();
        }
    }
    pub fn stop_music(&mut self) {
        crate::gamepkg::music_stop();
        self.should_play = false;
        self.requested = false;
        self.staged_play = false;
    }
    pub fn set_music_volume(&mut self, volume: i32) {
        self.volume = volume.clamp(0, 128);
        crate::gamepkg::music_volume(self.volume as u32);
    }
    pub fn get_music_volume(&self) -> i32 {
        self.volume
    }
    pub fn set_music_muted(&mut self, muted: bool) {
        let was_muted = self.muted;
        self.muted = muted;
        crate::gamepkg::music_volume(if muted { 0 } else { self.volume as u32 });
        // SDL resume on an inactive stream is a no-op. Do not stage an invalid
        // GamePKG pause transaction when there is no active generation.
        if (muted || was_muted) && self.active() {
            crate::gamepkg::music_pause(muted);
        }
    }
    pub fn is_music_muted(&self) -> bool {
        self.muted
    }
    pub fn toggle_music_mute(&mut self) {
        self.set_music_muted(!self.muted);
    }
}
