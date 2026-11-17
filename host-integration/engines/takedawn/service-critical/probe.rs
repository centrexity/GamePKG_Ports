use crate::gamepkg;
use crate::kernel_pool;
use crate::setup;
fn cs_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-801)
    }
}
fn cs_native(
    map: &kernel_pool::Shadow,
    ram: &crate::memory::Memory,
    guest: u32,
) -> Result<u64, i32> {
    let mut guard = map.shadow_lock(ram)?;
    let value = guard.shadow_lookup(ram, guest);
    let closed = guard.shadow_guard_close(ram);
    let id = value?.ok_or(-802)?;
    closed?;
    Ok(id)
}
pub fn critical_checks() -> Result<(), i32> {
    let mut f = setup::Fixture::mixed_fixture_new()?;
    let mut map = kernel_pool::Shadow::shadow_new(&f.ram)?;
    let mut domain = kernel_pool::Criticals::criticals_new(&f.ram, &map)?;
    f.ram.write_bytes(0x10000, &[0xa5u8; 32])?;
    for guest in [0u32, 1, 0x10003, 0x20000, 0xfffffff0] {
        kernel_pool::critical_initialize(&domain, &map, &f.ram, guest)?;
        kernel_pool::critical_initialize(&domain, &map, &f.ram, guest)?;
        kernel_pool::critical_enter(&domain, &map, &f.ram, guest)?;
        kernel_pool::critical_enter(&domain, &map, &f.ram, guest)?;
        if guest != 0 {
            let id = cs_native(&map, &f.ram, guest)?;
            let state = gamepkg::local_mutex_state(id)?;
            cs_need(state.owned_by_caller && state.depth == 2)?;
            cs_need(domain.criticals_close(&f.ram, &map) == Err(-2) && !domain.criticals_closed())?;
        }
        kernel_pool::critical_leave(&domain, &map, &f.ram, guest)?;
        kernel_pool::critical_leave(&domain, &map, &f.ram, guest)?;
    }
    cs_need(domain.criticals_counts(&f.ram, &map)? == [8, 8])?;
    let mut canary = [0u8; 32];
    f.ram.read_bytes(0x10000, &mut canary)?;
    cs_need(canary == [0xa5u8; 32])?;
    // Leave first creates a separate unowned shadow but cannot fake ownership.
    cs_need(kernel_pool::critical_leave(&domain, &map, &f.ram, 0x34567) == Err(-4))?;
    let id = cs_native(&map, &f.ram, 0x34567)?;
    cs_need(!gamepkg::local_mutex_state(id)?.locked)?;
    let other = crate::memory::Memory::new()?;
    cs_need(domain.criticals_close(&other, &map) == Err(-4))?;
    gamepkg::local_record_release(other.call_handle())?;
    domain.criticals_close(&f.ram, &map)?;
    cs_need(domain.criticals_closed())?;
    domain.criticals_close(&f.ram, &map)?;
    let mut guard = map.shadow_cleanup_lock(&f.ram)?;
    cs_need(guard.shadow_entries(&f.ram)?.is_empty())?;
    guard.shadow_guard_close(&f.ram)?;
    map.shadow_close(&f.ram)?;
    f.mixed_fixture_close()
}

// Scanned test code deliberately corrupts owned records through the public
// bounded record service. None of these bytes confer native ownership.
pub fn critical_controls() -> Result<(), i32> {
    let mut f = setup::Fixture::mixed_fixture_new()?;
    let mut map = kernel_pool::Shadow::shadow_new(&f.ram)?;
    let mut domain = kernel_pool::Criticals::criticals_new(&f.ram, &map)?;
    kernel_pool::critical_initialize(&domain, &map, &f.ram, 0x1234)?;
    let original = gamepkg::local_record_snapshot(domain.criticals_id(), 2624)?.1;
    for (at, value) in [
        (0usize, 0u64),
        (8, 0),
        (16, 0),
        (24, 0),
        (32, u64::MAX),
        (40, 2),
        (48, 0x100000000),
        (56, 0x100000000),
        (72, 4),
        (88, (0x1234u64 << 32) | 1),
        (2112, 123),
        (2120, 3),
    ] {
        let (version, mut bytes) = gamepkg::local_record_snapshot(domain.criticals_id(), 2624)?;
        bytes
            .get_mut(at..at + 8)
            .ok_or(-1)?
            .copy_from_slice(&u64::to_le_bytes(value));
        setup::mixed_need(
            820,
            gamepkg::local_record_compare_exchange(domain.criticals_id(), version, &bytes)?
                > version,
        )?;
        setup::mixed_need(
            821,
            kernel_pool::Criticals::criticals_borrow(&f.ram, &map, domain.criticals_id()).is_err(),
        )?;
        let version = gamepkg::local_record_snapshot(domain.criticals_id(), 2624)?.0;
        setup::mixed_need(
            822,
            gamepkg::local_record_compare_exchange(domain.criticals_id(), version, &original)?
                > version,
        )?;
    }
    let other = kernel_pool::Shadow::shadow_new(&f.ram)?;
    setup::mixed_need(
        823,
        kernel_pool::Criticals::criticals_borrow(&f.ram, &other, domain.criticals_id())
            .err()
            .is_some(),
    )?;
    let mut other = other;
    other.shadow_close(&f.ram)?;
    // A private reserved prefix closes without creating a native mutex.
    let mut reserved = domain.criticals_begin(&f.ram, &map, 0x2345)?;
    setup::mixed_need(
        824,
        reserved.critical_creation_id() == 0 && reserved.critical_creation_phase() == 0,
    )?;
    reserved.critical_creation_close(&f.ram)?;
    reserved.critical_creation_close(&f.ram)?;
    setup::mixed_need(825, reserved.critical_create_publish(&f.ram) == Err(-1))?;
    // No hidden quota increase and no silent unlocked success on exhaustion.
    let mut exhausted = false;
    let mut made = 0u32;
    for key in 1..129u32 {
        let mut pending = match domain.criticals_begin(&f.ram, &map, key) {
            Ok(value) => value,
            Err(-2) => {
                exhausted = true;
                break;
            }
            Err(code) => return Err(code),
        };
        let result = pending.critical_create_publish(&f.ram);
        pending.critical_creation_close(&f.ram)?;
        setup::mixed_need(826, pending.critical_creation_closed())?;
        match result {
            Ok(_) => {
                made += 1;
            }
            Err(-2) => {
                exhausted = true;
                break;
            }
            Err(code) => return Err(code),
        }
    }
    setup::mixed_need(827, exhausted && made > 0 && made < 128)?;
    domain.criticals_close(&f.ram, &map)?;
    map.shadow_close(&f.ram)?;
    f.mixed_fixture_close()
}
