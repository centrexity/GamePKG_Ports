// Ordinary source-checked image conformance entry, not a production launcher.
use crate::bootstrap;
use crate::disc;
use crate::gamepkg;
use crate::layout;
use crate::memory;
use crate::sha256;
use crate::xbe;
fn image_memory() -> Result<memory::Memory, i32> {
    let mut windows = [0u8; 48];
    if gamepkg::local_call(1, &[], &mut windows)? != 48 {
        return Err(-3);
    }
    let window = u64::from_le_bytes([
        windows[0], windows[1], windows[2], windows[3], windows[4], windows[5], windows[6],
        windows[7],
    ]);
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(window));
    request.extend(u64::to_le_bytes(0));
    request.extend(u64::to_le_bytes(67108864));
    request.extend(u32::to_le_bytes(2));
    if gamepkg::local_call(2, &request, &mut [])? != 0 {
        return Err(-3);
    }
    let root = memory::Memory::new()?;
    let plan = layout::Layout::layout_current(&root)?;
    if plan.layout_banks()[0] != window {
        return Err(-3);
    }
    let mut ram = plan.layout_view(&root)?;
    ram.fill(0, 0xa5, 67108864)?;
    Ok(ram)
}
fn image_run(
    handle: u64,
    expected: [u8; 32],
    ram: &mut memory::Memory,
    scenario: u8,
) -> Result<(), i32> {
    let indexed = disc::Disc::disc_open(handle)?;
    let file = indexed.disc_find("default.xbe")?;
    let plan = xbe::Image::image_plan(&indexed, file, expected)?;
    plan.image_load(ram)?;
    if scenario == 4 {
        gamepkg::local_record_release(ram.call_handle())?;
    }
    if scenario == 5 {
        ram.call_bounds(0, 0)?;
    }
    let startup = bootstrap::Bootstrap::boot_plan(&plan, ram)?;
    let startup_fields = startup.boot_layout();
    let mut fields = [0u64; 14];
    for number in 0..14usize {
        fields[number] = u64::from(startup_fields[number]);
    }
    gamepkg::diagnostic(1, 0x544b0300, &fields)?;
    if scenario == 1 {
        gamepkg::local_record_release(ram.call_handle())?;
    }
    if scenario == 2 {
        let mut windows = [0u8; 48];
        if gamepkg::local_call(1, &[], &mut windows)? != 48 {
            return Err(-3);
        }
        let window = u64::from_le_bytes([
            windows[0], windows[1], windows[2], windows[3], windows[4], windows[5], windows[6],
            windows[7],
        ]);
        let mut request = std::vec::Vec::new();
        request.extend(u64::to_le_bytes(window));
        request.extend(u64::to_le_bytes(0x4000));
        request.extend(u64::to_le_bytes(4096));
        request.extend(u32::to_le_bytes(1));
        if gamepkg::local_call(3, &request, &mut [])? != 0 {
            return Err(-3);
        }
    }
    let context = if scenario == 3 {
        let mut other = memory::Memory::new()?;
        startup.boot_apply(&mut other)?
    } else {
        startup.boot_apply(ram)?
    };
    gamepkg::diagnostic_bytes(1, 0x544b0301, &context.encode())?;
    // Loaded image and startup writes must be visible through every main mirror.
    for offset in [0x10000u32, context.fs_base, context.esp] {
        let expected = ram.read_u32(offset)?;
        for mirror in 1..29u32 {
            if ram.read_u32(mirror * 67108864 + offset)? != expected {
                return Err(-3);
            }
        }
    }
    for at in [
        0x80000000u32,
        0xf0000000,
        0xff000000,
        0xfd000000,
        0xfe800000,
    ] {
        if ram.read_u8(at) != Err(-4) {
            return Err(-3);
        }
    }

    Ok(())
}
fn image_witness(ram: &memory::Memory) -> Result<(), i32> {
    let mut bytes = std::vec::Vec::new();
    bytes.resize(1048576, 0u8);
    for number in 0..64u32 {
        ram.read_bytes(number * 1048576, &mut bytes)?;
        let mut hash = sha256::Sha256::sha_new();
        hash.sha_update(&bytes)?;
        let mut record = [0u8; 36];
        record[..4].copy_from_slice(&u32::to_le_bytes(number));
        record[4..].copy_from_slice(&hash.sha_finish());
        gamepkg::diagnostic_bytes(1, 0x544b0302, &record)?;
    }
    Ok(())
}
fn image_execute() -> Result<(), i32> {
    let mut rom = 0u64;
    let mut assets = 0u64;
    for grant in gamepkg::grants()? {
        if grant.kind == 3 {
            rom = grant.id;
        }
        if grant.kind == 1 {
            assets = grant.id;
        }
    }
    if rom == 0 || assets == 0 {
        return Err(-4);
    }
    let handle = gamepkg::file_open(assets, false, "expected-sha256")?;
    let mut expected = [0u8; 32];
    if gamepkg::file_read(handle, 0, &mut expected)? != 32 {
        return Err(-3);
    }
    gamepkg::file_close(handle)?;
    let scenario_handle = gamepkg::file_open(assets, false, "scenario")?;
    let mut scenario = [0u8; 1];
    if gamepkg::file_read(scenario_handle, 0, &mut scenario)? != 1 {
        return Err(-3);
    }
    gamepkg::file_close(scenario_handle)?;
    let mut ram = image_memory()?;
    let result = image_run(rom, expected, &mut ram, scenario[0]);
    image_witness(&ram)?;
    let status = match result {
        Ok(()) => 0u64,
        Err(error) => error as i64 as u64,
    };
    gamepkg::diagnostic(1, 0x544b03ff, &[status])?;
    Ok(())
}
pub fn game_main() {
    if let Err(error) = image_execute() {
        let _ = gamepkg::diagnostic(3, 0x544b03fe, &[error as i64 as u64]);
    }
}
