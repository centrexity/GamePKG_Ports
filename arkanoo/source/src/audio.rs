// Current synthesized effects; original recorded effects/music remain separate work.
pub fn effect_samples(kind: u32, frame: u32, volume: u32, muted: bool) -> [i16; 800] {
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
        // Reduce the phase before adding: identical on 32/64-bit targets at u32::MAX.
        let phase = (frame % period + index as u32) % period;
        let sign = if phase < period / 2 { 1 } else { -1 };
        *sample = (sign * (800 - index) as i32 * 5 * gain / 64) as i16;
    }
    samples
}
