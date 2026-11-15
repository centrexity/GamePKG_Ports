// Maintainer-only SHA oracle against the existing app's independently supplied
// SHA-256 implementation. This wrapper is never admitted game code.
#[path = "sha256.rs"]
mod sha256;
fn record(file: &mut Option<std::fs::File>, bytes: &[u8]) {
    use std::io::Write;
    if let Some(file) = file {
        let hash = gamepkg::verify::digest(bytes);
        file.write_all(&(bytes.len() as u32).to_le_bytes()).unwrap();
        file.write_all(bytes).unwrap();
        let digest: Vec<_> = (0..32)
            .map(|n| u8::from_str_radix(&hash[n * 2..n * 2 + 2], 16).unwrap())
            .collect();
        file.write_all(&digest).unwrap();
    }
}
fn main() {
    use std::io::Write;
    let mut file = std::env::args_os()
        .nth(1)
        .map(|path| std::fs::File::create(path).unwrap());
    if let Some(file) = file.as_mut() {
        file.write_all(b"SHV1").unwrap();
        file.write_all(&271u32.to_le_bytes()).unwrap();
    }

    let mut seed = 0x531dcefab7918462u64;
    let mut vectors = 0;
    for length in (0..=256).chain([
        511, 512, 513, 1023, 1024, 1025, 65535, 65536, 65537, 1048576, 1048577,
    ]) {
        let mut bytes = vec![0u8; length];
        for byte in &mut bytes {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            *byte = seed as u8;
        }
        record(&mut file, &bytes);
        for step in [1usize, 7, 55, 56, 63, 64, 65, 257, 65536] {
            let mut hash = sha256::Sha256::sha_new();
            for part in bytes.chunks(step) {
                hash.sha_update(part).unwrap();
                hash.sha_update(&[]).unwrap();
            }
            let digest = hash.sha_finish();
            let text: String = digest.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(text, gamepkg::verify::digest(&bytes));
            vectors += 1;
        }
    }
    for bytes in [
        b"".as_slice(),
        b"abc".as_slice(),
        b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq".as_slice(),
    ] {
        record(&mut file, bytes);
        let mut hash = sha256::Sha256::sha_new();
        hash.sha_update(bytes).unwrap();
        assert_eq!(
            hash.sha_finish()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            gamepkg::verify::digest(bytes)
        );
        vectors += 1;
    }
    println!("PASS: {vectors} SHA-256 comparisons across block/padding/streaming boundaries, including >1 MiB and empty updates");
}
