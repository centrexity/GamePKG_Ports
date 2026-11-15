// Maintainer-only cross-target replay; synthetic fixtures, not admitted source.
#[path = "sha256.rs"]
mod sha256;
use std::io::Read;
fn main() {
    let mut file = std::fs::File::open(std::env::args_os().nth(1).unwrap()).unwrap();
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, b"SHV1");
    file.read_exact(&mut magic).unwrap();
    let count = u32::from_le_bytes(magic);
    assert!(count > 0 && count <= 4096);
    let mut comparisons = 0;
    for _ in 0..count {
        file.read_exact(&mut magic).unwrap();
        let length = u32::from_le_bytes(magic) as usize;
        assert!(length <= 1048577);
        let mut bytes = vec![0u8; length];
        file.read_exact(&mut bytes).unwrap();
        let mut expected = [0u8; 32];
        file.read_exact(&mut expected).unwrap();
        for step in [1usize, 7, 55, 56, 63, 64, 65, 257, 65536] {
            let mut hash = sha256::Sha256::sha_new();
            for part in bytes.chunks(step) {
                hash.sha_update(part).unwrap();
                hash.sha_update(&[]).unwrap();
            }
            assert_eq!(hash.sha_finish(), expected);
            comparisons += 1;
        }
    }
    assert_eq!(file.read(&mut magic).unwrap(), 0);
    println!("PASS: {comparisons} exact SHA-256 streaming comparisons on this target");
}
