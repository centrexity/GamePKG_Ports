// Maintainer-only deterministic transport oracle. Not admitted game source.
#![allow(dead_code)]
#[path = "disc.rs"]
mod disc;
use std::sync::Mutex;
struct Source {
    data: Vec<u8>,
    length: u64,
    maximum: usize,
    reads: u64,
    denied: bool,
    overreply: bool,
}
static SOURCE: Mutex<Option<Source>> = Mutex::new(None);
mod gamepkg {
    // This fixture tests the parser and cursor with a synthetic reader.
    // The admitted probe exercises open/close on the real broker.
    pub fn file_rom_view(_: u64, _: u64, _: u64) -> Result<u64, i32> {
        Err(-5)
    }

    pub fn file_size(handle: u64) -> Result<u64, i32> {
        let source = crate::SOURCE.lock().unwrap();
        let s = source.as_ref().unwrap();
        if handle != 7 || s.denied {
            return Err(-4);
        }
        Ok(s.length)
    }
    pub fn file_read(handle: u64, offset: u64, bytes: &mut [u8]) -> Result<usize, i32> {
        let mut source = crate::SOURCE.lock().unwrap();
        let s = source.as_mut().unwrap();
        if handle != 7 || s.denied {
            return Err(-4);
        }
        assert!(bytes.len() <= 65536);
        s.reads += 1;
        if s.overreply {
            return Ok(bytes.len() + 1);
        }
        let start = usize::try_from(offset).map_err(|_| -1)?;
        if start >= s.data.len() {
            return Ok(0);
        }
        let count = bytes.len().min(s.data.len() - start).min(s.maximum);
        bytes[..count].copy_from_slice(&s.data[start..start + count]);
        Ok(count)
    }
}
fn install(data: Vec<u8>) {
    let length = data.len() as u64;
    *SOURCE.lock().unwrap() = Some(Source {
        data,
        length,
        maximum: 65536,
        reads: 0,
        denied: false,
        overreply: false,
    });
}
fn word(b: &mut [u8], at: usize, value: u32) {
    b[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn node(b: &mut [u8], at: usize, name: &[u8], sector: u32, bytes: u32, dir: bool, right: u16) {
    b[at..at + 2].copy_from_slice(&0u16.to_le_bytes());
    b[at + 2..at + 4].copy_from_slice(&right.to_le_bytes());
    word(b, at + 4, sector);
    word(b, at + 8, bytes);
    b[at + 12] = if dir { 16 } else { 0 };
    b[at + 13] = u8::try_from(name.len()).unwrap();
    b[at + 14..at + 14 + name.len()].copy_from_slice(name);
}
fn image() -> Vec<u8> {
    let mut b = vec![0u8; 4 * 1024 * 1024];
    b[65536..65556].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
    b[65536 + 0x7ec..65536 + 2048].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
    word(&mut b, 65536 + 0x14, 40);
    word(&mut b, 65536 + 0x18, 2048);
    node(&mut b, 40 * 2048, b"X", 50, 2 * 1024 * 1024, false, 0);
    for (n, byte) in b[50 * 2048..50 * 2048 + 2 * 1024 * 1024]
        .iter_mut()
        .enumerate()
    {
        *byte = (n.wrapping_mul(37) & 255) as u8;
    }
    b
}
#[test]
fn partial_reads_zero_reads_overreply_and_revocation_are_explicit() {
    install(image());
    SOURCE.lock().unwrap().as_mut().unwrap().maximum = 7;
    let image = disc::Disc::disc_open(7).unwrap();
    assert_eq!(image.disc_count(), 1);
    assert_eq!(image.disc_find("x"), Ok(0));
    let mut bytes = [0u8; 257];
    image.disc_read(0, 13, &mut bytes).unwrap();
    for (index, byte) in bytes.iter().enumerate() {
        assert_eq!(*byte, ((index + 13) * 37 & 255) as u8);
    }
    assert!(SOURCE.lock().unwrap().as_ref().unwrap().reads > 500);
    image.disc_read(0, 2 * 1024 * 1024, &mut []).unwrap();
    let before = SOURCE.lock().unwrap().as_ref().unwrap().reads;
    assert_eq!(image.disc_read(0, 2 * 1024 * 1024, &mut [0]), Err(-1));
    assert_eq!(image.disc_read(0, u64::MAX, &mut [0]), Err(-1));
    assert_eq!(image.disc_read(0, 0, &mut vec![0; 1048577]), Err(-1));
    assert_eq!(SOURCE.lock().unwrap().as_ref().unwrap().reads, before);
    SOURCE.lock().unwrap().as_mut().unwrap().denied = true;
    assert_eq!(image.disc_read(0, 0, &mut [0]), Err(-4));
    assert_eq!(image.disc_read(0, 0, &mut []), Err(-4));
    assert!(matches!(disc::Disc::disc_open(7), Err(-4)));
    assert!(matches!(disc::Disc::disc_open(8), Err(-4)));
    install(image_for_short());
    SOURCE.lock().unwrap().as_mut().unwrap().overreply = true;
    assert!(matches!(disc::Disc::disc_open(7), Err(-3)));
}
#[test]
fn streams_keep_independent_cursors_and_never_cross_the_file_extent() {
    install(image());
    let disc = disc::Disc::disc_open(7).unwrap();
    let mut first = disc.disc_open_file("x").unwrap();
    let mut second = disc.disc_open_entry(0).unwrap();
    let mut bytes = [0u8; 8];
    assert_eq!(first.disc_file_read(&mut bytes), Ok(8));
    assert_eq!(first.disc_file_position(), 8);
    assert_eq!(second.disc_file_position(), 0);
    let size = u64::from(first.disc_file_size());
    bytes.fill(0xa5);
    assert_eq!(first.disc_file_read_at(size - 2, &mut bytes), Ok(2));
    assert_eq!(&bytes[..2], &[182, 219]);
    assert_eq!(&bytes[2..], &[0xa5; 6]);
    assert_eq!(first.disc_file_read_at(u64::MAX, &mut bytes), Ok(0));
    assert_eq!(first.disc_file_position(), 8);
    assert_eq!(
        first.disc_file_read_at(size, &mut vec![0; 1048577]),
        Err(-1)
    );
    assert_eq!(second.disc_file_read(&mut bytes), Ok(8));

    // The provider allows a short prefix and then unexpectedly hits physical
    // EOF, before the indexed logical end. The failed request must not advance.
    SOURCE
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .data
        .truncate(50 * 2048 + 10);
    assert_eq!(first.disc_file_read(&mut bytes), Err(-3));
    assert_eq!(first.disc_file_position(), 8);
    SOURCE.lock().unwrap().as_mut().unwrap().denied = true;
    assert_eq!(first.disc_file_read_at(size, &mut []), Err(-4));
    assert_eq!(first.disc_file_read(&mut bytes), Err(-4));
    assert_eq!(first.disc_file_position(), 8);
    assert_eq!(disc.disc_open_file("x").err(), Some(-4));
}
fn image_for_short() -> Vec<u8> {
    let mut b = image();
    b.truncate(82000);
    word(&mut b, 65536 + 0x18, 16);
    node(&mut b, 81920, b"X", 40, 16, false, 0);
    b
}
#[test]
fn declared_size_cannot_hide_a_short_underlying_read() {
    let mut b = image();
    b.truncate(81920 + 25);
    let length = 4 * 1024 * 1024;
    install(b);
    SOURCE.lock().unwrap().as_mut().unwrap().length = length;
    assert!(matches!(disc::Disc::disc_open(7), Err(-3)));
}
#[test]
fn exact_entry_capacity_and_one_over_are_bounded() {
    for count in [8192u32, 8193] {
        let mut b = image();
        word(&mut b, 65536 + 0x18, count * 24);
        for n in 0..count {
            let right = if n + 1 < count {
                u16::try_from((n + 1) * 6).unwrap()
            } else {
                0
            };
            node(
                &mut b,
                81920 + n as usize * 24,
                format!("F{n:04X}").as_bytes(),
                0,
                0,
                false,
                right,
            );
        }
        install(b);
        let result = disc::Disc::disc_open(7);
        if count == 8192 {
            assert_eq!(result.unwrap().disc_count(), 8192);
        } else {
            assert!(matches!(result, Err(-1)));
        }
    }
}
#[test]
fn directory_depth_and_utf8_path_boundaries_are_exact() {
    for depth in [32u32, 33] {
        let mut b = image();
        for n in 0..depth {
            node(
                &mut b,
                (40 + n) as usize * 2048,
                b"D",
                41 + n,
                2048,
                true,
                0,
            );
        }
        b[(40 + depth) as usize * 2048..(40 + depth) as usize * 2048 + 2]
            .copy_from_slice(&65535u16.to_le_bytes());
        install(b);
        let result = disc::Disc::disc_open(7);
        if depth == 32 {
            assert_eq!(result.unwrap().disc_count(), 32);
        } else {
            assert!(matches!(result, Err(-1)));
        }
    }
    for parent in [b"D".as_slice(), b"DD".as_slice()] {
        let mut b = image();
        node(&mut b, 81920, parent, 41, 2048, true, 0);
        let mut name = vec![0xe9; 255];
        name[254] = b'X';
        node(&mut b, 41 * 2048, &name, 0, 0, false, 0);
        install(b);
        let result = disc::Disc::disc_open(7);
        if parent.len() == 1 {
            let image = result.unwrap();
            assert_eq!(image.disc_entry(1).unwrap().disc_path_bytes().len(), 511);
        } else {
            assert!(matches!(result, Err(-1)));
        }
    }
}
