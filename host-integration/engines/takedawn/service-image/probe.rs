// Ordinary source-checked image conformance entry, not a production launcher.
use crate::disc;
use crate::gamepkg;
use crate::memory;
use crate::sha256;
use crate::xbe;
fn image_memory() -> Result<memory::Memory, i32> {
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
    request.extend(u64::to_le_bytes(67108864));
    request.extend(u32::to_le_bytes(2));
    if gamepkg::local_call(2, &request, &mut [])? != 0 {
        return Err(-3);
    }
    let mut ram = memory::Memory::new()?;
    ram.attach(0, 67108864, window, 0, true)?;
    ram.fill(0, 0xa5, 67108864)?;
    Ok(ram)
}
fn image_run(handle: u64, expected: [u8; 32], ram: &mut memory::Memory) -> Result<(), i32> {
    let indexed = disc::Disc::disc_open(handle)?;
    let file = indexed.disc_find("default.xbe")?;
    let plan = xbe::Image::image_plan(&indexed, file, expected)?;
    let bounds = plan.image_bounds();
    gamepkg::diagnostic(
        1,
        0x544b0200,
        &[
            u64::from(plan.image_entry()),
            u64::from(plan.image_thunk()),
            u64::from(plan.image_tls()),
            u64::from(plan.image_section_count()),
            u64::from(bounds[0]),
            u64::from(bounds[1]),
            u64::from(bounds[2]),
            u64::from(bounds[3]),
        ],
    )?;
    gamepkg::diagnostic_bytes(1, 0x544b0201, &plan.image_digest())?;
    let (now, generation) = gamepkg::monotonic()?;
    gamepkg::diagnostic(1, 0x544b0203, &[handle, generation])?;
    let (_, _, reason) = gamepkg::wait_until(now.checked_add(1000000000).ok_or(-1)?, generation)?;
    if reason != 1 {
        return Err(-3);
    }
    plan.image_load(ram)?;
    let (_, record) = gamepkg::local_record_snapshot(ram.call_handle(), 280)?;
    let low = u32::from_le_bytes([record[0], record[1], record[2], record[3]]);
    let high = u32::from_le_bytes([record[4], record[5], record[6], record[7]]);
    if low != bounds[2] || high != bounds[3] {
        return Err(-3);
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
        gamepkg::diagnostic_bytes(1, 0x544b0202, &record)?;
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
    let mut ram = image_memory()?;
    let result = image_run(rom, expected, &mut ram);
    image_witness(&ram)?;
    let status = match result {
        Ok(()) => 0u64,
        Err(error) => error as i64 as u64,
    };
    gamepkg::diagnostic(1, 0x544b02ff, &[status])?;
    Ok(())
}
pub fn game_main() {
    if let Err(error) = image_execute() {
        let _ = gamepkg::diagnostic(3, 0x544b02fe, &[error as i64 as u64]);
    }
}
