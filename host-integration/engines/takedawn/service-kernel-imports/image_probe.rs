// Checked image -> CPU -> shared allocator -> data -> complete import pipeline.
use crate::bootstrap;
use crate::calls_runtime;
use crate::disc;
use crate::gamepkg;
use crate::image_imports;
use crate::kernel_data;
use crate::kernel_pool;
use crate::probe;
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
        if mode[0] == 5 {
            shape[0] + 65536
        } else {
            shape[0]
        },
        123,
        0,
        &[],
        None,
    )?;
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
                    && kernel_pool::kernel_supported(ordinal).is_err()
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
    let planned = image_imports::imports_image(&ram, &image, &data, &[]);
    if mode[0] >= 2 {
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
        if prepared.imports_layout() != [table, 3, 5] {
            return Err(-3);
        }
        prepared.imports_publish(&mut ram)?;
        if ram.read_u32(table)? != 0xfe000000
            || ram.read_u32(table + 4)? != data.data_address(&ram, 16)?
            || ram.read_u32(table - 4)? != 0xfe000010
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
        gamepkg::diagnostic(
            1,
            0x544b0f00,
            &[
                u64::from(mode[0]),
                u64::from(table),
                3,
                5,
                u64::from(state.eax),
            ],
        )?;
    }
    probe::imports_cleanup(&mut ram, job)?;
    Ok(())
}
