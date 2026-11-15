// Maintainer-only four-target codec checks; only registers.rs is game source.
#[path = "registers.rs"]
mod registers;
#[test]
fn guest_fs_is_a_fixed_little_endian_word_appended_to_the_private_v4_record() {
    let mut state = registers::Registers::new();
    assert!(state.valid());
    state.eax = 0x12345678;
    state.fp_control_word = 0xffff;
    state.xmm7 = [1, 2, 3, 4];
    let prefix = state.encode()[..240].to_vec();
    for index in 0..65536u32 {
        state.fs_base = (index << 16) | (!index & 65535);
        let bytes = state.encode();
        assert_eq!(bytes.len(), 244);
        assert_eq!(&bytes[..240], prefix);
        assert_eq!(&bytes[240..], state.fs_base.to_le_bytes());
        let mut restored = registers::Registers::new();
        restored.decode(&bytes).unwrap();
        assert_eq!(restored.fs_base, state.fs_base);
        assert_eq!(restored.encode(), bytes);
    }
}
#[test]
fn wrong_record_sizes_and_invalid_fp_state_are_rejected_before_mutation() {
    let mut state = registers::Registers::new();
    state.fs_base = u32::MAX;
    state.eax = 91;
    let original = state.encode();
    for size in [0usize, 239, 240, 243, 245, 1024] {
        assert_eq!(state.decode(&vec![0; size]), Err(-1));
        assert_eq!(state.encode(), original);
    }
    for top in [-1i32, 8, i32::MAX] {
        let mut bad = original.clone();
        bad[40..44].copy_from_slice(&top.to_le_bytes());
        assert_eq!(state.decode(&bad), Err(-1));
        assert_eq!(state.encode(), original);
    }
}
