// Asset-free checks of the actual migrated guest-FS epilogue, never host FS.
use crate::dispatch;
use crate::memory;
use crate::registers;
pub fn thread_memory_verify(ram: &mut memory::Memory, window: u64) -> Result<(), i32> {
    let mut context = registers::Registers::new();
    context.esp = 0x20000;
    context.seh_ebp = 0x20100;
    ram.write_u32(0x200f0, 0x12345678)?;
    for (address, error) in [(0xfffffffeu32, -1i32), (0x04000000, -4), (0x7ffe0000, -4)] {
        context.fs_base = address;
        if dispatch::invoke(&mut context, ram, 0x2447bf) != Err(error) {
            return Err(-3);
        }
    }
    let mut restricted = ram.call_view();
    restricted.attach(0x10000, 4096, window, 0x10000, false)?;
    restricted.attach(0x1fe00, 1024, window, 0x1fe00, true)?;
    context.fs_base = 0x10001;
    let before = ram.read_u32(0x10001)?;
    if dispatch::invoke(&mut context, &mut restricted, 0x2447bf) != Err(-4)
        || ram.read_u32(0x10001)? != before
    {
        return Err(-3);
    }
    Ok(())
}
