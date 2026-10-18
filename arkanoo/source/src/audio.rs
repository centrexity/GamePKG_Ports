// Bounce, glass and launch remain temporary tones; two credited recordings load below.
pub fn effect_samples(kind: u32, frame: u64, volume: u32, muted: bool) -> [i16; 800] {
    let mut samples = [0i16; 800];
    if muted || volume == 0 {
        return samples;
    }
    let period = match kind {
        1 => 100,
        2 => 64,
        3 => 48,
        _ => 180,
    };
    let gain = volume.min(128) as i32;
    for (index, sample) in samples.iter_mut().enumerate() {
        // Reduce the phase before adding: identical on 32/64-bit targets at u64::MAX.
        let phase = (frame % period + index as u64) % period;
        let sign = if phase < period / 2 { 1 } else { -1 };
        *sample = (sign * (800 - index) as i32 * 5 * gain / 64) as i16;
    }
    samples
}

// Four SDL-style effect channels. Clips are 48 kHz interleaved stereo PCM.
// Life-loss and explosion are recorded; the other effects remain temporary tones.
pub struct Voice {
    samples: std::sync::Arc<Vec<i16>>,
    offset: usize,
}
pub struct Mixer {
    channels: [Option<Voice>; 4],
    recordings: [Option<std::sync::Arc<Vec<i16>>>; 2],
    recording_attempted: [bool; 2],
}
impl Mixer {
    pub fn new() -> Self {
        Self {
            channels: [None, None, None, None],
            recordings: [None, None],
            recording_attempted: [false, false],
        }
    }
    pub fn effect_start(&mut self, samples: Vec<i16>, muted: bool) -> bool {
        self.effect_start_shared(std::sync::Arc::new(samples), muted)
    }
    pub fn effect_start_shared(&mut self, samples: std::sync::Arc<Vec<i16>>, muted: bool) -> bool {
        if muted || samples.is_empty() || samples.len() % 2 != 0 {
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
    pub fn effect_request(&mut self, kind: u32, frame: u64, muted: bool) -> bool {
        if muted {
            return false;
        }
        if kind == 4 || kind == 5 {
            let at = if kind == 4 { 0 } else { 1 };
            if !self.recording_attempted[at] {
                self.recording_attempted[at] = true;
                let path = if kind == 4 {
                    "audio/explosion.wav"
                } else {
                    "audio/life-loss.wav"
                };
                self.recordings[at] = read_recording(path);
            }
            if let Some(recorded) = &self.recordings[at] {
                return self.effect_start_shared(recorded.clone(), false);
            }
            return false;
        }
        let mono = effect_samples(kind, frame, 128, false);
        let mut stereo = Vec::with_capacity(1600);
        for sample in mono {
            stereo.push(sample);
            stereo.push(sample);
        }
        self.effect_start(stereo, false)
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
    if size == 0 || size > 2097152 || size % 4 != 0 || recording_word(header, 4) != size + 36 {
        return None;
    }
    Some(size)
}
fn read_recording(path: &str) -> Option<std::sync::Arc<Vec<i16>>> {
    let mut header = [0u8; 44];
    if crate::gamepkg::asset_read(path, 0, &mut header) != Some(44) {
        return None;
    }
    let size = recording_length(&header)?;
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
        for part in block[..count as usize].chunks_exact(2) {
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
