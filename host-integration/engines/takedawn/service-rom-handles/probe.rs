use crate::disc;
use crate::gamepkg;
use crate::memory;
use crate::rom_handles;
use crate::setup;

fn check(code: i32, value: bool) -> Result<(), i32> {
    setup::mixed_need(code, value)
}

fn run_handles(grant: u64) -> Result<(), i32> {
    let image = disc::Disc::disc_open(grant)?;
    let mut f = setup::Fixture::mixed_fixture_new()?;
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
    pending.rom_read_finish(&f.ram, true)?;
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
    files.rom_close(&f.ram, &f.table, first)?;
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

pub fn run() -> Result<(), i32> {
    for grant in gamepkg::grants()? {
        if grant.kind == 3 {
            return run_handles(grant.id);
        }
    }
    Err(-4)
}
