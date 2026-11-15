// Port-owned fixed certificate-key derivation; no file/OS/crypto dependency.
fn signature_block(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut words = [0u32; 80];
    for index in 0..16usize {
        let at = index * 4;
        words[index] = u32::from_be_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]]);
    }
    for index in 16..80usize {
        words[index] =
            (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                .rotate_left(1);
    }
    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    for index in 0..80usize {
        let (bits, constant) = if index < 20 {
            ((b & c) | (!b & d), 0x5a827999u32)
        } else if index < 40 {
            (b ^ c ^ d, 0x6ed9eba1)
        } else if index < 60 {
            ((b & c) | (b & d) | (c & d), 0x8f1bbcdc)
        } else {
            (b ^ c ^ d, 0xca62c1d6)
        };
        let next = a
            .rotate_left(5)
            .wrapping_add(bits)
            .wrapping_add(e)
            .wrapping_add(constant)
            .wrapping_add(words[index]);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = next;
    }
    for (index, value) in [a, b, c, d, e].iter().enumerate() {
        state[index] = state[index].wrapping_add(*value);
    }
}
fn signature_hash(pad: &[u8; 64], tail: &[u8]) -> [u8; 20] {
    let mut state = [
        0x67452301u32,
        0xefcdab89,
        0x98badcfe,
        0x10325476,
        0xc3d2e1f0,
    ];
    signature_block(&mut state, pad);
    // Private callers supply exactly 16 or 20 bytes; no arbitrary hash API.
    let mut block = [0u8; 64];
    for index in 0..tail.len() {
        block[index] = tail[index];
    }
    block[tail.len()] = 0x80;
    let bits = u64::to_be_bytes((64 + tail.len()) as u64 * 8);
    for index in 0..8usize {
        block[56 + index] = bits[index];
    }
    signature_block(&mut state, &block);
    let mut output = [0u8; 20];
    for index in 0..5usize {
        let bytes = u32::to_be_bytes(state[index]);
        for byte in 0..4usize {
            output[index * 4 + byte] = bytes[byte];
        }
    }
    output
}
pub fn console_signature(certificate: &[u8; 16]) -> [u8; 16] {
    // Public fixed signing constant from the pinned original implementation.
    let master = [
        0x5cu8, 0x07, 0x33, 0xae, 0x04, 0x01, 0xf7, 0xe8, 0xba, 0x79, 0x93, 0xfd, 0xcd, 0x2f, 0x1f,
        0xe0,
    ];
    let mut inner = [0x36u8; 64];
    let mut outer = [0x5cu8; 64];
    for index in 0..16usize {
        inner[index] ^= master[index];
        outer[index] ^= master[index];
    }
    let first = signature_hash(&inner, certificate);
    let second = signature_hash(&outer, &first);
    let mut output = [0u8; 16];
    for index in 0..16usize {
        output[index] = second[index];
    }
    output
}
