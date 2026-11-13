// Four SDL-style effect channels. Clips are 48 kHz interleaved stereo PCM.
// All five clips are original recordings; see the audio license and provenance.
pub struct Voice {
    samples: std::sync::Arc<Vec<i16>>,
    offset: usize,
}
pub struct Mixer {
    hosted: bool,
    channels: [Option<Voice>; 4],
    recordings: [Option<std::sync::Arc<Vec<i16>>>; 5],
    recording_attempted: [bool; 5],
    recording_bytes: u32,
}
impl Mixer {
    pub fn new() -> Self {
        Self::new_mode(false)
    }
    pub fn new_mode(hosted: bool) -> Self {
        Self {
            hosted,
            channels: [None, None, None, None],
            recordings: [None, None, None, None, None],
            recording_attempted: [false; 5],
            recording_bytes: 0,
        }
    }
    pub fn effect_start(&mut self, samples: Vec<i16>, muted: bool) -> bool {
        self.effect_start_shared(std::sync::Arc::new(samples), muted)
    }
    pub fn effect_start_shared(&mut self, samples: std::sync::Arc<Vec<i16>>, muted: bool) -> bool {
        if muted || samples.is_empty() || !samples.len().is_multiple_of(2) {
            return false;
        }
        for channel in &mut self.channels {
            if channel.is_none() {
                *channel = Some(Voice { samples, offset: 0 });
                return true;
            }
        }
        false
    }
    pub fn effect_render(&mut self, volume: u32, muted: bool) -> Option<[i16; 1600]> {
        if self.hosted {
            return None;
        }
        let gain = if muted { 0 } else { volume.min(128) } as i32;
        let mut output = [0i16; 1600];
        let mut voice_present = false;
        for channel in &mut self.channels {
            if let Some(voice) = channel {
                voice_present = true;
                let count = (voice.samples.len() - voice.offset).min(output.len());
                for (index, sample) in output.iter_mut().enumerate().take(count) {
                    let scaled = i32::from(voice.samples[voice.offset + index]) * gain / 128;
                    *sample = (i32::from(*sample) + scaled).clamp(-32768, 32767) as i16;
                }
                voice.offset += count;
                if voice.offset == voice.samples.len() {
                    *channel = None;
                }
            }
        }
        if voice_present && gain > 0 {
            Some(output)
        } else {
            None
        }
    }
    pub fn effect_request(&mut self, kind: u32, _frame: u64, muted: bool) -> bool {
        // Each clip starts at its first sample, independent of the game frame.
        if muted || !(1..=5).contains(&kind) {
            return false;
        }
        if self.hosted {
            let path = match kind {
                1 => "audio/bounce.wav",
                2 => "audio/glass.wav",
                3 => "audio/launch.wav",
                4 => "audio/explosion.wav",
                _ => "audio/life-loss.wav",
            };
            return crate::gamepkg::effect_play(path);
        }
        let at = (kind - 1) as usize;
        if !self.recording_attempted[at] {
            self.recording_attempted[at] = true;
            let path = match kind {
                1 => "audio/bounce.wav",
                2 => "audio/glass.wav",
                3 => "audio/launch.wav",
                4 => "audio/explosion.wav",
                _ => "audio/life-loss.wav",
            };
            self.recordings[at] = read_recording(path, 4194304 - self.recording_bytes);
            if let Some(recorded) = &self.recordings[at] {
                self.recording_bytes += recorded.len() as u32 * 2;
            }
        }
        if let Some(recorded) = &self.recordings[at] {
            return self.effect_start_shared(recorded.clone(), false);
        }
        false
    }
}

fn recording_word(header: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
}
pub fn recording_length(header: &[u8]) -> Option<u32> {
    if header.len() != 44
        || &header[..4] != b"RIFF"
        || &header[8..16] != b"WAVEfmt "
        || recording_word(header, 16) != 16
        || header[20..24] != [1, 0, 2, 0]
        || recording_word(header, 24) != 48000
        || recording_word(header, 28) != 192000
        || header[32..36] != [4, 0, 16, 0]
        || &header[36..40] != b"data"
    {
        return None;
    }
    let size = recording_word(header, 40);
    if size == 0
        || size > 2097152
        || !size.is_multiple_of(4)
        || recording_word(header, 4) != size + 36
    {
        return None;
    }
    Some(size)
}
fn read_recording(path: &str, budget: u32) -> Option<std::sync::Arc<Vec<i16>>> {
    let mut header = [0u8; 44];
    if crate::gamepkg::asset_read(path, 0, &mut header) != Some(44) {
        return None;
    }
    let size = recording_length(&header)?;
    if size > budget {
        return None;
    }
    let mut pcm = Vec::with_capacity(size as usize / 2);
    let mut block = [0u8; 65536];
    let mut offset = 0u32;
    while offset < size {
        let count = (size - offset).min(65536);
        if crate::gamepkg::asset_read(path, 44 + offset, &mut block[..count as usize])
            != Some(count)
        {
            return None;
        }
        for part in block[..count as usize].as_chunks::<2>().0.iter() {
            pcm.push(i16::from_le_bytes([part[0], part[1]]));
        }
        offset += count;
    }
    let mut tail = [0u8; 1];
    if crate::gamepkg::asset_read(path, 44 + size, &mut tail) != Some(0) {
        return None;
    }
    Some(std::sync::Arc::new(pcm))
}
