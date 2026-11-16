// Maintainer-only codec checks; only registers.rs is admitted game source.
#[path = "registers.rs"]
mod registers;
fn sample(index: u32) -> Vec<u8> {
    let mut seed = u64::from(index) ^ 0x6427d8fe351ab902;
    let mut bytes = Vec::new();
    for _ in 0..316 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        bytes.push(seed as u8);
    }
    bytes[40..44].copy_from_slice(&(index % 8).to_le_bytes());
    if index % 2 == 0 {
        let edges = [
            0u64,
            0x8000000000000000,
            1,
            0x8000000000000001,
            0x7ff0000000000000,
            0xfff0000000000000,
            0x7ff8000000000001,
            0x7ff0000000000001,
            0xfff8fedcba987654,
        ];
        for lane in 0..8usize {
            bytes[48 + lane * 8..56 + lane * 8]
                .copy_from_slice(&edges[(index as usize / 2 + lane) % edges.len()].to_le_bytes());
        }
    }
    bytes
}
#[test]
fn private_v5_preserves_every_bit_including_flags_comparison_mmx_and_floating_nan_payloads() {
    let mut state = registers::Registers::new();
    assert!(state.valid());
    assert_eq!(state.eflags, 0x202);
    assert_eq!(state.fp_cmp, 0);
    for index in 0..65536u32 {
        let bytes = sample(index);
        state.decode(&bytes).unwrap();
        assert_eq!(state.encode(), bytes);
        assert_eq!(
            state.fs_base,
            u32::from_le_bytes(bytes[240..244].try_into().unwrap())
        );
        assert_eq!(
            state.eflags,
            u32::from_le_bytes(bytes[244..248].try_into().unwrap())
        );
        assert_eq!(
            state.fp_cmp,
            i32::from_le_bytes(bytes[248..252].try_into().unwrap())
        );
        assert_eq!(
            state.mm0[0],
            u32::from_le_bytes(bytes[252..256].try_into().unwrap())
        );
        assert_eq!(
            state.mm7[1],
            u32::from_le_bytes(bytes[312..316].try_into().unwrap())
        );
    }
}
#[test]
fn wrong_sizes_including_v4_and_invalid_fp_top_reject_before_any_mutation() {
    let mut state = registers::Registers::new();
    state.decode(&sample(77)).unwrap();
    let before = state.encode();
    for size in [0usize, 239, 240, 243, 244, 245, 251, 252, 315, 317, 1024] {
        assert_eq!(state.decode(&vec![0; size]), Err(-1));
        assert_eq!(state.encode(), before);
        assert_eq!(state.irq_restore(&vec![0; size]), Err(-1));
        assert_eq!(state.encode(), before);
    }
    for top in [-1i32, 8, i32::MAX] {
        let mut bad = before.clone();
        bad[40..44].copy_from_slice(&top.to_le_bytes());
        assert_eq!(state.decode(&bad), Err(-1));
        assert_eq!(state.encode(), before);
        assert_eq!(state.irq_restore(&bad), Err(-1));
        assert_eq!(state.encode(), before);
    }
}
#[test]
fn irq_prepare_and_restore_preserve_original_holder_fs_semantics() {
    for index in 0..4096u32 {
        let original = sample(index);
        let modified = sample(index + 0x10000);
        let mut state = registers::Registers::new();
        state.decode(&original).unwrap();
        state.irq_prepare(0x12340000 + index);
        assert_eq!(
            [state.eax, state.ecx, state.edx, state.ebx, state.esi, state.edi],
            [0; 6]
        );
        assert_eq!(state.esp, 0x12340000 + index);
        let mut prepared = original.clone();
        for at in [0usize, 4, 8, 12, 16, 20] {
            prepared[at..at + 4].fill(0);
        }
        prepared[28..32].copy_from_slice(&(0x12340000 + index).to_le_bytes());
        assert_eq!(state.encode(), prepared);
        state.decode(&modified).unwrap();
        let holder_fs = state.fs_base;
        state.irq_restore(&original).unwrap();
        let mut expected = original;
        expected[240..244].copy_from_slice(&holder_fs.to_le_bytes());
        assert_eq!(state.encode(), expected);
    }
}
