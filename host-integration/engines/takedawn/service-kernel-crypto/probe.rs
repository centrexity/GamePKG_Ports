use crate::cases;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
pub fn crypto_memory(mode: u32) -> Result<memory::Memory, i32> {
    let mut windows = [0u8; 16];
    if gamepkg::local_call(1, &[], &mut windows)? != 16 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        windows[0], windows[1], windows[2], windows[3], windows[4], windows[5], windows[6],
        windows[7],
    ]);
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(0));
    request.extend(u64::to_le_bytes(262144));
    request.extend(u32::to_le_bytes(2));
    // The fixture commits once; each scenario uses a fresh root and map.
    if mode == 0 {
        gamepkg::local_call(2, &request, &mut [])?;
    }
    let mut ram = memory::Memory::new()?;
    ram.attach(0, 262144, window, 0, true)?;
    let mut data = std::vec::Vec::new();
    data.resize(4096, 0u8);
    for index in 0..4096usize {
        data[index] = (index as u32 * 37 + mode * 17 + (index as u32 >> 3)) as u8;
    }
    for page in 0..64u32 {
        ram.write_bytes(page * 4096, &data)?;
    }
    for (slot, ordinal) in [338u32, 339, 340].iter().enumerate() {
        ram.write_u32(0x10000 + slot as u32 * 4, 0x80000000 | *ordinal)?;
    }
    kernel_pool::kernel_prepare(&mut ram, 0x10000, 3)?;
    Ok(ram)
}
pub fn crypto_state(mode: u32, turn: u32) -> registers::Registers {
    let mut state = registers::Registers::new();
    state.eax = 0x12345678 ^ mode;
    state.ebx = 0x87654321 ^ turn;
    state.ecx = mode * 13;
    state.edx = turn * 17;
    state.edi = 0x12344321;
    state.esi = 0x56788765;
    state.ebp = 0x90909090;
    state.esp = 0x18000;
    state.fs_base = 0x8000;
    state.xmm7 = [mode, turn, 0xffffffff, 0x89abcdef];
    state
}
pub fn crypto_arguments(ram: &mut memory::Memory, step: [u32; 8]) -> Result<(), i32> {
    ram.write_u32(0x18000, 0x12345678)?;
    for index in 1..8usize {
        ram.write_u32(0x18000 + index as u32 * 4, step[index])?;
    }
    Ok(())
}
pub fn crypto_witness(
    ram: &memory::Memory,
    state: &registers::Registers,
) -> Result<std::vec::Vec<u8>, i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.resize(9120, 0u8);
    ram.read_bytes(0x10fc0, &mut bytes[..576])?;
    ram.read_bytes(0x12000, &mut bytes[576..640])?;
    ram.read_bytes(0x20000, &mut bytes[640..672])?;
    ram.read_bytes(0x21000, &mut bytes[672..704])?;
    ram.read_bytes(0x22000, &mut bytes[704..8896])?;
    ram.read_bytes(0x25000, &mut bytes[8896..9024])?;
    bytes.truncate(9024);
    bytes.extend(state.encode());
    Ok(bytes)
}
pub fn crypto_case(mode: u32) -> Result<std::vec::Vec<u8>, i32> {
    let mut ram = crypto_memory(mode)?;
    let mut output = std::vec::Vec::new();
    for (turn, step) in cases::crypto_steps(mode)?.iter().enumerate() {
        let mut state = crypto_state(mode, turn as u32);
        crypto_arguments(&mut ram, *step)?;
        kernel_pool::kernel_invoke(&mut state, &mut ram, step[0] - 338)?;
        output.extend(crypto_witness(&ram, &state)?);
    }
    ram.kernel_clear()?;
    gamepkg::local_record_release(ram.call_handle())?;
    Ok(output)
}
