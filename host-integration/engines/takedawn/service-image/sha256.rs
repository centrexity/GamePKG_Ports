// Dependency-free SHA-256 value computation. No platform or cryptography API.
const SHA_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];
pub struct Sha256 {
    state: [u32; 8],
    block: [u8; 64],
    used: u32,
    total: u64,
}
impl Sha256 {
    pub fn sha_new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
                0x5be0cd19,
            ],
            block: [0; 64],
            used: 0,
            total: 0,
        }
    }
    fn sha_compress(&mut self) {
        let mut words = [0u32; 64];
        for number in 0..16u32 {
            let at = (number * 4) as usize;
            words[number as usize] = u32::from_be_bytes([
                self.block[at],
                self.block[at + 1],
                self.block[at + 2],
                self.block[at + 3],
            ]);
        }
        for number in 16..64usize {
            let first = words[number - 15];
            let second = words[number - 2];
            let low = first.rotate_right(7) ^ first.rotate_right(18) ^ (first >> 3);
            let high = second.rotate_right(17) ^ second.rotate_right(19) ^ (second >> 10);
            words[number] = words[number - 16]
                .wrapping_add(low)
                .wrapping_add(words[number - 7])
                .wrapping_add(high);
        }
        let mut current = self.state;
        for number in 0..64usize {
            let sum = current[4].rotate_right(6)
                ^ current[4].rotate_right(11)
                ^ current[4].rotate_right(25);
            let choose = (current[4] & current[5]) ^ (!current[4] & current[6]);
            let first = current[7]
                .wrapping_add(sum)
                .wrapping_add(choose)
                .wrapping_add(SHA_K[number])
                .wrapping_add(words[number]);
            let sum = current[0].rotate_right(2)
                ^ current[0].rotate_right(13)
                ^ current[0].rotate_right(22);
            let majority =
                (current[0] & current[1]) ^ (current[0] & current[2]) ^ (current[1] & current[2]);
            let second = sum.wrapping_add(majority);
            current = [
                first.wrapping_add(second),
                current[0],
                current[1],
                current[2],
                current[3].wrapping_add(first),
                current[4],
                current[5],
                current[6],
            ];
        }
        for number in 0..8usize {
            self.state[number] = self.state[number].wrapping_add(current[number]);
        }
    }
    pub fn sha_update(&mut self, bytes: &[u8]) -> Result<(), i32> {
        let next = self.total.checked_add(bytes.len() as u64).ok_or(-1)?;
        if next > 0x1fffffffffffffff {
            return Err(-1);
        }
        self.total = next;
        let mut done = 0usize;
        while done < bytes.len() {
            let length = (64 - self.used as usize).min(bytes.len() - done);
            self.block[self.used as usize..self.used as usize + length]
                .copy_from_slice(&bytes[done..done + length]);
            self.used += length as u32;
            done += length;
            if self.used == 64 {
                self.sha_compress();
                self.used = 0;
            }
        }
        Ok(())
    }
    pub fn sha_finish(mut self) -> [u8; 32] {
        let bits = self.total * 8;
        self.block[self.used as usize] = 128;
        self.used += 1;
        if self.used > 56 {
            self.block[self.used as usize..].fill(0);
            self.sha_compress();
            self.used = 0;
        }
        self.block[self.used as usize..56].fill(0);
        self.block[56..].copy_from_slice(&u64::to_be_bytes(bits));
        self.sha_compress();
        let mut output = [0u8; 32];
        for number in 0..8usize {
            output[number * 4..number * 4 + 4]
                .copy_from_slice(&u32::to_be_bytes(self.state[number]));
        }
        output
    }
}
