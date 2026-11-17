use crate::gamepkg;
use crate::kernel_pool;
use crate::kernel_startup;
use crate::memory;
use crate::registers;
use crate::startup_file;
use crate::xbe;

fn rom_startup_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-901)
    }
}

fn rom_startup_call(
    routed: &mut startup_file::StartupFiles,
    ram: &mut memory::Memory,
    marker: u32,
    stack_va: u32,
    args: &[u32],
) -> Result<u32, i32> {
    let mut cpu = registers::Registers::new();
    cpu.esp = stack_va;
    cpu.ecx = 0x44332211;
    cpu.eax = 0xfeedface;
    ram.write_u32(stack_va, 0x87654321)?;
    for (lane, value) in args.iter().enumerate() {
        ram.write_u32(stack_va + 4 + lane as u32 * 4, *value)?;
    }
    let mut expected = registers::Registers::new();
    expected.decode(&cpu.encode())?;
    routed.startup_files_call(ram, &mut cpu, marker)?;
    expected.esp += 4 + args.len() as u32 * 4;
    expected.eax = cpu.eax;
    rom_startup_need(cpu.encode() == expected.encode() && ram.kernel_caller() == 0x87654321)?;
    Ok(cpu.eax)
}

pub fn rom_startup_calls(
    ram: &mut memory::Memory,
    owner: &kernel_startup::Startup,
    image: &xbe::Image,
    with_event: bool,
) -> Result<[u64; 3], i32> {
    let mut routed = startup_file::StartupFiles::startup_files_new(owner, image, ram)?;
    let base = ram.read_u32(0x4008)?;
    let stack_va = base.checked_add(0x2000).ok_or(-1)?;
    let attrib_va = stack_va + 64;
    let name_va = stack_va + 80;
    let path_va = stack_va + 96;
    let token_va = stack_va + 128;
    let status_va = stack_va + 144;
    let output_va = stack_va + 160;
    let path = b"D:\\default.xbe";
    ram.write_u32(attrib_va, 0)?;
    ram.write_u32(attrib_va + 4, name_va)?;
    ram.write_u32(attrib_va + 8, 0x40)?;
    ram.write_u16(name_va, path.len() as u16)?;
    ram.write_u16(name_va + 2, path.len() as u16)?;
    ram.write_u32(name_va + 4, path_va)?;
    ram.write_bytes(path_va, path)?;
    let open_marker = ram.read_u32(image.image_thunk() + 40)?;
    let read_marker = ram.read_u32(image.image_thunk() + 44)?;
    let close_marker = ram.read_u32(image.image_thunk() + 48)?;
    rom_startup_need(
        [open_marker, read_marker, close_marker] == [0xfe000028, 0xfe00002c, 0xfe000030],
    )?;

    let mut rejected = registers::Registers::new();
    let before = rejected.encode();
    rom_startup_need(
        routed.startup_files_call(ram, &mut rejected, open_marker + 1) == Err(-5)
            && rejected.encode() == before,
    )?;
    rom_startup_need(kernel_pool::kernel_ordinal(ram, 10) == Err(-5))?;
    rom_startup_need(
        rom_startup_call(
            &mut routed,
            ram,
            open_marker,
            stack_va,
            &[token_va, 0x80100080, attrib_va, status_va, 1, 0x60],
        )? == 0,
    )?;
    let token = ram.read_u32(token_va)?;
    rom_startup_need(token != 0 && ram.read_u64(status_va)? == 0x100000000)?;
    rom_startup_need(
        rom_startup_call(
            &mut routed,
            ram,
            read_marker,
            stack_va,
            &[token, 0, 0, 0, status_va, output_va, 4, 0],
        )? == 0,
    )?;
    let mut prefix = [0u8; 4];
    ram.read_bytes(output_va, &mut prefix)?;
    rom_startup_need(&prefix == b"XBEH" && ram.read_u64(status_va)? == 0x400000000)?;
    rom_startup_need(rom_startup_call(&mut routed, ram, close_marker, stack_va, &[token])? == 0)?;
    if with_event {
        let (table, event_domain) = owner.startup_objects(ram)?;
        let mut incoming = event_domain.events_begin(ram, &table)?;
        incoming.event_create(ram, 1, 0)?;
        let native_event = incoming.event_creation_id();
        let event_token = incoming.event_publish(ram, stack_va + 192)?;
        incoming.event_creation_close(ram)?;
        let create_marker = ram.read_u32(image.image_thunk() + 52)?;
        rom_startup_need(create_marker == 0xfe000034)?;
        rom_startup_need(
            rom_startup_call(
                &mut routed,
                ram,
                create_marker,
                stack_va,
                &[
                    token_va, 0x80100080, attrib_va, status_va, 0, 0x80, 1, 1, 0x40,
                ],
            )? == 0,
        )?;
        let token = ram.read_u32(token_va)?;
        rom_startup_need(
            rom_startup_call(
                &mut routed,
                ram,
                read_marker,
                stack_va,
                &[token, event_token, 0, 0, status_va, output_va, 4, 0],
            )? == 0x103,
        )?;
        ram.read_bytes(output_va, &mut prefix)?;
        rom_startup_need(&prefix == b"XBEH" && ram.read_u64(status_va)? == 0x400000000)?;
        rom_startup_need(gamepkg::local_event_wait(native_event, 0) == Ok(()))?;
        rom_startup_need(gamepkg::local_event_wait(native_event, 0) == Err(-2))?;
        rom_startup_need(
            rom_startup_call(&mut routed, ram, close_marker, stack_va, &[token])? == 0,
        )?;
        rom_startup_need(
            rom_startup_call(
                &mut routed,
                ram,
                create_marker,
                stack_va,
                &[
                    token_va, 0x80100080, attrib_va, status_va, 0, 0x80, 1, 1, 0x40,
                ],
            )? == 0,
        )?;
        let file_wait_token = ram.read_u32(token_va)?;
        let file_wait_marker = ram.read_u32(image.image_thunk() + 56)?;
        let file_wait_ex_marker = ram.read_u32(image.image_thunk() + 60)?;
        rom_startup_need([file_wait_marker, file_wait_ex_marker] == [0xfe000038, 0xfe00003c])?;
        rom_startup_need(
            rom_startup_call(
                &mut routed,
                ram,
                read_marker,
                stack_va,
                &[file_wait_token, 0, 0, 0, status_va, output_va, 4, 0],
            )? == 0x103,
        )?;
        ram.read_bytes(output_va, &mut prefix)?;
        rom_startup_need(&prefix == b"XBEH" && ram.read_u64(status_va)? == 0x400000000)?;
        rom_startup_need(gamepkg::local_event_wait(native_event, 0) == Err(-2))?;
        rom_startup_need(
            rom_startup_call(
                &mut routed,
                ram,
                file_wait_marker,
                stack_va,
                &[file_wait_token, 0, 0],
            )? == 0,
        )?;
        ram.write_u64(stack_va + 200, 0)?;
        rom_startup_need(
            rom_startup_call(
                &mut routed,
                ram,
                file_wait_ex_marker,
                stack_va,
                &[file_wait_token, 1, 0, stack_va + 200],
            )? == 0,
        )?;
        rom_startup_need(
            rom_startup_call(&mut routed, ram, close_marker, stack_va, &[file_wait_token])? == 0,
        )?;
        event_domain.events_remove(ram, &table, event_token)?;
    }
    routed.startup_files_close(ram)?;
    Ok([1, 4, 1 + 2 * u64::from(with_event)])
}
