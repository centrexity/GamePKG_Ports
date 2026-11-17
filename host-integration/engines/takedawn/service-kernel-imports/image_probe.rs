// Checked image -> CPU -> shared allocator -> data -> complete import pipeline.
use crate::bootstrap;
use crate::calls_runtime;
use crate::disc;
use crate::gamepkg;
use crate::image_imports;
use crate::kernel_data;
use crate::kernel_pool;
use crate::probe;
use crate::startup_probe;
use crate::xbe;
pub fn image_import_run() -> Result<(), i32> {
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
    let expected_file = gamepkg::file_open(assets, false, "expected-sha256")?;
    let mut expected = [0u8; 32];
    if gamepkg::file_read(expected_file, 0, &mut expected)? != 32 {
        return Err(-3);
    }
    gamepkg::file_close(expected_file)?;
    let mode_file = gamepkg::file_open(assets, false, "mode")?;
    let mut mode = [0u8; 1];
    if gamepkg::file_read(mode_file, 0, &mut mode)? != 1 {
        return Err(-3);
    }
    gamepkg::file_close(mode_file)?;
    let (mut ram, banks) = probe::imports_memory()?;
    let indexed = disc::Disc::disc_open(rom)?;
    let file = indexed.disc_find("default.xbe")?;
    let image = xbe::Image::image_plan(&indexed, file, expected)?;
    image.image_load(&mut ram)?;
    let startup = bootstrap::Bootstrap::boot_plan(&image, &ram)?;
    let shape = startup.boot_layout();
    let mut state = startup.boot_apply(&mut ram)?;
    let job = probe::imports_owner(&mut ram, banks, shape[4], true)?;
    let data = kernel_data::Data::data_initialize(
        &mut ram,
        if mode[0] == 5 || mode[0] == 17 {
            shape[0] + 65536
        } else {
            shape[0]
        },
        123,
        0,
        &[],
        None,
    )?;
    if mode[0] >= 12 {
        startup_probe::startup_image(&mut ram, &image, &data, mode[0])?;
        probe::imports_cleanup(&mut ram, job)?;
        return Ok(());
    }
    let services = if mode[0] == 4 || mode[0] == 8 || mode[0] == 10 || mode[0] == 11 {
        Some(kernel_pool::kernel_services_initialize(
            &ram,
            0x10000,
            if mode[0] == 8 || mode[0] == 10 { 6 } else { 2 },
        )?)
    } else {
        None
    };
    let table = image.image_thunk();
    let mut before = std::vec::Vec::new();
    before.resize(64 + 379 * 4, 0u8);
    ram.read_bytes(table - 64, &mut before)?;
    if mode[0] == 3 {
        ram.call_bounds(0, 0)?;
    }
    if mode[0] == 4 {
        let mut missing = std::vec::Vec::new();
        for index in 0..378u32 {
            let current = ram.read_u32(table + index * 4)?;
            if current == 0 {
                break;
            }
            if current & 0x80000000 != 0 {
                let ordinal = current & 0x7fffffff;
                if data.data_address(&ram, ordinal)? == 0
                    && (kernel_pool::kernel_supported(ordinal).is_err()
                        || kernel_pool::kernel_requires_context(ordinal))
                {
                    missing.extend([u64::from(ordinal)]);
                    if missing.len() == 32 {
                        gamepkg::diagnostic(1, 0x544b0f10, &missing)?;
                        missing.clear();
                    }
                }
            }
        }
        if !missing.is_empty() {
            gamepkg::diagnostic(1, 0x544b0f10, &missing)?;
        }
    }
    let mut registry = if mode[0] == 11 {
        Some(kernel_pool::Connections::connections_new(&ram)?)
    } else {
        None
    };
    let mut binding = if let Some(owner) = registry.as_ref() {
        Some(kernel_pool::KernelContext::kernel_context_new(&ram, owner)?)
    } else {
        None
    };
    let planned = if let Some(owner) = binding.as_ref() {
        image_imports::imports_image_with_context(&ram, &image, &data, &[], owner)
    } else {
        image_imports::imports_image(&ram, &image, &data, &[])
    };
    if mode[0] >= 2
        && mode[0] != 6
        && mode[0] != 7
        && mode[0] != 8
        && mode[0] != 10
        && mode[0] != 11
    {
        let expected = if mode[0] == 3 || mode[0] == 5 { -1 } else { -5 };
        match planned {
            Err(code) => {
                if code != expected {
                    return Err(-3);
                }
            }
            Ok(_) => return Err(-3),
        }
        let mut after = std::vec::Vec::new();
        after.resize(before.len(), 0u8);
        ram.read_bytes(table - 64, &mut after)?;
        if before != after || ram.kernel_handle() != 0 {
            return Err(-3);
        }
        gamepkg::diagnostic(1, 0x544b0f00, &[u64::from(mode[0]), expected as i64 as u64])?;
    } else {
        let prepared = planned?;
        let count = if mode[0] == 11 {
            5
        } else if mode[0] == 10 {
            14
        } else if mode[0] == 8 {
            10
        } else if mode[0] == 7 {
            9
        } else if mode[0] == 6 {
            6
        } else {
            3
        };
        if prepared.imports_layout() != [table, count, count + 2] {
            return Err(-3);
        }
        prepared.imports_publish(&mut ram)?;
        if ram.read_u32(table)? != 0xfe000000
            || ram.read_u32(table + 4)? != data.data_address(&ram, 16)?
            || ram.read_u32(table - 4)? != 0xfe000000 + (count + 1) * 4
        {
            return Err(-3);
        }
        let sp = state.esp;
        ram.write_u32(sp, 0x12345678)?;
        ram.write_u32(sp + 4, 64)?;
        calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000000, sp)?;
        if state.eax != shape[4] || state.esp != sp + 8 {
            return Err(-3);
        }
        let allocation = state.eax;
        if mode[0] == 11 {
            for (slot, args) in [
                (3u32, [0x1d000u32, 0x1100, 0x89abcdef, 3, 16, 1, 0]),
                (4, [0x1d000, 0, 0, 0, 0, 0, 0]),
            ] {
                state.esp = sp;
                ram.write_u32(sp, 0x12345678)?;
                for at in 0..7usize {
                    ram.write_u32(sp + 4 + at as u32 * 4, args[at])?;
                }
                calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000000 + slot * 4, sp)?;
                if state.eax != u32::from(slot == 4)
                    || state.esp
                        != sp + 4 + kernel_pool::kernel_supported(if slot == 3 { 109 } else { 98 })?
                {
                    return Err(-3);
                }
            }
            if registry.as_ref().ok_or(-3)?.connection_get(&ram, 3)? != 0x1d000 {
                return Err(-3);
            }
        }

        if mode[0] == 6 || mode[0] == 7 || mode[0] == 8 || mode[0] == 10 {
            ram.write_bytes(0x13000, &[97u8, 98, 99])?;
            for (slot, args) in [
                (3u32, [0x14003u32, 0, 0]),
                (4, [0x14003, 0x13000, 3]),
                (5, [0x14003, 0x15000, 0]),
            ] {
                state.esp = sp;
                ram.write_u32(sp, 0x12345678)?;
                for index in 0..3usize {
                    ram.write_u32(sp + 4 + index as u32 * 4, args[index])?;
                }
                calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000000 + slot * 4, sp)?;
                if state.eax != 0
                    || state.esp != sp + 4 + kernel_pool::kernel_supported(332 + slot)?
                {
                    return Err(-3);
                }
            }
            let mut digest = [0u8; 20];
            ram.read_bytes(0x15000, &mut digest)?;
            if digest
                != [
                    0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78,
                    0x50, 0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
                ]
            {
                return Err(-3);
            }
        }
        if mode[0] == 7 || mode[0] == 8 || mode[0] == 10 {
            ram.write_bytes(0x16000, &[75u8, 101, 121])?;
            ram.write_bytes(0x17000, &[80u8, 108, 97, 105, 110, 116, 101, 120, 116])?;
            ram.fill(0x18000, 0x0b, 20)?;
            ram.write_bytes(0x19000, &[72u8, 105, 32])?;
            ram.write_bytes(0x1a000, &[84u8, 104, 101, 114, 101])?;
            let crypto_sp = sp - 64;
            for (slot, args) in [
                (6u32, [0x1b003u32, 3, 0x16000, 0, 0, 0, 0]),
                (7, [0x1b003, 9, 0x17000, 0, 0, 0, 0]),
                (8, [0x18000, 20, 0x19000, 3, 0x1a000, 5, 0x1c000]),
            ] {
                state.esp = crypto_sp;
                ram.write_u32(crypto_sp, 0x12345678)?;
                for index in 0..7usize {
                    ram.write_u32(crypto_sp + 4 + index as u32 * 4, args[index])?;
                }
                calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000000 + slot * 4, crypto_sp)?;
                if state.eax != 0
                    || state.esp != crypto_sp + 4 + kernel_pool::kernel_supported(332 + slot)?
                {
                    return Err(-3);
                }
            }
            let mut cipher = [0u8; 9];
            ram.read_bytes(0x17000, &mut cipher)?;
            if cipher != [0xbb, 0xf3, 0x16, 0xe8, 0xd9, 0x40, 0xaf, 0x0a, 0xd3] {
                return Err(-3);
            }
            let mut digest = [0u8; 20];
            ram.read_bytes(0x1c000, &mut digest)?;
            if digest
                != [
                    0xb6, 0x17, 0x31, 0x86, 0x55, 0x05, 0x72, 0x64, 0xe2, 0x8b, 0xc0, 0xb6, 0xfb,
                    0x37, 0x8c, 0x8e, 0xf1, 0x46, 0xbe, 0x00,
                ]
            {
                return Err(-3);
            }
        }
        if mode[0] == 8 || mode[0] == 10 {
            let query_sp = sp - 64;
            for (index, expected) in [(8u32, 0x10000u32), (9, 0x10002)] {
                state.esp = query_sp;
                ram.write_u32(query_sp, 0x12345678)?;
                for (at, arg) in [index, 0x1d000, 0x1e000, 4, 0x1f000].iter().enumerate() {
                    ram.write_u32(query_sp + 4 + at as u32 * 4, *arg)?;
                }
                calls_runtime::invoke_call(&mut state, &mut ram, 0xfe000024, query_sp)?;
                if state.eax != 0
                    || state.esp != query_sp + 24
                    || ram.read_u32(0x1e000)? != expected
                    || ram.read_u32(0x1d000)? != 4
                    || ram.read_u32(0x1f000)? != 4
                {
                    return Err(-3);
                }
            }
        }
        if mode[0] == 10 {
            let timer_sp = sp - 96;
            for ordinal in [125u32, 126, 127, 128] {
                state.esp = timer_sp;
                ram.write_u32(timer_sp, 0x12345678)?;
                ram.write_u32(timer_sp + 4, 0x1f010)?;
                let before_cpu = state.encode();
                let before = gamepkg::clock_sample()?;
                calls_runtime::invoke_call(
                    &mut state,
                    &mut ram,
                    0xfe000028 + (ordinal - 125) * 4,
                    timer_sp,
                )?;
                let after = gamepkg::clock_sample()?;
                let value = if ordinal == 128 {
                    ram.read_u64(0x1f010)?
                } else {
                    u64::from(state.eax) | (u64::from(state.edx) << 32)
                };
                let low_base = if ordinal == 125 {
                    before.uptime_ms
                } else if ordinal == 126 {
                    before.counter
                } else if ordinal == 127 {
                    before.frequency
                } else {
                    before.utc_100ns
                };
                let high_base = if ordinal == 125 {
                    after.uptime_ms
                } else if ordinal == 126 {
                    after.counter
                } else if ordinal == 127 {
                    after.frequency
                } else {
                    after.utc_100ns
                };
                let low = kernel_pool::kernel_time_value(ordinal, low_base, before.frequency, 0)?;
                let high = kernel_pool::kernel_time_value(ordinal, high_base, after.frequency, 0)?;
                if value < low || value > high {
                    return Err(-3);
                }
                let mut expected_cpu = std::vec::Vec::new();
                expected_cpu.extend(before_cpu);
                expected_cpu[..4].copy_from_slice(&u32::to_le_bytes(if ordinal == 128 {
                    0
                } else {
                    value as u32
                }));
                if ordinal != 128 {
                    expected_cpu[12..16].copy_from_slice(&u32::to_le_bytes((value >> 32) as u32));
                }
                expected_cpu[28..32].copy_from_slice(&u32::to_le_bytes(
                    timer_sp + 4 + kernel_pool::kernel_supported(ordinal)?,
                ));
                if state.encode() != expected_cpu {
                    return Err(-3);
                }
            }
        }
        gamepkg::diagnostic(
            1,
            0x544b0f00,
            &[
                u64::from(mode[0]),
                u64::from(table),
                u64::from(count),
                u64::from(count + 2),
                u64::from(allocation),
            ],
        )?;
    }
    if let Some(owner) = binding.as_mut() {
        ram.kernel_clear()?;
        owner.kernel_context_close(&ram)?;
    }
    if let Some(owner) = registry.as_mut() {
        owner.connections_close(&ram)?;
    }
    if let Some(owner) = services {
        owner.service_close(&ram)?;
    }
    probe::imports_cleanup(&mut ram, job)?;
    Ok(())
}
