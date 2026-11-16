use crate::cases;
use crate::events;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::shadow;
fn event_need(value: bool) -> Result<(), i32> {
    if value {
        Ok(())
    } else {
        Err(-3)
    }
}
fn event_word(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let b = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
    ]))
}
fn event_operation(
    map_view: &shadow::Shadow,
    ram: &mut memory::Memory,
    args: [u32; 4],
) -> Result<u64, i32> {
    let guest = 0x740100 + args[1] * 64;
    match args[0] {
        0 => {
            events::event_initialize(map_view, ram, guest, args[2], args[3])?;
            Ok(0)
        }
        1 => {
            events::timer_initialize(map_view, ram, guest, args[2])?;
            Ok(0)
        }
        2 => Ok(u64::from(events::event_set(map_view, ram, guest)?)),
        3 => {
            events::event_reset(map_view, ram, guest)?;
            Ok(0)
        }
        4 => match events::event_wait(map_view, ram, guest, 0) {
            Ok(()) => Ok(0),
            Err(-2) => Ok(258),
            Err(-5) => Ok(0xffffffff),
            Err(code) => Err(code),
        },
        5 => Ok(u64::from(events::timer_reset_after_set(
            map_view, ram, guest,
        )?)),
        6 => Ok(u64::from(events::timer_signal_after_callback(
            map_view, ram, guest,
        )?)),
        _ => Ok(u64::from(events::event_remove(map_view, ram, guest)?)),
    }
}
pub fn event_probe() -> Result<std::vec::Vec<u8>, i32> {
    let mut windows = [0u8; 16];
    event_need(gamepkg::local_call(1, &[], &mut windows)? == 16)?;
    let window = event_word(&windows, 0)?;
    gamepkg::local_call(
        2,
        &[
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(65536),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    let mut ram = memory::Memory::new()?;
    ram.attach(0x740000, 65536, window, 0, true)?;
    ram.fill(0x740000, 0xa5, 65536)?;
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2)?;
    let mut map_view = shadow::Shadow::shadow_new(&ram)?;
    let mut known = std::vec::Vec::new();
    let mut output = std::vec::Vec::new();
    for index in 0..256u32 {
        let result = event_operation(&map_view, &mut ram, cases::event_case(index))?;
        let mut guard = map_view.shadow_lock(&ram)?;
        let mut current = std::vec::Vec::new();
        let mut states = std::vec::Vec::new();
        for slot in 0..16u32 {
            match guard.shadow_lookup(&ram, 0x740100 + slot * 64)? {
                None => {
                    current.push(0u64);
                    states.extend([0u8; 12]);
                }
                Some(id) => {
                    let mut epoch = 0u32;
                    for lane in 0..known.len() {
                        if known[lane] == id {
                            epoch = lane as u32 + 1;
                        }
                    }
                    if epoch == 0 {
                        known.push(id);
                        epoch = known.len() as u32;
                    }
                    let state = gamepkg::local_event_state(id)?;
                    current.push(id);
                    states.extend(u32::to_le_bytes(epoch));
                    states.extend(u32::to_le_bytes(u32::from(state.manual)));
                    states.extend(u32::to_le_bytes(u32::from(state.signaled)));
                }
            }
        }
        guard.shadow_guard_close(&ram)?;
        for id in &known {
            let mut live = false;
            for present in &current {
                if id == present {
                    live = true;
                }
            }
            if !live {
                event_need(gamepkg::local_event_state(*id).err() == Some(-4))?;
            }
        }
        output.extend(u64::to_le_bytes(result));
        output.extend(u64::to_le_bytes(known.len() as u64));
        output.extend(states);
        let mut bytes = [0u8; 1024];
        ram.read_bytes(0x740100, &mut bytes)?;
        output.extend(bytes);
    }
    crate::event_workers::event_races(&map_view, &mut ram, window)?;
    crate::event_controls::event_controls(&map_view, &mut ram, window)?;
    services.service_close(&ram)?;
    events::event_close_all(&map_view, &ram)?;
    event_need(map_view.shadow_close(&ram) == Err(-4) && map_view.shadow_closed())?;
    for id in known {
        event_need(gamepkg::local_event_state(id).err() == Some(-4))?;
    }
    gamepkg::local_record_release(ram.call_handle())?;
    gamepkg::local_call(
        4,
        &[
            u64::to_le_bytes(window),
            u64::to_le_bytes(0),
            u64::to_le_bytes(65536),
        ]
        .concat(),
        &mut [],
    )?;
    event_need(output.len() == 256 * 1232)?;
    Ok(output)
}
