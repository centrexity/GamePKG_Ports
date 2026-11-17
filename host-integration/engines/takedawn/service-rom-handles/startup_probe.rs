use crate::gamepkg;
use crate::kernel_pool;
use crate::kernel_regular;
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
    regular_session: Option<&mut kernel_regular::SaveSession>,
) -> Result<[u64; 3], i32> {
    let regular_enabled = regular_session.is_some();
    let mut routed = match regular_session {
        Some(session) => {
            rom_startup_need(
                startup_file::StartupFiles::startup_files_new(owner, image, ram).err() == Some(-4),
            )?;
            startup_file::StartupFiles::startup_files_with_regular(owner, image, ram, session)?
        }
        None => startup_file::StartupFiles::startup_files_new(owner, image, ram)?,
    };
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

    let optical_open = [token_va, 0x80100080, attrib_va, status_va, 1, 0x60];
    if regular_enabled {
        let create_link = ram.read_u32(image.image_thunk() + 88)?;
        let delete_link = ram.read_u32(image.image_thunk() + 92)?;
        rom_startup_need([create_link, delete_link] == [0xfe000058, 0xfe00005c])?;
        rom_startup_need(kernel_pool::kernel_ordinal(ram, 22) == Err(-5))?;
        rom_startup_need(
            rom_startup_call(&mut routed, ram, open_marker, stack_va, &optical_open) == Err(-5),
        )?;
        rom_drive_startup(&mut routed, ram, stack_va, create_link, delete_link)?;
    }

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
    if regular_enabled {
        let delete_link = ram.read_u32(image.image_thunk() + 92)?;
        let symbol = stack_va + 672;
        save_startup_name(ram, symbol, stack_va + 704, b"\\??\\d:")?;
        rom_startup_need(
            rom_startup_call(&mut routed, ram, delete_link, stack_va, &[symbol])? == 0,
        )?;
        rom_startup_need(
            rom_startup_call(&mut routed, ram, delete_link, stack_va, &[symbol])? == 0xc0000034,
        )?;
        rom_startup_need(
            rom_startup_call(&mut routed, ram, open_marker, stack_va, &optical_open) == Err(-5),
        )?;
        rom_startup_need(ram.read_u32(token_va)? == token)?;
    }
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
        if regular_enabled {
            let query_marker = ram.read_u32(image.image_thunk() + 68)?;
            let write_marker = ram.read_u32(image.image_thunk() + 72)?;
            let flush_marker = ram.read_u32(image.image_thunk() + 76)?;
            let control_marker = ram.read_u32(image.image_thunk() + 80)?;
            let path_query_marker = ram.read_u32(image.image_thunk() + 84)?;
            rom_startup_need(
                [
                    query_marker,
                    write_marker,
                    flush_marker,
                    control_marker,
                    path_query_marker,
                ] == [0xfe000044, 0xfe000048, 0xfe00004c, 0xfe000050, 0xfe000054],
            )?;
            rom_startup_need(kernel_pool::kernel_ordinal(ram, 17) == Err(-5))?;
            regular_startup_calls(
                &mut routed,
                ram,
                save_frame,
                [
                    ensure_marker,
                    open_marker,
                    query_marker,
                    write_marker,
                    read_marker,
                    close_marker,
                    flush_marker,
                    control_marker,
                    path_query_marker,
                ],
                token,
            )?;
        }
        let event_path = if regular_enabled {
            b"\\Device\\CdRom0\\default.xbe".as_slice()
        } else {
            path.as_slice()
        };
        save_startup_name(ram, name_va, path_va, event_path)?;
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
    ram.write_u32(string_descriptor + 4, path_address)?;
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

fn rom_drive_startup(
    routed: &mut startup_file::StartupFiles,
    ram: &mut memory::Memory,
    stack: u32,
    create_link: u32,
    delete_link: u32,
) -> Result<(), i32> {
    let symbol = stack + 672;
    let target = stack + 688;
    let symbol_text = stack + 704;
    let target_text = stack + 768;
    save_startup_name(ram, symbol, symbol_text, b"\\??\\D:")?;
    for rejected_target in [
        b"\\Device\\CdRom0\\..".as_slice(),
        b"\\??\\D:",
        b"\\Device\\Harddisk0\\partition2",
        b"\\Device\\Harddisk0\\partition1\\UDATA\\4541005c",
    ] {
        save_startup_name(ram, target, target_text, rejected_target)?;
        rom_startup_need(
            rom_startup_call(routed, ram, create_link, stack, &[symbol, target]) == Err(-5),
        )?;
    }
    save_startup_name(ram, target, target_text, b"\\Device\\CdRom0")?;
    ram.write_u16(target + 2, 1)?;
    rom_startup_need(
        rom_startup_call(routed, ram, create_link, stack, &[symbol, target]) == Err(-1),
    )?;
    rom_startup_need(rom_startup_call(routed, ram, delete_link, stack, &[symbol])? == 0xc0000034)?;
    ram.write_u16(target + 2, 15)?;
    rom_startup_need(rom_startup_call(routed, ram, create_link, stack, &[symbol, target])? == 0)?;

    save_startup_name(ram, symbol, symbol_text, b"\\??\\d:")?;
    save_startup_name(ram, target, target_text, b"\\Device\\Harddisk0\\partition1")?;
    rom_startup_need(
        rom_startup_call(routed, ram, create_link, stack, &[symbol, target])? == 0xc0000035,
    )?;

    save_startup_name(ram, symbol, symbol_text, b"\\??\\U:")?;
    save_startup_name(
        ram,
        target,
        target_text,
        b"\\Device\\Harddisk0\\partition1\\UDATA\\4541005b",
    )?;
    rom_startup_need(rom_startup_call(routed, ram, create_link, stack, &[symbol, target])? == 0)?;
    Ok(())
}

fn regular_startup_calls(
    routed: &mut startup_file::StartupFiles,
    ram: &mut memory::Memory,
    frame: [u32; 7],
    slots: [u32; 9],
    optical_token: u32,
) -> Result<(), i32> {
    let [stack, obj, name_address, text_address, handle_address, io_address, query_output] = frame;
    let [create_marker, open_marker, query_marker, write_marker, read_marker, close_marker, flush_marker, control_marker, path_query_marker] =
        slots;
    save_startup_name(
        ram,
        name_address,
        text_address,
        b"\\Device\\Harddisk0\\partition1\\UDATA\\4541005b\\TitleMeta.xbx",
    )?;
    let create_file = [
        handle_address,
        0x40100000,
        obj,
        io_address,
        0,
        4,
        1,
        3,
        0x22,
    ];
    rom_startup_need(rom_startup_call(routed, ram, create_marker, stack, &create_file)? == 0)?;
    let regular_token = ram.read_u32(handle_address)?;
    rom_startup_need(regular_token != optical_token && ram.read_u64(io_address)? == 0x200000000)?;
    rom_startup_need(rom_startup_call(routed, ram, create_marker, stack, &create_file) == Err(-2))?;
    let query_file = [regular_token, io_address, query_output, 56, 34];
    ram.write_u32(query_output + 52, 0x8a6bedcc)?;
    rom_startup_need(rom_startup_call(routed, ram, query_marker, stack, &query_file)? == 0)?;
    rom_startup_need(
        ram.read_u64(query_output)? > 116_444_736_000_000_000
            && ram.read_u64(query_output + 32)? == 0
            && ram.read_u64(query_output + 40)? == 0
            && ram.read_u32(query_output + 48)? == 0x24
            && ram.read_u32(query_output + 52)? == 0x8a6bedcc
            && ram.read_u64(io_address)? == 56u64 << 32,
    )?;
    let foreign_query = [optical_token, io_address, query_output, 56, 34];
    rom_startup_need(
        rom_startup_call(routed, ram, query_marker, stack, &foreign_query) == Err(-4),
    )?;
    let transfer = [regular_token, 0, 0, 0, io_address, query_output, 5, 0];
    rom_startup_need(rom_startup_call(routed, ram, read_marker, stack, &transfer) == Err(-4))?;
    ram.write_bytes(query_output, b"meta!")?;
    rom_startup_need(rom_startup_call(routed, ram, write_marker, stack, &transfer)? == 0)?;
    rom_startup_need(ram.read_u64(io_address)? == 5u64 << 32)?;
    rom_startup_need(rom_startup_call(routed, ram, query_marker, stack, &query_file)? == 0)?;
    rom_startup_need(
        ram.read_u64(query_output + 32)? == 16384
            && ram.read_u64(query_output + 40)? == 5
            && ram.read_u32(query_output + 48)? == 0x24
            && ram.read_u32(query_output + 52)? == 0x8a6bedcc,
    )?;
    let position_query = [regular_token, io_address, query_output, 8, 14];
    rom_startup_need(rom_startup_call(routed, ram, query_marker, stack, &position_query)? == 0)?;
    rom_startup_need(ram.read_u64(query_output)? == 5 && ram.read_u64(io_address)? == 8u64 << 32)?;
    let control_input = query_output + 64;
    let seek_call = [regular_token, io_address, control_input, 8, 14];
    ram.write_u64(control_input, u64::MAX)?;
    rom_startup_need(rom_startup_call(routed, ram, control_marker, stack, &seek_call) == Err(-1))?;
    ram.write_u64(control_input, 2)?;
    rom_startup_need(rom_startup_call(routed, ram, control_marker, stack, &seek_call)? == 0)?;
    rom_startup_need(ram.read_u64(io_address)? == 0)?;
    let eof_call = [regular_token, io_address, control_input, 8, 20];
    ram.write_u64(control_input, 7)?;
    rom_startup_need(rom_startup_call(routed, ram, control_marker, stack, &eof_call)? == 0)?;
    rom_startup_need(
        rom_startup_call(routed, ram, query_marker, stack, &query_file)? == 0
            && ram.read_u64(query_output + 40)? == 7,
    )?;
    ram.write_u64(control_input, 5)?;
    rom_startup_need(rom_startup_call(routed, ram, control_marker, stack, &eof_call)? == 0)?;
    rom_startup_need(
        rom_startup_call(routed, ram, query_marker, stack, &position_query)? == 0
            && ram.read_u64(query_output)? == 2,
    )?;
    let flush_call = [regular_token, io_address];
    rom_startup_need(
        rom_startup_call(routed, ram, flush_marker, stack, &flush_call)? == 0
            && ram.read_u64(io_address)? == 0,
    )?;
    rom_startup_need(
        rom_startup_call(
            routed,
            ram,
            flush_marker,
            stack,
            &[optical_token, io_address],
        ) == Err(-4),
    )?;
    let path_query = [obj, query_output];
    rom_startup_need(
        rom_startup_call(routed, ram, path_query_marker, stack, &path_query)? == 0xc0000034
            && ram.read_u64(query_output)? == 2,
    )?;
    save_startup_name(
        ram,
        name_address,
        text_address,
        b"\\Device\\Harddisk0\\partition1\\UDATA\\4541005b\\missing\\field.xbx",
    )?;
    rom_startup_need(
        rom_startup_call(routed, ram, path_query_marker, stack, &path_query)? == 0xc000003a
            && ram.read_u64(query_output)? == 2,
    )?;
    save_startup_name(
        ram,
        name_address,
        text_address,
        b"\\Device\\Harddisk0\\partition1\\UDATA\\4541005c\\TitleMeta.xbx",
    )?;
    rom_startup_need(
        rom_startup_call(routed, ram, path_query_marker, stack, &path_query) == Err(-5),
    )?;
    rom_startup_need(rom_startup_call(routed, ram, close_marker, stack, &[regular_token])? == 0)?;
    rom_startup_need(rom_startup_call(routed, ram, query_marker, stack, &query_file) == Err(-4))?;
    save_startup_name(ram, name_address, text_address, b"\\??\\u:\\titlemeta.XBX")?;
    let reopen = [handle_address, 0x80100000, obj, io_address, 3, 0x20];
    let mut result = Err(-2);
    for _ in 0..4096 {
        result = rom_startup_call(routed, ram, open_marker, stack, &reopen);
        if result != Err(-2) && result != Ok(0xc0000034) {
            break;
        }
        memory::Memory::call_retry_pause()?;
    }
    rom_startup_need(result == Ok(0) && ram.read_u64(io_address)? == 0x100000000)?;
    rom_startup_need(rom_startup_call(routed, ram, path_query_marker, stack, &path_query)? == 0)?;
    rom_startup_need(
        ram.read_u64(query_output + 40)? == 5
            && ram.read_u32(query_output + 48)? == 0x24
            && ram.read_u32(query_output + 52)? == 0x8a6bedcc,
    )?;
    let reader_token = ram.read_u32(handle_address)?;
    let read_file = [reader_token, 0, 0, 0, io_address, query_output, 5, 0];
    rom_startup_need(rom_startup_call(routed, ram, read_marker, stack, &read_file)? == 0)?;
    let mut verified_bytes = [0; 5];
    ram.read_bytes(query_output, &mut verified_bytes)?;
    rom_startup_need(&verified_bytes == b"meta!" && ram.read_u64(io_address)? == 5u64 << 32)?;
    rom_startup_need(rom_startup_call(routed, ram, read_marker, stack, &read_file)? == 0xc0000011)?;
    rom_startup_need(
        rom_startup_call(
            routed,
            ram,
            flush_marker,
            stack,
            &[reader_token, io_address],
        ) == Err(-4),
    )?;
    let reader_eof = [reader_token, io_address, control_input, 8, 20];
    rom_startup_need(rom_startup_call(routed, ram, control_marker, stack, &reader_eof) == Err(-4))?;
    // Leave the reader open to establish that the startup router drains it.
    save_startup_name(
        ram,
        name_address,
        text_address,
        b"U:\\..\\4541005c\\TitleMeta.xbx",
    )?;
    rom_startup_need(
        rom_startup_call(routed, ram, path_query_marker, stack, &path_query) == Err(-1),
    )?;
    gamepkg::diagnostic(1, 0x544b4700, &[5, 0x24, 16384, 2, 7, 210])?;
    Ok(())
}
