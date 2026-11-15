// Ordinary checked game source: no oracle, device access or native pointers.
use crate::gamepkg;
use crate::layout;
use crate::memory;
fn layout_page(window: u64, offset: u64, operation: u32, permission: u32) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(offset));
    request.extend(u64::to_le_bytes(4096));
    if operation != 4 {
        request.extend(u32::to_le_bytes(permission));
    }
    if gamepkg::local_call(operation, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn layout_commit(window: u64, length: u64) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(0));
    request.extend(u64::to_le_bytes(length));
    request.extend(u32::to_le_bytes(2));
    if gamepkg::local_call(2, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn layout_main_aliases(view: &mut memory::Memory) -> Result<(), i32> {
    for writer in 0..29u32 {
        for offset in [0u32, 1, 4093, 67108860] {
            let value = 0x19370000 + writer * 16 + offset % 16;
            view.write_u32(writer * 67108864 + offset, value)?;
            for reader in 0..29u32 {
                if view.read_u32(reader * 67108864 + offset)? != value {
                    return Err(-3);
                }
            }
        }
    }
    for index in 0..28u32 {
        let at = (index + 1) * 67108864 - 2;
        view.write_bytes(at, &[0x12, 0x34, 0x56, 0x78])?;
        if view.read_u16(67108862)? != 0x3412
            || view.read_u16(0)? != 0x7856
            || view.read_u32(at)? != 0x78563412
        {
            return Err(-3);
        }
    }
    view.write_u16(67108862, 0x9876)?;
    if view.write_u32(0x73fffffe, 0) != Err(-4) || view.read_u16(67108862)? != 0x9876 {
        return Err(-3);
    }
    Ok(())
}
fn layout_secondary(view: &mut memory::Memory) -> Result<(), i32> {
    view.write_u32(0x1000, 0xaaaa5555)?;
    for offset in [0u32, 1, 4093, 4096, 67108860] {
        view.write_u32(0x80000000 + offset, 0x11223344)?;
        if view.read_u32(0xf0000000 + offset)? != 0x11223344 {
            return Err(-3);
        }
        view.write_u32(0xf0000000 + offset, 0x87654321)?;
        if view.read_u32(0x80000000 + offset)? != 0x87654321 {
            return Err(-3);
        }
    }
    if view.read_u32(0x1000)? != 0xaaaa5555 {
        return Err(-3);
    }
    if view.copy_nonoverlapping(0xf0001000, 0x80001000, 4) != Err(-1) {
        return Err(-3);
    }
    view.copy_nonoverlapping(0xf0002000, 0x80001000, 4)?;
    if view.read_u32(0x80002000)? != 0x87654321 {
        return Err(-3);
    }
    for at in [0xff000000u32, 0xff0ffffc] {
        if view.read_u32(at)? != 0 {
            return Err(-3);
        }
        view.write_u32(at, 0x12131415)?;
        if view.read_u32(at)? != 0x12131415 {
            return Err(-3);
        }
    }
    for at in [
        0x74000000u32,
        0x7fffffff,
        0x84000000,
        0xefffffff,
        0xf4000000,
        0xfd000000,
        0xfdffffff,
        0xfe000000,
        0xfe800000,
        0xfeffffff,
        0xff100000,
        0xffffffff,
    ] {
        if view.read_u8(at) != Err(-4) || view.write_u8(at, 0) != Err(-4) {
            return Err(-3);
        }
    }
    if view.write_u32(0xff0ffffe, 0) != Err(-4) || view.read_u16(0xff0ffffe)? != 0x1213 {
        return Err(-3);
    }
    Ok(())
}
fn layout_protections(
    view: &mut memory::Memory,
    main: u64,
    physical: u64,
    flash: u64,
) -> Result<(), i32> {
    view.write_u32(4096, 0x22334455)?;
    view.write_u8(4095, 0x88)?;
    layout_page(main, 4096, 3, 1)?;
    for index in 0..29u32 {
        let base = index * 67108864;
        if view.write_u32(base + 4096, 0) != Err(-4)
            || view.write_u32(base + 4095, 0) != Err(-4)
            || view.read_u8(base + 4095)? != 0x88
            || view.read_u32(base + 4096)? != 0x22334455
        {
            return Err(-3);
        }
    }
    // Cross-mirror transfers use two explicit spans but one shared bank.
    // A denied later segment must preserve both earlier RAM and caller output.
    view.write_u16(67108862, 0xbbaa)?;
    view.write_u16(0, 0xddcc)?;
    layout_page(main, 0, 3, 1)?;
    for mirror in 1..29u32 {
        let address = mirror * 67108864 - 2;
        if view.write_u32(address, 0) != Err(-4)
            || view.read_u16(67108862)? != 0xbbaa
            || view.read_u16(0)? != 0xddcc
        {
            return Err(-3);
        }
    }
    layout_page(main, 0, 3, 0)?;
    for mirror in 1..29u32 {
        let mut copied = [0xa5u8; 4];
        if view.read_bytes(mirror * 67108864 - 2, &mut copied) != Err(-4) || copied != [0xa5u8; 4] {
            return Err(-3);
        }
    }
    layout_page(main, 0, 3, 2)?;
    layout_page(main, 4096, 3, 2)?;
    layout_page(main, 4096, 4, 0)?;
    for index in 0..29u32 {
        if view.read_u8(index * 67108864 + 4096) != Err(-4) {
            return Err(-3);
        }
    }
    layout_page(main, 4096, 2, 2)?;
    if view.read_u32(4096)? != 0 {
        return Err(-3);
    }
    layout_page(physical, 4096, 3, 1)?;
    if view.write_u8(0x80001000, 0) != Err(-4) || view.write_u8(0xf0001000, 0) != Err(-4) {
        return Err(-3);
    }
    layout_page(physical, 4096, 3, 2)?;
    layout_page(flash, 0, 3, 1)?;
    if view.write_u32(0xff000000, 0) != Err(-4) || view.read_u32(0xff000000)? != 0x12131415 {
        return Err(-3);
    }
    layout_page(flash, 0, 3, 2)?;
    Ok(())
}
pub fn layout_verify() -> Result<(), i32> {
    let root = memory::Memory::new()?;
    let plan = layout::Layout::layout_current(&root)?;
    let banks = plan.layout_banks();
    let mut view = plan.layout_view(&root)?;
    for at in [
        0u32, 0x04000000, 0x70000000, 0x80000000, 0xf0000000, 0xff000000,
    ] {
        if view.read_u8(at) != Err(-4) {
            return Err(-3);
        }
    }
    for index in 0..3usize {
        layout_commit(banks[index], if index == 2 { 1048576 } else { 67108864 })?;
    }
    for at in [
        0u32, 0x03fffffc, 0x70000000, 0x80000000, 0x83fffffc, 0xf3fffffc, 0xff000000, 0xff0ffffc,
    ] {
        if view.read_u32(at)? != 0 {
            return Err(-3);
        }
    }
    if view.thread_context() != root.thread_context()
        || view.attach(0x78000000, 4096, banks[0], 0, true) != Err(-1)
    {
        return Err(-3);
    }
    layout_main_aliases(&mut view)?;
    layout_secondary(&mut view)?;
    layout_protections(&mut view, banks[0], banks[1], banks[2])?;
    let other = memory::Memory::new()?;
    if plan.layout_view(&other).is_ok() {
        return Err(-3);
    }
    gamepkg::local_record_release(other.call_handle())?;
    let mut rebuilt = plan.layout_view(&root)?;
    if rebuilt.read_u32(0x80002000)? != view.read_u32(0xf0002000)? {
        return Err(-3);
    }
    rebuilt.write_u32(0x70003000, 0xaabbccdd)?;
    if view.read_u32(0x3000)? != 0xaabbccdd {
        return Err(-3);
    }
    gamepkg::local_record_release(root.call_handle())?;
    if plan.layout_view(&root).is_ok() || layout::Layout::layout_current(&root).is_ok() {
        return Err(-3);
    }
    Ok(())
}
