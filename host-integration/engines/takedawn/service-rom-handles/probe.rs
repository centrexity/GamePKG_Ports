use crate::disc;
use crate::gamepkg;
use crate::kernel_file;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
use crate::rom_handles;
use crate::setup;

fn check(code: i32, value: bool) -> Result<(), i32> {
    setup::mixed_need(code, value)
}

fn run_handles(grant: u64) -> Result<(), i32> {
    let image = disc::Disc::disc_open(grant)?;
    let mut f = setup::Fixture::mixed_fixture_new()?;
    run_guest_file_calls(&image, &mut f)?;
    let mut files = rom_handles::RomFiles::rom_files_new(&f.ram, &f.table)?;
    let event = f.mixed_event(false, 0)?;
    let wrong = event[1] as u32;

    let mut first = 0;
    check(
        851,
        files.rom_open(&f.ram, &f.table, &image, "../outside", &mut first) == Err(-1),
    )?;
    check(852, first == 0)?;
    files.rom_open(&f.ram, &f.table, &image, "default.xbe", &mut first)?;
    let mut second = 0;
    files.rom_open(&f.ram, &f.table, &image, "DEFAULT.XBE", &mut second)?;
    check(853, first != second && first != wrong && second != wrong)?;
    let mut read = None;
    check(
        854,
        files.rom_read_begin(&f.ram, &f.table, wrong, &mut read) == Err(-4),
    )?;
    check(855, files.rom_close(&f.ram, &f.table, wrong) == Err(-4))?;
    files.rom_read_begin(&f.ram, &f.table, first, &mut read)?;
    check(856, files.rom_close(&f.ram, &f.table, first) == Err(-2))?;
    let mut blocked = None;
    check(
        857,
        files.rom_read_begin(&f.ram, &f.table, first, &mut blocked) == Err(-2),
    )?;
    let mut independent = None;
    files.rom_read_begin(&f.ram, &f.table, second, &mut independent)?;
    let mut bytes = [0u8; 4];
    let second_read = independent.as_mut().ok_or(-3)?;
    check(
        858,
        second_read.rom_read_into(&f.ram, None, &mut bytes)? == 4 && &bytes == b"XBEH",
    )?;
    second_read.rom_read_finish(&f.ram, true)?;

    let busy_file = files.rom_wait_begin(&f.ram, &f.table, first)?;
    check(912, busy_file.rom_wait(&f.ram, Some(0)) == Ok(false))?;
    check(913, busy_file.rom_wait(&f.ram, Some(2)) == Ok(false))?;
    let pending = read.as_mut().ok_or(-3)?;
    let mut expected = [0u8; 4];
    let stream = image.disc_open_file("default.xbe")?;
    stream.disc_file_exact(8, &mut expected)?;
    check(
        859,
        pending.rom_read_into(&f.ram, Some(8), &mut bytes)? == 4 && bytes == expected,
    )?;
    check(
        860,
        pending.rom_read_into(&f.ram, None, &mut bytes) == Err(-1),
    )?;
    let foreign = memory::Memory::new()?;
    check(861, pending.rom_read_finish(&foreign, true) == Err(-4))?;
    gamepkg::local_record_release(foreign.call_handle())?;
    rom_wait_across_tasks(&f, pending, first)?;
    check(914, busy_file.rom_wait(&f.ram, None) == Ok(true))?;
    pending.rom_read_finish(&f.ram, true)?;

    let mut begin = None;
    files.rom_read_begin(&f.ram, &f.table, first, &mut begin)?;
    let sequential = begin.as_mut().ok_or(-3)?;
    check(
        862,
        sequential.rom_read_into(&f.ram, None, &mut bytes)? == 4 && &bytes == b"XBEH",
    )?;
    sequential.rom_read_finish(&f.ram, true)?;
    let mut begin = None;
    files.rom_read_begin(&f.ram, &f.table, first, &mut begin)?;
    let sequential = begin.as_mut().ok_or(-3)?;
    stream.disc_file_exact(4, &mut expected)?;
    check(
        863,
        sequential.rom_read_into(&f.ram, None, &mut bytes)? == 4 && bytes == expected,
    )?;
    sequential.rom_read_finish(&f.ram, true)?;
    let mut begin = None;
    files.rom_read_begin(&f.ram, &f.table, first, &mut begin)?;
    let abandoned = begin.as_mut().ok_or(-3)?;
    stream.disc_file_exact(8, &mut expected)?;
    check(
        868,
        abandoned.rom_read_into(&f.ram, None, &mut bytes)? == 4 && bytes == expected,
    )?;
    abandoned.rom_read_finish(&f.ram, false)?;
    check(869, abandoned.rom_read_finish(&f.ram, true) == Err(-1))?;
    let mut begin = None;
    files.rom_read_begin(&f.ram, &f.table, first, &mut begin)?;
    let retry = begin.as_mut().ok_or(-3)?;
    check(
        870,
        retry.rom_read_into(&f.ram, None, &mut bytes)? == 4 && bytes == expected,
    )?;
    retry.rom_read_finish(&f.ram, true)?;
    let mut begin = None;
    files.rom_read_begin(&f.ram, &f.table, first, &mut begin)?;
    let guest = begin.as_mut().ok_or(-3)?;
    let mut missing = f.ram.call_view();
    check(
        871,
        guest.rom_read_guest(&mut missing, None, 0x10100, 4, 0x10200) == Err(-4),
    )?;
    check(
        872,
        guest.rom_read_guest(&mut f.ram, None, 0x10100, 4, 0x10200)? == 0,
    )?;
    stream.disc_file_exact(12, &mut expected)?;
    f.ram.read_bytes(0x10100, &mut bytes)?;
    check(
        873,
        bytes == expected && f.ram.read_u64(0x10200)? == 0x400000000,
    )?;
    guest.rom_read_finish(&f.ram, true)?;

    let mut begin = None;
    files.rom_read_begin(&f.ram, &f.table, first, &mut begin)?;
    let eof = begin.as_mut().ok_or(-3)?;
    check(
        874,
        eof.rom_read_guest(&mut f.ram, Some(u64::MAX), 0x10100, 4, 0x10200)? == 0xc0000011,
    )?;
    check(875, f.ram.read_u64(0x10200)? == 0xc0000011)?;
    f.ram.read_bytes(0x10100, &mut bytes)?;
    check(876, bytes == expected)?;
    eof.rom_read_finish(&f.ram, true)?;

    // The second guest range resolves, but its backing page is uncommitted.
    // The local scatter operation must keep the first range intact.
    let mut info = [0u8; 64];
    check(879, gamepkg::local_call(1, &[], &mut info)? == 64)?;
    f.ram
        .attach(0x11000, 4096, setup::mixed_word(&info, 0)?, 4096, true)?;
    check(880, f.ram.memory_check_write(0x11000, 8) == Err(-4))?;
    check(
        877,
        f.ram.memory_write_pair(0x10100, b"FAIL", 0x11000, b"\x01") == Err(-4),
    )?;
    f.ram.read_bytes(0x10100, &mut bytes)?;
    check(878, bytes == expected)?;
    let original_file = files.rom_wait_begin(&f.ram, &f.table, first)?;
    check(910, original_file.rom_wait_ready(&f.ram) == Ok(true))?;
    files.rom_close(&f.ram, &f.table, first)?;
    let mut reused_file = 0;
    files.rom_open(&f.ram, &f.table, &image, "default.xbe", &mut reused_file)?;
    check(
        911,
        reused_file == first && original_file.rom_wait_ready(&f.ram) == Err(-4),
    )?;
    files.rom_close(&f.ram, &f.table, reused_file)?;
    check(864, files.rom_close(&f.ram, &f.table, first) == Err(-4))?;

    let mut begin = None;
    files.rom_read_begin(&f.ram, &f.table, second, &mut begin)?;
    let waiting = begin.as_mut().ok_or(-3)?;
    let (now, generation) = gamepkg::monotonic()?;
    gamepkg::diagnostic(1, 0x544b4400, &[grant])?;
    let (_, _, reason) = gamepkg::wait_until(now.checked_add(1000000000).ok_or(-1)?, generation)?;
    check(
        865,
        reason == 1 && waiting.rom_read_into(&f.ram, None, &mut bytes) == Err(-4),
    )?;
    waiting.rom_read_finish(&f.ram, false)?;
    files.rom_drain(&f.ram, &f.table)?;
    check(866, files.rom_close(&f.ram, &f.table, second) == Err(-4))?;
    check(867, gamepkg::local_event_state(event[0]).is_ok())?;
    f.events.events_remove(&f.ram, &f.table, wrong)?;
    f.mixed_fixture_close()
}

fn rom_wait_across_tasks(
    fixture: &setup::Fixture,
    reservation: &mut rom_handles::RomRead,
    guest_file: u32,
) -> Result<(), i32> {
    let mut identifier = [0u8; 8];
    gamepkg::local_call(20, &[1, 0, 0, 0, 0, 0, 0, 0], &mut identifier)?;
    let rendezvous = u64::from_le_bytes(identifier);
    let arguments = [
        u64::to_le_bytes(fixture.ram.call_handle()).as_slice(),
        &u64::to_le_bytes(fixture.table.handles_id()),
        &u64::to_le_bytes(u64::from(guest_file)),
        &u64::to_le_bytes(rendezvous),
    ]
    .concat();
    let argument_record = gamepkg::local_record_create(&arguments)?;
    let request = [
        u32::to_le_bytes(0).as_slice(),
        &u64::to_le_bytes(argument_record),
        &u32::to_le_bytes(0),
    ]
    .concat();
    gamepkg::local_call(10, &request, &mut identifier)?;
    let waiter_task = u64::from_le_bytes(identifier);
    gamepkg::local_event_wait(rendezvous, 1000)?;
    reservation.rom_read_finish(&fixture.ram, true)?;

    let mut waiter_result = None;
    for _ in 0..4096 {
        match gamepkg::local_task_join(waiter_task) {
            Ok(proof) => {
                waiter_result = Some(proof.result());
                break;
            }
            Err(-2) => memory::Memory::call_retry_pause()?,
            Err(error) => return Err(error),
        }
    }
    gamepkg::local_record_release(argument_record)?;
    gamepkg::local_call(25, &u64::to_le_bytes(rendezvous), &mut [])?;
    check(915, waiter_result == Some(0))
}

pub fn rom_file_wait_worker(argument_record: u64) -> u64 {
    match rom_file_wait_actor(argument_record) {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}

fn rom_file_wait_actor(argument_record: u64) -> Result<(), i32> {
    let (_, arguments) = gamepkg::local_record_snapshot(argument_record, 32)?;
    check(916, arguments.len() == 32)?;
    let ram = memory::Memory::with_call_handle(setup::mixed_word(&arguments, 0)?)?;
    let table = kernel_pool::Handles::handles_borrow(&ram, setup::mixed_word(&arguments, 8)?)?;
    let mut files = rom_handles::RomFiles::rom_files_new(&ram, &table)?;
    let guest_file = setup::mixed_word(&arguments, 16)? as u32;
    let rendezvous = setup::mixed_word(&arguments, 24)?;
    let observation = files.rom_wait_begin(&ram, &table, guest_file)?;
    check(917, observation.rom_wait(&ram, Some(0)) == Ok(false))?;
    let signal = [
        u64::to_le_bytes(rendezvous).as_slice(),
        &u32::to_le_bytes(1),
    ]
    .concat();
    gamepkg::local_call(22, &signal, &mut [])?;
    check(918, observation.rom_wait(&ram, Some(1000)) == Ok(true))
}

pub fn run() -> Result<(), i32> {
    for grant in gamepkg::grants()? {
        if grant.kind == 3 {
            return run_handles(grant.id);
        }
    }
    Err(-4)
}

fn call_guest_file(
    bridge: &mut kernel_file::RomKernel,
    image: &disc::Disc,
    fixture: &mut setup::Fixture,
    ordinal: u32,
    args: &[u32],
    outcome: Result<u32, i32>,
) -> Result<(), i32> {
    let mut cpu = registers::Registers::new();
    cpu.esp = 0x10300;
    cpu.eax = 0x89abcdef;
    cpu.ebx = 0x12345678;
    fixture.ram.write_u32(cpu.esp, 0x87654321)?;
    for (index, value) in args.iter().enumerate() {
        fixture
            .ram
            .write_u32(cpu.esp + 4 + index as u32 * 4, *value)?;
    }
    let mut expected = registers::Registers::new();
    expected.decode(&cpu.encode())?;
    let result = bridge.rom_kernel_invoke(
        image,
        &fixture.table,
        &fixture.events,
        &mut fixture.ram,
        &mut cpu,
        ordinal,
    );
    match outcome {
        Ok(code) => {
            result?;
            expected.eax = code;
            expected.esp += 4 + args.len() as u32 * 4;
            check(890, fixture.ram.kernel_caller() == 0x87654321)?;
        }
        Err(code) => check(891, result == Err(code))?,
    }
    check(892, expected.encode() == cpu.encode())
}

fn guest_file_path(fixture: &mut setup::Fixture, path: &[u8]) -> Result<(), i32> {
    // Counted string deliberately has no NUL terminator within its length.
    fixture.ram.write_u32(0x10400, 0xfffffffdu32)?;
    fixture.ram.write_u32(0x10404, 0x10420)?;
    fixture.ram.write_u32(0x10408, 0x40)?;
    fixture.ram.write_u16(0x10420, path.len() as u16)?;
    fixture.ram.write_u16(0x10422, path.len() as u16)?;
    fixture.ram.write_u32(0x10424, 0x10440)?;
    fixture.ram.write_bytes(0x10440, path)
}

fn run_guest_file_calls(image: &disc::Disc, fixture: &mut setup::Fixture) -> Result<(), i32> {
    let mut bridge = kernel_file::RomKernel::rom_kernel_new(&fixture.ram, &fixture.table)?;
    let mut open_args = [0x10500, 0x80100080, 0x10400, 0x10510, 1, 0x60];
    fixture.ram.write_u32(0x10500, 0xaabbccdd)?;
    guest_file_path(fixture, b"D:\\missing.bin")?;
    call_guest_file(&mut bridge, image, fixture, 202, &open_args, Ok(0xc0000034))?;
    check(
        893,
        fixture.ram.read_u32(0x10500)? == 0xaabbccdd
            && fixture.ram.read_u64(0x10510)? == 0xc0000034,
    )?;
    guest_file_path(fixture, b"E:\\default.xbe")?;
    call_guest_file(&mut bridge, image, fixture, 202, &open_args, Err(-5))?;
    guest_file_path(fixture, b"D:\\..\\default.xbe")?;
    call_guest_file(&mut bridge, image, fixture, 202, &open_args, Err(-1))?;
    guest_file_path(fixture, b"\\??\\d:\\DEFAULT.XBE")?;
    open_args[1] = 0x40000000;
    call_guest_file(&mut bridge, image, fixture, 202, &open_args, Err(-5))?;
    open_args[1] = 0x80100080;
    call_guest_file(&mut bridge, image, fixture, 202, &open_args, Ok(0))?;
    let token = fixture.ram.read_u32(0x10500)?;
    check(
        894,
        token != 0 && fixture.ram.read_u64(0x10510)? == 0x100000000,
    )?;

    let mut read_args = [token, 0, 1, 0, 0x10510, 0x10520, 4, 0];
    call_guest_file(&mut bridge, image, fixture, 219, &read_args, Err(-5))?;
    read_args[2] = 0;
    call_guest_file(&mut bridge, image, fixture, 219, &read_args, Ok(0))?;
    let mut witness = [0u8; 4];
    fixture.ram.read_bytes(0x10520, &mut witness)?;
    check(
        895,
        &witness == b"XBEH" && fixture.ram.read_u64(0x10510)? == 0x400000000,
    )?;
    fixture.ram.write_u64(0x10540, 127)?;
    read_args[7] = 0x10540;
    call_guest_file(&mut bridge, image, fixture, 219, &read_args, Ok(0))?;
    check(896, fixture.ram.read_u64(0x10510)? == 0x100000000)?;
    fixture.ram.write_u64(0x10540, 128)?;
    call_guest_file(&mut bridge, image, fixture, 219, &read_args, Ok(0xc0000011))?;
    fixture.ram.write_i64(0x10540, -1)?;
    call_guest_file(&mut bridge, image, fixture, 219, &read_args, Ok(0))?;
    let stream = image.disc_open_file("default.xbe")?;
    let mut expected = [0u8; 4];
    stream.disc_file_exact(4, &mut expected)?;
    fixture.ram.read_bytes(0x10520, &mut witness)?;
    check(897, witness == expected)?;
    call_guest_file(&mut bridge, image, fixture, 187, &[token], Ok(0))?;
    call_guest_file(&mut bridge, image, fixture, 219, &read_args, Err(-4))?;
    check(898, crate::kernel_pool::kernel_supported(219) == Err(-5))?;
    let event_pair = fixture.mixed_event(false, 0)?;
    let event_token = event_pair[1] as u32;
    let mut completion =
        fixture
            .events
            .events_completion_begin(&fixture.ram, &fixture.table, event_token)?;
    completion.completion_prepare(&fixture.ram)?;
    check(
        908,
        fixture
            .events
            .events_remove(&fixture.ram, &fixture.table, event_token)
            == Err(-2),
    )?;
    completion.completion_signal(&fixture.ram)?;
    check(909, completion.completion_signal(&fixture.ram) == Err(-1))?;
    completion.completion_close(&fixture.ram)?;
    check(910, gamepkg::local_event_wait(event_pair[0], 0) == Ok(()))?;
    check(911, gamepkg::local_event_wait(event_pair[0], 0) == Err(-2))?;

    // Synchronous with an event still returns the final status.
    open_args[5] = 0x60;
    call_guest_file(&mut bridge, image, fixture, 202, &open_args, Ok(0))?;
    let sync_token = fixture.ram.read_u32(0x10500)?;
    let sync_read = [sync_token, event_token, 0, 0, 0x10510, 0x10520, 4, 0];
    call_guest_file(&mut bridge, image, fixture, 219, &sync_read, Ok(0))?;
    check(900, gamepkg::local_event_wait(event_pair[0], 0) == Ok(()))?;
    check(901, gamepkg::local_event_wait(event_pair[0], 0) == Err(-2))?;
    call_guest_file(&mut bridge, image, fixture, 187, &[sync_token], Ok(0))?;

    // FILE_OPEN through NtCreateFile tracks the asynchronous return mode.
    let mut create_args = [0x10500, 0x80100080, 0x10400, 0x10510, 0, 0x80, 1, 2, 0x40];
    call_guest_file(&mut bridge, image, fixture, 190, &create_args, Err(-5))?;
    create_args[7] = 1;
    call_guest_file(&mut bridge, image, fixture, 190, &create_args, Ok(0))?;
    let async_token = fixture.ram.read_u32(0x10500)?;
    let mut async_read = [async_token, 0, 0, 0, 0x10510, 0x10520, 4, 0];
    fixture.ram.write_u64(0x10540, 0)?;
    async_read[7] = 0x10540;
    call_guest_file(&mut bridge, image, fixture, 219, &async_read, Ok(0x103))?;
    check(908, fixture.ram.read_u64(0x10510)? == 0x400000000)?;
    call_guest_file(
        &mut bridge,
        image,
        fixture,
        233,
        &[async_token, 0, 0],
        Ok(0),
    )?;
    call_guest_file(
        &mut bridge,
        image,
        fixture,
        234,
        &[async_token, 1, 0, 0x10540],
        Ok(0),
    )?;
    call_guest_file(
        &mut bridge,
        image,
        fixture,
        233,
        &[async_token, 1, 0],
        Err(-5),
    )?;
    call_guest_file(
        &mut bridge,
        image,
        fixture,
        233,
        &[event_token, 0, 0],
        Err(-4),
    )?;
    check(909, gamepkg::local_event_wait(event_pair[0], 0) == Err(-2))?;
    async_read[1] = async_token;
    call_guest_file(&mut bridge, image, fixture, 219, &async_read, Err(-1))?;
    async_read[1] = event_token;
    async_read[5] = 0x12000;
    call_guest_file(&mut bridge, image, fixture, 219, &async_read, Err(-4))?;
    check(902, gamepkg::local_event_wait(event_pair[0], 0) == Err(-2))?;
    async_read[5] = 0x10520;
    call_guest_file(&mut bridge, image, fixture, 219, &async_read, Ok(0x103))?;
    fixture.ram.read_bytes(0x10520, &mut witness)?;
    check(
        903,
        &witness == b"XBEH" && fixture.ram.read_u64(0x10510)? == 0x400000000,
    )?;
    check(904, gamepkg::local_event_wait(event_pair[0], 0) == Ok(()))?;
    fixture.ram.write_u64(0x10540, 128)?;
    async_read[7] = 0x10540;
    call_guest_file(&mut bridge, image, fixture, 219, &async_read, Ok(0x103))?;
    check(905, fixture.ram.read_u64(0x10510)? == 0xc0000011)?;
    check(906, gamepkg::local_event_wait(event_pair[0], 0) == Ok(()))?;
    call_guest_file(&mut bridge, image, fixture, 187, &[async_token], Ok(0))?;
    fixture
        .events
        .events_remove(&fixture.ram, &fixture.table, event_token)?;
    check(907, crate::kernel_pool::kernel_supported(190) == Err(-5))?;

    bridge.rom_kernel_drain(&fixture.ram, &fixture.table)
}
