// Current synthesized effects; original recorded effects/music remain separate work.
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
// Recorded clips are not yet shipped; production currently supplies temporary tones.
pub struct Voice {
    samples: Vec<i16>,
    offset: usize,
}
pub struct Mixer {
    channels: [Option<Voice>; 4],
}
impl Mixer {
    pub fn new() -> Self {
        Self {
            channels: [None, None, None, None],
        }
    }
    pub fn effect_start(&mut self, samples: Vec<i16>, muted: bool) -> bool {
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
    pub fn effect_tone(&mut self, kind: u32, frame: u64, muted: bool) -> bool {
        if muted {
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
