// Scalar ChaCha12/PCG seed expansion and rand 0.8.5 sampling compatibility.
// Adapted from the Rand project (MIT); see assets/LICENSE-RAND.txt.
// This public seeded stream reproduces level layouts and brick refraction lines.
pub struct Rng {
    key: [u32; 8],
    counter: u64,
    words: [u32; 16],
    index: usize,
}
fn quarter(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(7);
}
impl Rng {
    pub fn new(mut seed: u64) -> Self {
        let mut key = [0u32; 8];
        for word in &mut key {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(11634580027462260723);
            let shifted = (((seed >> 18) ^ seed) >> 27) as u32;
            *word = shifted.rotate_right((seed >> 59) as u32);
        }
        Self {
            key,
            counter: 0,
            words: [0; 16],
            index: 16,
        }
    }
    pub fn next_u32(&mut self) -> u32 {
        if self.index == 16 {
            let mut state = [0u32; 16];
            state[0] = 0x61707865;
            state[1] = 0x3320646e;
            state[2] = 0x79622d32;
            state[3] = 0x6b206574;
            state[4..12].copy_from_slice(&self.key);
            state[12] = self.counter as u32;
            state[13] = (self.counter >> 32) as u32;
            let initial = state;
            for _ in 0..6 {
                quarter(&mut state, 0, 4, 8, 12);
                quarter(&mut state, 1, 5, 9, 13);
                quarter(&mut state, 2, 6, 10, 14);
                quarter(&mut state, 3, 7, 11, 15);
                quarter(&mut state, 0, 5, 10, 15);
                quarter(&mut state, 1, 6, 11, 12);
                quarter(&mut state, 2, 7, 8, 13);
                quarter(&mut state, 3, 4, 9, 14);
            }
            for i in 0..16 {
                self.words[i] = state[i].wrapping_add(initial[i]);
            }
            self.counter = self.counter.wrapping_add(1);
            self.index = 0;
        }
        let value = self.words[self.index];
        self.index += 1;
        value
    }
    pub fn next_u64(&mut self) -> u64 {
        let low = u64::from(self.next_u32());
        low | (u64::from(self.next_u32()) << 32)
    }
    pub fn range(&mut self, low: u64, high: u64) -> u64 {
        // All callers supply finite positive ranges. Fixed u64 reproduces the
        // original 64-bit usize sampler on every supported guest architecture.
        let span = high - low;
        let zone = (span << span.leading_zeros()).wrapping_sub(1);
        loop {
            let product = u128::from(self.next_u64()) * u128::from(span);
            if product as u64 <= zone {
                return low + (product >> 64) as u64;
            }
        }
    }
    pub fn range_i32(&mut self, low: i32, high: i32) -> i32 {
        let span = (high - low) as u32;
        let zone = (span << span.leading_zeros()).wrapping_sub(1);
        loop {
            let product = u64::from(self.next_u32()) * u64::from(span);
            if product as u32 <= zone {
                return low + (product >> 32) as i32;
            }
        }
    }
    pub fn float(&mut self, low: f32, high: f32) -> f32 {
        loop {
            let value = f32::from_bits((self.next_u32() >> 9) | 0x3f800000) - 1.0;
            let result = value * (high - low) + low;
            if result < high {
                return result;
            }
        }
    }
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16777216.0)
    }
    pub fn sixty_percent(&mut self) -> bool {
        self.next_u64() < 11068046444225730560
    }
}
