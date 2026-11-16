// Synthetic original-defined input domain, copied into the C oracle unchanged.
pub fn dispatcher_case(mode: u32) -> [u32; 8] {
    let ordinal = [44u32, 107, 109][mode as usize % 3];
    let group = mode / 3;
    let size = if ordinal == 109 { 44u32 } else { 32u32 };
    let pointer = if ordinal == 44 {
        [
            0u32, 0x1000, 0x11001, 0x11ffc, 0x18000, 0x18004, 0x3ffffff, 0x80000001, 0x83ffffff,
            0x4000000, 0xf0000040, 0xff000020,
        ][group as usize % 12]
    } else {
        [
            0u32,
            1,
            4095,
            4096,
            0x11001,
            0x11ff0,
            0x18000,
            0x18004,
            0x4000000 - size,
            0x4000001 - size,
            0x80000001,
            0x84000000 - size,
            0x84000001 - size,
            0x4000000,
            0xf0000000,
            0xfffffff0,
        ][group as usize % 16]
    };
    let a = mode.wrapping_mul(0xabcde1).wrapping_add(0x12345678);
    let b = mode.wrapping_mul(0x12345).wrapping_add(0xdeadbeef);
    if ordinal == 44 {
        [ordinal, a, pointer, 0, 0, 0, 0, 0]
    } else {
        [
            ordinal,
            pointer,
            a,
            b,
            group ^ 0x87654321,
            0xff00aa55,
            0x13572468,
            0xffffffff,
        ]
    }
}
pub fn dispatcher_state(mode: u32) -> crate::registers::Registers {
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
