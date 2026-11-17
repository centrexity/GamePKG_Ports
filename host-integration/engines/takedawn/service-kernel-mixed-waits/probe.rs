use crate::gamepkg;
use crate::setup;
fn mixed_put(output: &mut [u8], cursor: &mut usize, value: u32) -> Result<(), i32> {
    output
        .get_mut(*cursor..*cursor + 4)
        .ok_or(-1)?
        .copy_from_slice(&u32::to_le_bytes(value));
    *cursor += 4;
    Ok(())
}
pub fn mixed_vectors() -> Result<std::vec::Vec<u8>, i32> {
    let mut fixture = setup::Fixture::mixed_fixture_new()?;
    let mut output = std::vec::Vec::new();
    output.resize(2560, 0);
    let mut cursor = 0usize;
    for code in 0..128u32 {
        let first = fixture.mixed_event(code & 16 == 0, code & 1)?;
        let second = fixture.mixed_event(code & 16 != 0, (code >> 1) & 1)?;
        let third = fixture.mixed_semaphore(if code & 4 != 0 { 2 } else { 0 }, 3)?;
        let fourth = fixture.mixed_semaphore((code >> 3) & 1, 2)?;
        let tokens = if code & 64 == 0 {
            [
                first[1] as u32,
                third[1] as u32,
                second[1] as u32,
                fourth[1] as u32,
            ]
        } else {
            [
                fourth[1] as u32,
                second[1] as u32,
                third[1] as u32,
                first[1] as u32,
            ]
        };
        let mut mixed = crate::mixed::MixedWait::mixed_wait_new(
            &fixture.ram,
            &fixture.table,
            &fixture.events,
            &fixture.semaphores,
        )?;
        mixed.mixed_wait_prepare(&fixture.ram, &tokens)?;
        setup::mixed_need(
            702,
            mixed.mixed_wait_count() == 4 && !mixed.mixed_wait_closed(),
        )?;
        setup::mixed_need(
            703,
            mixed.mixed_wait_prepare(&fixture.ram, &tokens) == Err(-1),
        )?;
        setup::mixed_need(
            704,
            fixture
                .events
                .events_remove(&fixture.ram, &fixture.table, first[1] as u32)
                == Err(-2)
                && fixture.semaphores.semaphores_remove(
                    &fixture.ram,
                    &fixture.table,
                    third[1] as u32,
                ) == Err(-2),
        )?;
        setup::mixed_need(
            705,
            fixture.events.events_close(&fixture.ram, &fixture.table) == Err(-2)
                && fixture
                    .semaphores
                    .semaphores_close(&fixture.ram, &fixture.table)
                    == Err(-2),
        )?;
        setup::mixed_need(706, mixed.mixed_wait(&fixture.ram, false, 1001) == Err(-1))?;
        let result = mixed.mixed_wait(&fixture.ram, code & 32 != 0, 0);
        setup::mixed_need(707, mixed.mixed_wait_native_result() == result)?;
        mixed_put(
            &mut output,
            &mut cursor,
            match result {
                Ok(value) => {
                    setup::mixed_need(708, !value.abandoned)?;
                    value.index
                }
                Err(code) => code as u32,
            },
        )?;
        setup::mixed_need(709, mixed.mixed_wait(&fixture.ram, false, 0) == Err(-1))?;
        let ordered = if code & 64 == 0 {
            [
                [first[0], fixture.events.events_id()],
                [third[0], fixture.semaphores.semaphores_id()],
                [second[0], fixture.events.events_id()],
                [fourth[0], fixture.semaphores.semaphores_id()],
            ]
        } else {
            [
                [fourth[0], fixture.semaphores.semaphores_id()],
                [second[0], fixture.events.events_id()],
                [third[0], fixture.semaphores.semaphores_id()],
                [first[0], fixture.events.events_id()],
            ]
        };
        for (index, item) in ordered.iter().enumerate() {
            let outcome = match result {
                Ok(value) => {
                    if code & 32 != 0 || value.index as usize == index {
                        0
                    } else {
                        -2
                    }
                }
                Err(code) => code,
            };
            crate::controls::mixed_completion(item[1], item[0], outcome)?;
        }
        mixed.mixed_wait_close(&fixture.ram)?;
        mixed.mixed_wait_close(&fixture.ram)?;
        setup::mixed_need(710, mixed.mixed_wait_closed())?;
        mixed_put(
            &mut output,
            &mut cursor,
            u32::from(
                fixture
                    .events
                    .events_state(&fixture.ram, &fixture.table, first[1] as u32)?[1],
            ),
        )?;
        mixed_put(
            &mut output,
            &mut cursor,
            u32::from(
                fixture
                    .events
                    .events_state(&fixture.ram, &fixture.table, second[1] as u32)?[1],
            ),
        )?;
        mixed_put(
            &mut output,
            &mut cursor,
            fixture
                .semaphores
                .semaphores_state(&fixture.ram, &fixture.table, third[1] as u32)?[0],
        )?;
        mixed_put(
            &mut output,
            &mut cursor,
            fixture
                .semaphores
                .semaphores_state(&fixture.ram, &fixture.table, fourth[1] as u32)?[0],
        )?;
        fixture
            .events
            .events_remove(&fixture.ram, &fixture.table, first[1] as u32)?;
        fixture
            .events
            .events_remove(&fixture.ram, &fixture.table, second[1] as u32)?;
        fixture
            .semaphores
            .semaphores_remove(&fixture.ram, &fixture.table, third[1] as u32)?;
        fixture
            .semaphores
            .semaphores_remove(&fixture.ram, &fixture.table, fourth[1] as u32)?;
        setup::mixed_need(
            711,
            gamepkg::local_event_state(first[0]).err() == Some(-4)
                && gamepkg::local_semaphore_state(third[0]).err() == Some(-4),
        )?;
    }
    setup::mixed_need(712, cursor == 2560)?;
    fixture.mixed_fixture_close()?;
    Ok(output)
}
