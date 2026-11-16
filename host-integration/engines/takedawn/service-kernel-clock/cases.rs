// Original defined-domain clock inputs; no native handles or clock injection.
pub fn clock_case(mode: u32) -> [u64; 7] {
    let group = mode / 4;
    let ordinal = 125 + mode % 4;
    let skew = [
        0u64,
        1,
        99,
        100,
        999999999,
        1000000000,
        1000000001,
        100000000000000,
    ][group as usize % 8];
    let frequency = [
        1u64, 3, 10000000, 1000000000, 3579545, 100000000, 733333333, 1000000,
    ][(group / 8) as usize % 8];
    let counter = [0u64, 1, 0x123456789, 9000000000000000000][(group / 3) as usize % 4];
    let uptime = [0u64, 1, 0x100000001, u64::MAX / 10000, u64::MAX][(group / 5) as usize % 5];
    let utc = [0u64, 1, 116444736000000000, 133000000000000000][(group / 7) as usize % 4];
    let pointer = if group % 7 == 0 {
        0
    } else {
        [0x11001u64, 0x11ffc, 0x18004][group as usize % 3]
    };
    [
        u64::from(ordinal),
        counter,
        frequency,
        uptime,
        utc,
        skew,
        pointer,
    ]
}
pub fn clock_initial(mode: u32) -> crate::registers::Registers {
    let mut state = crate::registers::Registers::new();
    state.eax = 0x12345678 ^ mode;
    state.edx = 0x87654321 ^ mode;
    state.ebx = 0x88776655;
    state.ecx = 0x13572468;
    state.ebp = 0x12344321;
    state.esi = 0xff00aa55;
    state.edi = 0xabcdef01;
    state.esp = 0x18000;
    state.fs_base = 0x8000;
    state.fp_top = (mode & 7) as i32;
    state.fp_stack[1] = f64::from_bits(0x7ff8000012345678);
    state.xmm3 = [mode, 0x80000000, 0xffffffff, 0x7fc12345];
    state
}
