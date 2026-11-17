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
    let path_va = stack_va + 256;
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
    let startup_storage_grant = owner.startup_private_save(ram, image)?;
    if startup_storage_grant != 0 {
        let device_marker = ram.read_u32(image.image_thunk() + 64)?;
        let ensure_marker = ram.read_u32(image.image_thunk() + 52)?;
        rom_startup_need(device_marker == 0xfe000040)?;
        rom_startup_need(kernel_pool::kernel_ordinal(ram, 16) == Err(-5))?;
        let save_frame = [
            stack_va, attrib_va, name_va, path_va, token_va, status_va, output_va,
        ];
        save_startup_calls(
            &mut routed,
            ram,
            save_frame,
            [
                open_marker,
                ensure_marker,
                device_marker,
                close_marker,
                read_marker,
            ],
            token,
            startup_storage_grant,
        )?;
        ram.write_u16(name_va, path.len() as u16)?;
        ram.write_u16(name_va + 2, path.len() as u16)?;
        ram.write_bytes(path_va, path)?;
    }
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

fn save_startup_name(
    ram: &mut memory::Memory,
    string_descriptor: u32,
    path_address: u32,
    disk_name: &[u8],
) -> Result<(), i32> {
    ram.write_u16(string_descriptor, disk_name.len() as u16)?;
    ram.write_u16(string_descriptor + 2, disk_name.len() as u16)?;
    ram.write_bytes(path_address, disk_name)
}

fn save_startup_calls(
    routed: &mut startup_file::StartupFiles,
    ram: &mut memory::Memory,
    frame: [u32; 7],
    slots: [u32; 5],
    optical_token: u32,
    storage_grant: u64,
) -> Result<(), i32> {
    let [call_stack, attributes_address, string_descriptor, pathname_address, handle_address, iosb_address, payload_address] =
        frame;
    let [observe_marker, ensure_marker, volume_marker, release_marker, optical_read_marker] = slots;
    // A rejected root or a cross-domain token may not change the guest result.
    save_startup_name(
        ram,
        string_descriptor,
        pathname_address,
        b"\\Device\\Harddisk0\\partition2\\",
    )?;
    let observe = [
        handle_address,
        0x100001,
        attributes_address,
        iosb_address,
        3,
        0x800021,
    ];
    rom_startup_need(
        rom_startup_call(routed, ram, observe_marker, call_stack, &observe) == Err(-5),
    )?;
    save_startup_name(
        ram,
        string_descriptor,
        pathname_address,
        b"\\Device\\Harddisk0\\partition1\\",
    )?;
    rom_startup_need(rom_startup_call(routed, ram, observe_marker, call_stack, &observe)? == 0)?;
    let private_token = ram.read_u32(handle_address)?;
    rom_startup_need(private_token != optical_token && ram.read_u64(iosb_address)? == 0x100000000)?;
    let cross_volume = [optical_token, iosb_address, payload_address, 24, 3];
    rom_startup_need(
        rom_startup_call(routed, ram, volume_marker, call_stack, &cross_volume) == Err(-4),
    )?;
    let volume = [private_token, iosb_address, payload_address, 24, 3];
    rom_startup_need(rom_startup_call(routed, ram, volume_marker, call_stack, &volume)? == 0)?;
    rom_startup_need(
        ram.read_u64(payload_address)? == 4
            && ram.read_u64(payload_address + 8)? == 4
            && ram.read_u32(payload_address + 16)? == 32
            && ram.read_u32(payload_address + 20)? == 512
            && ram.read_u64(iosb_address)? == 24u64 << 32,
    )?;
    let cross_read = [private_token, 0, 0, 0, iosb_address, payload_address, 4, 0];
    rom_startup_need(
        rom_startup_call(routed, ram, optical_read_marker, call_stack, &cross_read) == Err(-4),
    )?;
    rom_startup_need(
        rom_startup_call(routed, ram, release_marker, call_stack, &[private_token])? == 0,
    )?;
    rom_startup_need(rom_startup_call(routed, ram, volume_marker, call_stack, &volume) == Err(-4))?;

    let ensure = [
        handle_address,
        0x100001,
        attributes_address,
        iosb_address,
        0,
        128,
        3,
        3,
        0x4021,
    ];
    for disk_name in [
        b"\\Device\\Harddisk0\\partition1\\TDATA".as_slice(),
        b"\\Device\\Harddisk0\\partition1\\TDATA\\4541005b",
        b"\\Device\\Harddisk0\\partition1\\UDATA",
        b"\\Device\\Harddisk0\\partition1\\UDATA\\4541005b",
    ] {
        save_startup_name(ram, string_descriptor, pathname_address, disk_name)?;
        for creation_state in [2u64, 1] {
            rom_startup_need(
                rom_startup_call(routed, ram, ensure_marker, call_stack, &ensure)? == 0,
            )?;
            let token = ram.read_u32(handle_address)?;
            rom_startup_need(
                token != optical_token && ram.read_u64(iosb_address)? == creation_state << 32,
            )?;
            rom_startup_need(
                rom_startup_call(routed, ram, release_marker, call_stack, &[token])? == 0,
            )?;
        }
    }
    rom_startup_need(gamepkg::save_volume(storage_grant) == Ok([65536, 65536]))?;
    gamepkg::diagnostic(1, 0x544b4600, &[4, 4, 16384])?;
    Ok(())
}
