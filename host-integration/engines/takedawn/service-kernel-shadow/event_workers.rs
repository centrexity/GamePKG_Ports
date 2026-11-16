use crate::events;
use crate::gamepkg;
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
fn event_worker_inner(context: u64) -> Result<u64, i32> {
    let (_, bytes) = gamepkg::local_record_snapshot(context, 64)?;
    event_need(bytes.len() == 64)?;
    let mut ram = memory::Memory::with_call_handle(event_word(&bytes, 0)?)?;
    ram.attach(0x740000, 65536, event_word(&bytes, 8)?, 0, false)?;
    let map_view = shadow::Shadow::shadow_borrow(&ram, event_word(&bytes, 16)?)?;
    gamepkg::local_mailbox_send(event_word(&bytes, 40)?, &u64::to_le_bytes(1))?;
    gamepkg::local_event_wait(event_word(&bytes, 32)?, 1000)?;
    let id = events::event_for(&map_view, &ram, event_word(&bytes, 48)? as u32)?.ok_or(-3)?;
    let status = if event_word(&bytes, 56)? != 0 {
        match gamepkg::local_event_wait(id, 0) {
            Ok(()) => 0,
            Err(-2) => -2,
            Err(code) => return Err(code),
        }
    } else {
        0
    };
    gamepkg::local_mailbox_send(
        event_word(&bytes, 24)?,
        &[u64::to_le_bytes(id), u64::to_le_bytes(status as i64 as u64)].concat(),
    )?;
    Ok(0)
}
pub fn event_worker(context: u64) -> u64 {
    match event_worker_inner(context) {
        Ok(value) => value,
        Err(code) => code as i64 as u64,
    }
}
pub fn event_races(
    map_view: &shadow::Shadow,
    ram: &mut memory::Memory,
    window: u64,
) -> Result<(), i32> {
    for auto in [false, true] {
        let guest = if auto { 0x74e040u32 } else { 0x74e000 };
        ram.fill(guest, 0, 16)?;
        ram.write_u8(guest, u8::from(auto))?;
        ram.write_u32(guest + 4, 1)?;
        let mut out = [0u8; 8];
        gamepkg::local_call(
            20,
            &[u32::to_le_bytes(1), u32::to_le_bytes(0)].concat(),
            &mut out,
        )?;
        let go = u64::from_le_bytes(out);
        let replies = gamepkg::local_mailbox_create(16, 8)?;
        let ready = gamepkg::local_mailbox_create(8, 8)?;
        let context = gamepkg::local_record_create(
            &[
                u64::to_le_bytes(ram.call_handle()),
                u64::to_le_bytes(window),
                u64::to_le_bytes(map_view.shadow_id()),
                u64::to_le_bytes(replies),
                u64::to_le_bytes(go),
                u64::to_le_bytes(ready),
                u64::to_le_bytes(u64::from(guest)),
                u64::to_le_bytes(u64::from(auto)),
            ]
            .concat(),
        )?;
        let mut jobs = std::vec::Vec::new();
        for _ in 0..6u32 {
            let mut worker = [0u8; 8];
            gamepkg::local_call(
                10,
                &[
                    u32::to_le_bytes(0).as_slice(),
                    &u64::to_le_bytes(context),
                    &u32::to_le_bytes(0),
                ]
                .concat(),
                &mut worker,
            )?;
            jobs.push(u64::from_le_bytes(worker));
        }
        for _ in 0..6u32 {
            let mut message = [0u8; 8];
            event_need(gamepkg::local_mailbox_receive(ready, 1000, &mut message)? == 8)?;
        }
        gamepkg::local_call(
            22,
            &[u64::to_le_bytes(go).as_slice(), &u32::to_le_bytes(1)].concat(),
            &mut [],
        )?;
        let mut selected = 0u64;
        let mut consumed = 0u32;
        for _ in 0..6u32 {
            let mut message = [0u8; 16];
            event_need(gamepkg::local_mailbox_receive(replies, 1000, &mut message)? == 16)?;
            let id = event_word(&message, 0)?;
            if selected == 0 {
                selected = id;
            }
            event_need(id == selected)?;
            let status = event_word(&message, 8)? as i64;
            if status == 0 {
                consumed += 1;
            } else {
                event_need(status == -2 && auto)?;
            }
        }
        event_need(consumed == if auto { 1 } else { 6 })?;
        let state = gamepkg::local_event_state(selected)?;
        event_need(state.manual == !auto && state.signaled == !auto)?;
        for job in jobs {
            for _ in 0..1000u32 {
                if gamepkg::local_task_finished(job)? {
                    break;
                }
                memory::Memory::call_retry_pause()?;
            }
            let mut value = [0u8; 8];
            event_need(
                gamepkg::local_call(11, &u64::to_le_bytes(job), &mut value)? == 8
                    && value == [0u8; 8],
            )?;
        }
        gamepkg::local_record_release(context)?;
        gamepkg::local_mailbox_release(replies)?;
        gamepkg::local_mailbox_release(ready)?;
        gamepkg::local_call(25, &u64::to_le_bytes(go), &mut [])?;
    }
    Ok(())
}
