// Asset-free admitted test game; this is not production engine integration.
use crate::gamepkg;
use crate::worker;
fn fixture_reply(connection: &mut worker::Connection) -> Result<worker::Reply, i32> {
    let end = gamepkg::monotonic()?.0.saturating_add(6000000000);
    loop {
        if let Some(packet) = connection.poll(20)? {
            return Ok(packet);
        }
        if gamepkg::monotonic()?.0 >= end {
            return Err(-2);
        }
    }
}
fn fixture_call(
    connection: &mut worker::Connection,
    operation: u32,
    payload: &[u8],
) -> Result<worker::Reply, i32> {
    let sequence = connection.submit(operation, payload)?;
    let packet = fixture_reply(connection)?;
    if packet.sequence != sequence {
        return Err(-3);
    }
    Ok(packet)
}
fn fixture_ok(
    connection: &mut worker::Connection,
    operation: u32,
    payload: &[u8],
) -> Result<std::vec::Vec<u8>, i32> {
    let packet = fixture_call(connection, operation, payload)?;
    if packet.status != 0 {
        return Err(packet.status);
    }
    Ok(packet.bytes)
}
fn fixture_u64(bytes: &[u8]) -> Result<u64, i32> {
    if bytes.len() != 8 {
        return Err(-1);
    }
    Ok(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}
fn fixture_finish(connection: &mut worker::Connection, expected: u64) -> Result<(), i32> {
    let end = gamepkg::monotonic()?.0.saturating_add(6000000000);
    loop {
        if let Some(value) = connection.finish()? {
            if value == expected {
                return Ok(());
            }
            return Err(-3);
        }
        if gamepkg::monotonic()?.0 >= end {
            return Err(-2);
        }
        let (now, generation) = gamepkg::monotonic()?;
        let _ = gamepkg::wait_until(now.saturating_add(1000000), generation)?;
    }
}
fn fixture_regular_open(
    connection: &mut worker::Connection,
    flags: u32,
    disposition: u32,
    filename: &[u8],
    expected_information: u32,
) -> Result<u64, i32> {
    let fields = [
        u32::to_le_bytes(flags).as_slice(),
        &u32::to_le_bytes(disposition),
        filename,
    ]
    .concat();
    let reply = fixture_ok(connection, worker::OPEN_FILE, &fields)?;
    if reply.len() != 12 || reply[8..] != u32::to_le_bytes(expected_information) {
        return Err(-3);
    }
    let id = fixture_u64(&reply[..8])?;
    if (id == 0) != (expected_information >= 4) {
        return Err(-3);
    }
    Ok(id)
}

fn fixture_regular_files(connection: &mut worker::Connection, grant: u64) -> Result<(), i32> {
    let profile_path = b"UDATA/12345678/profile";
    let new_path = b"UDATA/12345678/TitleMeta.xbx";
    fixture_regular_open(connection, 0, 1, new_path, 5)?;
    fixture_regular_open(connection, 1, 1, new_path, 5)?;
    fixture_regular_open(connection, 1, 2, profile_path, 4)?;
    let old_reader = fixture_regular_open(connection, 0, 1, profile_path, 1)?;
    let mut bytes = [0u8; 8];
    if gamepkg::file_read(old_reader, 0, &mut bytes) != Ok(8) || &bytes != b"old-save" {
        return Err(-3);
    }
    fixture_ok(connection, worker::CLOSE, &u64::to_le_bytes(old_reader))?;

    let writer = fixture_regular_open(connection, 0x403, 3, new_path, 2)?;
    let first_info = fixture_ok(connection, worker::QUERY_FILE, &u64::to_le_bytes(writer))?;
    if first_info.len() != 52
        || first_info[32..48] != [0; 16]
        || first_info[48..52] != [0x24, 0, 0, 0]
    {
        return Err(-3);
    }
    fixture_regular_open(connection, 1, 2, new_path, 4)?;
    fixture_regular_open(connection, 0, 1, new_path, 5)?;
    let peer = fixture_regular_open(connection, 1, 3, new_path, 1)?;
    let write = [
        u64::to_le_bytes(writer).as_slice(),
        &u64::to_le_bytes(0),
        b"contents",
    ]
    .concat();
    fixture_ok(connection, worker::WRITE, &write)?;
    if gamepkg::file_size(peer) != Ok(8)
        || gamepkg::file_exists(grant, "UDATA/12345678/TitleMeta.xbx") != Ok(false)
    {
        return Err(-3);
    }
    let new_length = [u64::to_le_bytes(writer), u64::to_le_bytes(5)].concat();
    fixture_ok(connection, worker::RESIZE, &new_length)?;
    let resized_info = fixture_ok(connection, worker::QUERY_FILE, &u64::to_le_bytes(writer))?;
    if resized_info.len() != 52
        || resized_info[..8] != first_info[..8]
        || resized_info[32..40] != u64::to_le_bytes(16384)
        || resized_info[40..48] != u64::to_le_bytes(5)
        || resized_info[48..52] != [0x24, 0, 0, 0]
    {
        return Err(-3);
    }
    fixture_ok(connection, worker::CLOSE, &u64::to_le_bytes(peer))?;
    fixture_ok(connection, worker::CLOSE, &u64::to_le_bytes(writer))?;
    if fixture_call(connection, worker::QUERY_FILE, &u64::to_le_bytes(writer))?.status != -4 {
        return Err(-3);
    }
    Ok(())
}

pub fn exercise(grant: u64, phase: u8) -> Result<(), i32> {
    if phase == 14 {
        for _ in 0..20 {
            if !matches_launch_failure(grant) {
                return Err(-3);
            }
        }
    }
    let chosen = if phase == 13 {
        grant.saturating_add(999)
    } else {
        grant
    };
    let mut connection = worker::Connection::launch(0, chosen, "12345678", "20261114-120000")?;
    if connection.submit(worker::STATUS, &[]) != Err(-2) {
        return Err(-3);
    }
    let initial = fixture_reply(&mut connection)?;
    if phase == 13 {
        if initial.status != -4 {
            return Err(-3);
        }
        return fixture_finish(&mut connection, 1);
    }
    if initial.status != 0 || initial.bytes.len() != 68 {
        return Err(-3);
    }
    if phase == 15 && initial.bytes[44..48] != [1, 0, 0, 0] {
        return Err(-3);
    }
    if phase == 16 && initial.bytes[48..52] != [1, 0, 0, 0] {
        return Err(-3);
    }
    if phase == 19 {
        for operation in [
            worker::WRITE,
            worker::RESIZE,
            worker::CLOSE,
            worker::DELETE_ON_CLOSE,
        ] {
            if fixture_call(&mut connection, operation, &[0u8; 7])?.status != -1 {
                return Err(-3);
            }
        }
        if fixture_call(
            &mut connection,
            worker::OPEN_WRITE,
            b"UDATA/87654321/profile",
        )?
        .status
            != -4
        {
            return Err(-3);
        }
        if fixture_call(&mut connection, worker::OPEN_WRITE, &[255u8])?.status != -1 {
            return Err(-3);
        }
        if fixture_call(&mut connection, worker::STAMP, b"bad")?.status != -1 {
            return Err(-3);
        }
    }
    if phase == 10 || phase == 11 || phase == 12 || phase == 17 || phase == 18 {
        let handle = fixture_u64(&fixture_ok(
            &mut connection,
            worker::OPEN_WRITE,
            b"UDATA/12345678/profile",
        )?)?;
        let mut payload = std::vec::Vec::new();
        payload.extend(u64::to_le_bytes(handle));
        payload.extend(u64::to_le_bytes(0));
        if phase == 18 {
            payload.extend(b"old-save");
        } else {
            payload.extend(b"new-save");
        }
        let _ = fixture_ok(&mut connection, worker::WRITE, &payload)?;
        if phase == 11 {
            connection.cancel()?;
            return fixture_finish(&mut connection, 2);
        }
        if phase == 17 {
            let _ = fixture_ok(
                &mut connection,
                worker::DELETE_ON_CLOSE,
                &u64::to_le_bytes(handle),
            )?;
        }
        if phase != 12 {
            let _ = fixture_ok(&mut connection, worker::CLOSE, &u64::to_le_bytes(handle))?;
        }
    }
    if phase == 20 {
        fixture_regular_files(&mut connection, grant)?;
    }
    let final_packet = fixture_call(&mut connection, worker::SHUTDOWN, &[])?;
    if final_packet.bytes.len() != 68 {
        return Err(-3);
    }
    if phase == 12 {
        if final_packet.status != -2 || final_packet.bytes[36..40] != [1, 0, 0, 0] {
            return Err(-3);
        }
        fixture_finish(&mut connection, 1)?;
    } else {
        if final_packet.status != 0 {
            return Err(final_packet.status);
        }
        fixture_finish(&mut connection, 0)?;
    }
    Ok(())
}
fn matches_launch_failure(grant: u64) -> bool {
    match worker::Connection::launch(256, grant, "12345678", "20261114-120000") {
        Err(-4) => true,
        _ => false,
    }
}
