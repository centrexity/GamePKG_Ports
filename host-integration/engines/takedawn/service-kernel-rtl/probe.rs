use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
fn rtl_probe_need(stage: i32, ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-stage)
    }
}
fn rtl_probe_emit(out: &mut std::vec::Vec<u8>, value: u32) {
    out.extend(u32::to_le_bytes(value));
}
pub fn rtl_probe_frame(
    ram: &mut memory::Memory,
    args: &[u32],
) -> Result<registers::Registers, i32> {
    let mut state = registers::Registers::new();
    state.esp = 0x2000;
    state.eax = 0xdeadbeef;
    state.edx = 0x11223344;
    state.ebx = 0x89abcdef;
    state.ecx = 0x76543210;
    state.eflags = 0x202;
    state.fs_base = 0x7000;
    state.mm3 = [0x12345678, 0xabcdef01];
    state.xmm7 = [1, 2, 3, 4];
    ram.write_u32(state.esp, 0x12345678)?;
    for (index, value) in args.iter().enumerate() {
        ram.write_u32(state.esp + 4 + index as u32 * 4, *value)?;
    }
    Ok(state)
}
pub fn rtl_probe_call(ram: &mut memory::Memory, slot: u32, args: &[u32]) -> Result<u32, i32> {
    let mut state = rtl_probe_frame(ram, args)?;
    let before = state.encode();
    kernel_pool::kernel_invoke(&mut state, ram, slot)?;
    let mut expected = registers::Registers::new();
    expected.decode(&before)?;
    expected.eax = state.eax;
    expected.esp += 4 + args.len() as u32 * 4;
    rtl_probe_need(
        501,
        state.encode() == expected.encode() && ram.kernel_caller() == 0x12345678,
    )?;
    Ok(state.eax)
}
pub fn rtl_probe_error(
    ram: &mut memory::Memory,
    slot: u32,
    args: &[u32],
    code: i32,
) -> Result<(), i32> {
    let mut state = rtl_probe_frame(ram, args)?;
    let before = state.encode();
    let outcome = kernel_pool::kernel_invoke(&mut state, ram, slot);
    rtl_probe_need(502, outcome == Err(code) && state.encode() == before)
}
fn rtl_probe_header(
    ram: &mut memory::Memory,
    address: u32,
    len: u16,
    buffer: u32,
) -> Result<(), i32> {
    ram.write_u16(address, len)?;
    ram.write_u16(address + 2, 0)?;
    ram.write_u32(address + 4, buffer)
}
pub fn rtl_probe_vectors(ram: &mut memory::Memory) -> Result<std::vec::Vec<u8>, i32> {
    let mut out = std::vec::Vec::new();
    let lengths = [
        0u32, 1, 2, 3, 4, 5, 7, 8, 9, 15, 16, 31, 32, 63, 64, 255, 256, 257, 511,
    ];
    for length in lengths {
        for mode in 0..4u32 {
            let pattern = 0x91abcdefu32;
            for index in 0..128u32 {
                ram.write_u32(0x14001 + index * 4, pattern)?;
            }
            if mode == 1 {
                ram.write_u32(0x14001, pattern ^ 1)?;
            }
            if mode == 2 && length >= 4 {
                ram.write_u32(0x14001 + (length & !3u32) - 4, pattern ^ 1)?;
            }
            let value = rtl_probe_call(
                ram,
                0,
                &[if mode == 3 { 0 } else { 0x14001 }, length, pattern],
            )?;
            rtl_probe_emit(&mut out, value);
        }
    }
    for length in [0u32, 1, 2, 3, 7, 31, 255, 256, 257, 512] {
        for flags in [0u32, 1, 0x100, 0x101, 0xffffffff] {
            for mode in 0..5u32 {
                let mut a = std::vec::Vec::new();
                let mut b = std::vec::Vec::new();
                a.resize(length as usize, 0u8);
                b.resize(length as usize, 0u8);
                for index in 0..length {
                    a[index as usize] = (65 + index % 26) as u8;
                    b[index as usize] = (65 + index % 26) as u8;
                }
                if length != 0 && mode == 1 {
                    for value in &mut b {
                        *value += 32;
                    }
                }
                if length != 0 && mode == 2 {
                    b[length as usize - 1] ^= 0x80;
                }
                if length != 0 && mode == 3 {
                    a[length as usize / 2] = 0;
                    b[length as usize / 2] = 0;
                    if length - 1 > length / 2 {
                        b[length as usize - 1] ^= 0x80;
                    }
                }
                if length != 0 && mode == 4 {
                    a[0] = 0xc0;
                    b[0] = 0xe0;
                }
                ram.write_bytes(0x14000, &a)?;
                ram.write_bytes(0x16000, &b)?;
                rtl_probe_header(ram, 0x3000, length as u16, 0x14000)?;
                rtl_probe_header(ram, 0x3010, length as u16, 0x16000)?;
                let value = rtl_probe_call(ram, 1, &[0x3000, 0x3010, flags])?;
                rtl_probe_emit(&mut out, value);
            }
        }
    }
    for wide in [false, true] {
        for length in [0u32, 1, 2, 255, 256, 257, 32767, 65533, 65534, 65536] {
            for mode in 0..3u32 {
                let mut bytes = std::vec::Vec::new();
                let width = if wide { 2usize } else { 1usize };
                bytes.resize((length as usize + 1) * width, 0u8);
                for index in 0..length {
                    bytes[index as usize * width] = (65 + index % 26) as u8;
                }
                ram.write_bytes(0x18000, &bytes)?;
                ram.write_bytes(0x12ffc, &[0xccu8; 16])?;
                let value = rtl_probe_call(
                    ram,
                    if wide { 3 } else { 2 },
                    &[
                        if mode == 2 { 0 } else { 0x13000 },
                        if mode == 1 { 0 } else { 0x18000 },
                    ],
                )?;
                rtl_probe_emit(&mut out, value);
                let mut header = [0u8; 16];
                ram.read_bytes(0x12ffc, &mut header)?;
                out.extend(&header);
            }
        }
    }
    let statuses = [
        0u32, 0x104, 0xc0000034, 0xc000003a, 0xc0000022, 0xc0000008, 0xc0000017, 0xc000000d, 0x103,
        0x102, 0x80000005, 0x80000006, 0xc0000011, 0xc0000023, 0xc0000035, 0xc00000bb, 0xffffffff,
        0xc000009a, 0xc0000001, 1, 0x80000000, 0x7fffffff, 0x101, 0x105, 0xc0000020, 0xc0000033,
        0xc0000012, 0x80000007,
    ];
    for index in 0..92u32 {
        let status = if index < 28 {
            statuses[index as usize]
        } else {
            0x91234567u32.wrapping_mul(index - 27)
        };
        let value = rtl_probe_call(ram, 4, &[status])?;
        rtl_probe_emit(&mut out, value);
    }
    rtl_probe_need(503, out.len() == 2872)?;
    Ok(out)
}
fn rtl_probe_protect(window: u64, offset: u64, mode: u32) -> Result<(), i32> {
    gamepkg::local_call(
        3,
        &[
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(offset),
            &u64::to_le_bytes(4096),
            &u32::to_le_bytes(mode),
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
pub fn rtl_probe_controls(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    let mut partial = ram.call_view();
    partial.attach(0x100000, 17, window, 0x3eff0, true)?;
    let mut chunk = [0u8; 256];
    rtl_probe_need(
        518,
        partial.memory_read_chunk(0x100000, 4, &mut chunk)? == 16,
    )?;
    rtl_probe_need(
        519,
        partial.memory_read_chunk(0x100010, 4, &mut chunk) == Err(-4),
    )?;
    partial.attach(0x110000, 23, window, 0x30000, true)?;
    rtl_probe_need(
        520,
        partial.memory_read_chunk(0x110000, 4, &mut chunk)? == 20,
    )?;
    for slot in 0..5u32 {
        let bytes = kernel_pool::kernel_supported([269u32, 279, 289, 290, 301][slot as usize])?;
        rtl_probe_need(504, bytes == [12u32, 12, 8, 8, 4][slot as usize])?;
    }
    rtl_probe_need(
        505,
        rtl_probe_call(ram, 0, &[0, u32::MAX, 1])? == 0
            && rtl_probe_call(ram, 0, &[0xffffffff, 3, 1])? == 0,
    )?;
    rtl_probe_need(
        506,
        rtl_probe_call(ram, 1, &[0, 0xffffffff, 1])? == 0
            && rtl_probe_call(ram, 1, &[0xffffffff, 0, 1])? == 0,
    )?;
    for slot in [2u32, 3] {
        rtl_probe_need(507, rtl_probe_call(ram, slot, &[0, 0xffffffff])? == 0)?;
    }
    for unit in [0u32, 3, 5, u32::MAX] {
        rtl_probe_need(
            508,
            ram.memory_read_chunk(0x3000, unit, &mut [0u8; 8]) == Err(-1),
        )?;
    }
    rtl_probe_need(
        509,
        ram.memory_read_chunk(0x3000, 4, &mut [0u8; 3]) == Err(-1)
            && ram.memory_read_chunk(0x3000, 1, &mut []) == Err(-1),
    )?;
    rtl_probe_header(ram, 0x3000, 0, 0)?;
    rtl_probe_header(ram, 0x3010, 0, 0x16000)?;
    rtl_probe_need(522, rtl_probe_call(ram, 1, &[0x3000, 0x3010, 1])? == 0)?;
    rtl_probe_header(ram, 0x3000, 0, 0xffffffff)?;
    rtl_probe_header(ram, 0x3010, 0, 0xfffffffe)?;
    rtl_probe_need(510, rtl_probe_call(ram, 1, &[0x3000, 0x3010, 1])? == 1)?;
    rtl_probe_header(ram, 0x3000, 1, 0xffffffff)?;
    rtl_probe_header(ram, 0x3010, 2, 0xfffffffe)?;
    rtl_probe_need(511, rtl_probe_call(ram, 1, &[0x3000, 0x3010, 1])? == 0)?;
    rtl_probe_header(ram, 0x3000, 1, 0)?;
    rtl_probe_error(ram, 1, &[0x3000, 0x40000, 0], -4)?;
    ram.write_u8(0x3efff, 0)?;
    ram.write_u16(0x3effc, 0)?;
    ram.write_u16(0x3effe, 0)?;
    rtl_probe_protect(window, 0x3f000, 0)?;
    rtl_probe_need(
        512,
        rtl_probe_call(ram, 2, &[0x3000, 0x3efff])? == 0
            && ram.read_u16(0x3000)? == 0
            && ram.read_u16(0x3002)? == 1,
    )?;
    rtl_probe_need(
        513,
        rtl_probe_call(ram, 3, &[0x3000, 0x3effe])? == 0 && ram.read_u16(0x3002)? == 2,
    )?;
    rtl_probe_error(ram, 3, &[0x3000, 0x3efff], -4)?;
    rtl_probe_header(ram, 0x3000, 512, 0x3efff)?;
    rtl_probe_header(ram, 0x3010, 512, 0x14000)?;
    ram.write_u8(0x14000, 0)?;
    rtl_probe_need(514, rtl_probe_call(ram, 1, &[0x3000, 0x3010, 0])? == 1)?;
    ram.write_u8(0x3efff, 65)?;
    ram.write_u8(0x14000, 66)?;
    rtl_probe_need(515, rtl_probe_call(ram, 1, &[0x3000, 0x3010, 0])? == 0)?;
    ram.write_u8(0x14000, 65)?;
    rtl_probe_error(ram, 1, &[0x3000, 0x3010, 0], -4)?;
    ram.write_u32(0x3effc, 1)?;
    rtl_probe_need(516, rtl_probe_call(ram, 0, &[0x3effc, 256, 2])? == 0)?;
    rtl_probe_error(ram, 0, &[0x3effc, 256, 1], -4)?;
    rtl_probe_protect(window, 0x3f000, 2)?;
    let mut unterminated = std::vec::Vec::new();
    unterminated.resize(65536, 65u8);
    ram.write_bytes(0x2f000, &unterminated)?;
    ram.write_bytes(0x3000, &[0xccu8; 8])?;
    rtl_probe_protect(window, 0x3f000, 0)?;
    for slot in [2u32, 3] {
        rtl_probe_error(ram, slot, &[0x3000, 0x2f000], -4)?;
    }
    let mut untouched = [0u8; 8];
    ram.read_bytes(0x3000, &mut untouched)?;
    rtl_probe_need(521, untouched == [0xcc; 8])?;
    rtl_probe_protect(window, 0x3f000, 2)?;
    for destination in [0x3effeu32, 0x3effc] {
        ram.write_bytes(destination, &[0xccu8; 8])?;
        ram.write_bytes(0x18000, &[65u8, 66, 67, 0])?;
        rtl_probe_protect(window, 0x3f000, 0)?;
        rtl_probe_error(ram, 2, &[destination, 0x18000], -4)?;
        rtl_probe_protect(window, 0x3f000, 2)?;
        let mut bytes = [0u8; 8];
        ram.read_bytes(destination, &mut bytes)?;
        let mut expected = [0xccu8; 8];
        expected[0] = 3;
        expected[1] = 0;
        if destination == 0x3effc {
            expected[2] = 4;
            expected[3] = 0;
        }
        rtl_probe_need(517, bytes == expected)?;
    }
    rtl_probe_protect(window, 0x3f000, 1)?;
    rtl_probe_error(ram, 2, &[0x3f000, 0x18000], -4)?;
    rtl_probe_protect(window, 0x3f000, 2)?;
    for slot in [0u32, 1, 2, 3, 4] {
        let mut state = rtl_probe_frame(ram, &[1, 2, 3])?;
        state.esp = 0xfffffff0;
        let before = state.encode();
        rtl_probe_need(
            518,
            kernel_pool::kernel_invoke(&mut state, ram, slot).is_err() && state.encode() == before,
        )?;
    }
    Ok(())
}
