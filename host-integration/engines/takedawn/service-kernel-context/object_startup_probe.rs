// Checked synthetic/authenticated startup controls, never a native title entry.
use crate::gamepkg;
use crate::handles;
use crate::kernel_data;
use crate::kernel_pool;
use crate::kernel_regular;
use crate::kernel_startup;
use crate::layout;
use crate::memory;
use crate::typed_events;
use crate::xbe;
fn object_need(stage: i32, ok: bool) -> Result<(), i32> {
    if ok {
        Ok(())
    } else {
        Err(-stage)
    }
}
fn object_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn object_write_byte(window: u64, offset: u64, value: u8) -> Result<(), i32> {
    gamepkg::local_call(
        6,
        &[
            u64::to_le_bytes(window).as_slice(),
            &u64::to_le_bytes(offset),
            &[value],
        ]
        .concat(),
        &mut [],
    )?;
    Ok(())
}
fn object_read_byte(window: u64, offset: u64) -> Result<u8, i32> {
    let mut value = [0u8; 1];
    object_need(
        301,
        gamepkg::local_call(
            5,
            &[
                u64::to_le_bytes(window).as_slice(),
                &u64::to_le_bytes(offset),
                &u32::to_le_bytes(1),
            ]
            .concat(),
            &mut value,
        )? == 1,
    )?;
    Ok(value[0])
}
fn object_native_event() -> Result<u64, i32> {
    let mut out = [0u8; 8];
    object_need(302, gamepkg::local_call(20, &[0u8; 8], &mut out)? == 8)?;
    Ok(u64::from_le_bytes(out))
}
fn object_release_event(id: u64) -> Result<(), i32> {
    gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
    Ok(())
}
fn object_schema(
    ram: &memory::Memory,
    owner: &kernel_startup::Startup,
    mode: u8,
) -> Result<(), i32> {
    let ids = owner.startup_ids(ram)?;
    let binding = kernel_pool::KernelContext::kernel_context_borrow(ram, ids[2])?;
    let pair = binding.kernel_context_objects(ram)?;
    let (map, domain) = owner.startup_objects(ram)?;
    object_need(303, pair == [map.handles_id(), domain.events_id()])?;
    let (_, original) = gamepkg::local_record_snapshot(ids[2], 80)?;
    object_need(
        304,
        original.len() == 80 && object_word(&original, 0)? == 0x3343494b,
    )?;
    if mode == 64 {
        let mut alternate = handles::Handles::handles_new(ram)?;
        alternate.handles_bind(
            ram,
            layout::Layout::layout_services(ram)?.layout_scratch()?,
            262144,
        )?;
        let mut substitute = typed_events::Events::events_new(ram, &alternate)?;
        let (version, mut b) = gamepkg::local_record_snapshot(ids[2], 80)?;
        b[40..48].copy_from_slice(&u64::to_le_bytes(alternate.handles_id()));
        b[48..56].copy_from_slice(&u64::to_le_bytes(substitute.events_id()));
        gamepkg::local_record_compare_exchange(ids[2], version, &b)?;
        object_need(
            305,
            kernel_pool::KernelContext::kernel_context_borrow(ram, ids[2]).is_ok()
                && owner.startup_ids(ram) == Err(-4)
                && owner.startup_objects(ram).is_err(),
        )?;
        let (version, _) = gamepkg::local_record_snapshot(ids[2], 80)?;
        gamepkg::local_record_compare_exchange(ids[2], version, &original)?;
        substitute.events_close(ram, &alternate)?;
        alternate.handles_close(ram)?;
    }
    if mode == 65 {
        let registry = binding.kernel_context_registry(ram)?;
        let object_shadow = binding.kernel_context_shadow(ram)?;
        let mut legacy = kernel_pool::KernelContext::kernel_context_new(ram, &registry)?;
        let mut extended = kernel_pool::KernelContext::kernel_context_new_with_shadow(
            ram,
            &registry,
            &object_shadow,
        )?;
        object_need(
            306,
            legacy.kernel_context_objects(ram) == Err(-5)
                && extended.kernel_context_objects(ram) == Err(-5),
        )?;
        let (version, mut b) = gamepkg::local_record_snapshot(extended.kernel_context_id(), 72)?;
        b[40..48].copy_from_slice(&u64::to_le_bytes(map.handles_id()));
        gamepkg::local_record_compare_exchange(extended.kernel_context_id(), version, &b)?;
        object_need(
            307,
            kernel_pool::KernelContext::kernel_context_borrow(ram, extended.kernel_context_id())
                .is_err(),
        )?;
        extended.kernel_context_close(ram)?;
        legacy.kernel_context_close(ram)?;
    }
    if mode == 66 {
        for at in [0usize, 8, 16, 24, 32, 40, 48, 56, 64, 72] {
            let (version, mut b) = gamepkg::local_record_snapshot(ids[2], 80)?;
            b[at] ^= 1;
            gamepkg::local_record_compare_exchange(ids[2], version, &b)?;
            object_need(
                308,
                kernel_pool::KernelContext::kernel_context_borrow(ram, ids[2]).is_err(),
            )?;
            let (version, _) = gamepkg::local_record_snapshot(ids[2], 80)?;
            gamepkg::local_record_compare_exchange(ids[2], version, &original)?;
        }
    }
    Ok(())
}
fn object_jobs(ram: &memory::Memory, owner: &mut kernel_startup::Startup) -> Result<u64, i32> {
    let baseline_jobs = owner.startup_jobs();
    let ids = owner.startup_ids(ram)?;
    let argument = gamepkg::local_record_create(
        &[
            u64::to_le_bytes(ram.call_handle()).as_slice(),
            &u64::to_le_bytes(ids[2]),
        ]
        .concat(),
    )?;
    let task = owner.startup_spawn(ram, 5, argument)?;
    object_need(
        309,
        owner.startup_close(&mut ram.call_view()) == Err(-2) && owner.startup_ids(ram)? == ids,
    )?;
    let mut resumed = [0u8; 4];
    object_need(
        330,
        gamepkg::local_call(13, &u64::to_le_bytes(task), &mut resumed)? == 4
            && u32::from_le_bytes(resumed) == 1,
    )?;
    let mut finished = false;
    for _ in 0..4096u32 {
        if gamepkg::local_task_finished(task)? {
            finished = true;
            break;
        }
        memory::Memory::call_retry_pause()?;
    }
    object_need(
        310,
        finished
            && owner.startup_close(&mut ram.call_view()) == Err(-2)
            && owner.startup_jobs() == baseline_jobs + 1,
    )?;
    let proof = owner.startup_join_task(ram, task)?;
    let id = proof.result();
    object_need(
        311,
        id != 0 && gamepkg::local_event_state(id).is_ok() && owner.startup_jobs() == baseline_jobs,
    )?;
    object_need(312, owner.startup_accept_join(ram, &proof) == Err(-4))?;
    gamepkg::local_record_release(argument)?;
    Ok(id)
}
pub fn object_startup_image(
    ram: &mut memory::Memory,
    image: &xbe::Image,
    data: &kernel_data::Data,
    mode: u8,
) -> Result<(), i32> {
    let topology = layout::Layout::layout_services(ram)?;
    let scratch = topology.layout_scratch()?;
    object_need(313, layout::Layout::layout_current(ram).is_err())?;
    let isolated = topology.layout_view(ram)?;
    object_need(
        314,
        isolated.memory_page_backing(0x29000000)?[0] == topology.layout_banks()[0]
            && isolated.memory_page_backing(0x29000000)?[0] != scratch,
    )?;
    gamepkg::local_call(
        2,
        &[
            u64::to_le_bytes(scratch).as_slice(),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(1048576),
            &u32::to_le_bytes(2),
        ]
        .concat(),
        &mut [],
    )?;
    if mode == 53 {
        gamepkg::local_call(
            3,
            &[
                u64::to_le_bytes(scratch).as_slice(),
                &u64::to_le_bytes(0),
                &u64::to_le_bytes(1048576),
                &u32::to_le_bytes(1),
            ]
            .concat(),
            &mut [],
        )?;
    }
    if mode == 54 {
        object_write_byte(scratch, 5 * 16384, 0x5a)?;
    }
    let mut held = std::vec::Vec::new();
    let mut native = std::vec::Vec::new();
    let mut object_free = 0usize;
    if mode >= 55 && mode <= 60 || mode >= 76 && mode <= 78 {
        for _ in 0..64u32 {
            match gamepkg::local_record_create(&[0u8; 8]) {
                Ok(id) => held.push(id),
                Err(-2) => break,
                Err(code) => return Err(code),
            }
        }
        object_free = if mode >= 76 {
            usize::from(83 - mode)
        } else {
            usize::from(mode - 54)
        };
        for _ in 0..object_free {
            gamepkg::local_record_release(held.pop().ok_or(-3)?)?;
        }
    }
    if mode == 61 {
        for _ in 0..127u32 {
            native.push(object_native_event()?);
        }
    }
    let mut owner = kernel_startup::Startup::startup_new(ram)?;
    let expected = if mode == 52 || mode == 53 {
        -4
    } else if mode == 54 || (mode >= 55 && mode <= 61) || mode >= 76 && mode <= 78 {
        -2
    } else if mode == 67
        || mode == 79
        || mode == 84
        || mode == 85
        || mode == 87
        || mode == 88
        || mode == 91
        || mode == 92
        || mode == 93
        || mode == 95
        || mode == 96
    {
        -5
    } else {
        0
    };
    let before = ram.read_u32(image.image_thunk())?;
    let mut original_file_imports = std::vec::Vec::new();
    if mode >= 91 && mode != 94 {
        original_file_imports.resize(64 + 379 * 4, 0u8);
        ram.read_bytes(image.image_thunk() - 64, &mut original_file_imports)?;
    }
    if mode == 67 || mode == 79 || mode == 84 || mode == 93 || mode == 96 {
        let mut missing = std::vec::Vec::new();
        for index in 0..378u32 {
            let value = ram.read_u32(image.image_thunk() + index * 4)?;
            if value == 0 {
                break;
            }
            if value & 0x80000000 != 0 {
                let ordinal = value & 0x7fffffff;
                if data.data_address(ram, ordinal)? == 0
                    && (mode != 93 && mode != 96
                        || !kernel_pool::kernel_scoped_file_supported(
                            ordinal,
                            if mode == 96 { 3 } else { 2 },
                        ))
                    && (kernel_pool::kernel_supported(ordinal).is_err()
                        || kernel_pool::kernel_vm_ordinal(ordinal)
                            && ram.memory_guest_arena_ready().is_err()
                        || mode == 67 && (ordinal == 193 || ordinal == 222)
                        || mode != 84
                            && mode != 93
                            && mode != 96
                            && (ordinal == 277 || ordinal == 291 || ordinal == 294))
                {
                    missing.push(u64::from(ordinal));
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

    let mut bound_regular = None;
    let result = if mode >= 90 && mode != 91 {
        let regular_profile = mode == 94 || mode == 96;
        let mut boot_save_grant = 0;
        for item in gamepkg::grants()? {
            if item.kind == 2 {
                boot_save_grant = item.id;
            }
        }
        object_need(
            924,
            owner.startup_prepare_with_private_files(
                ram,
                image,
                data,
                &[],
                0x10000,
                2,
                [scratch, 0, image.image_rom_grant()],
            ) == Err(-4)
                && owner.startup_phase() == 0
                && ram.kernel_handle() == 0,
        )?;
        if regular_profile {
            let mut regular_session = kernel_regular::SaveSession::regular_session_launch(
                7,
                boot_save_grant,
                "20261117-120000",
            )?;
            regular_session.regular_session_start()?;
            let storage = regular_session.regular_session_ids()?;
            object_need(
                926,
                owner.startup_prepare_with_regular_files(
                    ram,
                    image,
                    data,
                    &[],
                    0x10000,
                    2,
                    [scratch, 0, storage[0], 0],
                ) == Err(-4)
                    && owner.startup_phase() == 0,
            )?;
            let prepared = owner.startup_prepare_with_regular_files(
                ram,
                image,
                data,
                &[],
                0x10000,
                2,
                [scratch, 0, storage[0], storage[1]],
            );
            bound_regular = Some(regular_session);
            prepared
        } else {
            owner.startup_prepare_with_private_files(
                ram,
                image,
                data,
                &[],
                0x10000,
                2,
                [scratch, 0, boot_save_grant],
            )
        }
    } else if mode == 86 || mode == 88 || mode == 89 || mode == 91 {
        owner.startup_prepare_with_rom(ram, image, data, &[], 0x10000, 2, scratch, 0)
    } else if mode == 83 || mode == 84 || mode == 87 {
        owner.startup_prepare_with_criticals(ram, image, data, &[], 0x10000, 2, scratch, 0)
    } else if mode >= 69 {
        owner.startup_prepare_with_sync_objects(
            ram,
            image,
            data,
            &[],
            0x10000,
            2,
            if mode == 52 {
                scratch | 0x100000000
            } else {
                scratch
            },
            0,
        )
    } else {
        owner.startup_prepare_with_objects(
            ram,
            image,
            data,
            &[],
            0x10000,
            2,
            if mode == 52 {
                scratch | 0x100000000
            } else {
                scratch
            },
            0,
        )
    };
    object_need(
        315,
        result == if expected == 0 { Ok(()) } else { Err(expected) },
    )?;
    let mut owned = std::vec::Vec::new();
    let mut sync_owned = std::vec::Vec::new();
    let mut critical_owned = [0u64; 3];
    if expected == 0 {
        object_need(316, owner.startup_phase() == 2)?;
        if mode >= 69 {
            if mode == 94 {
                crate::sync_startup_probe::protection_fixture(ram, [26, 27])?;
                crate::sync_startup_probe::virtual_startup_fixture(ram, [28, 29, 30])?;
            }
            if mode < 83 {
                crate::sync_startup_probe::sync_startup_schema(ram, &owner, mode)?;
            }
            if mode == 83 {
                let values = crate::critical_image_probe::critical_image_calls(ram, &owner)?;
                critical_owned = [values[2], values[3], values[4]];
                gamepkg::diagnostic(1, 0x544b4300, &[values[0], values[1], 1, 1, 1])?;
            }
            if mode == 86 || mode == 89 || mode == 90 || mode == 94 {
                let values = crate::rom_startup_probe::rom_startup_calls(
                    ram,
                    &owner,
                    image,
                    mode != 86,
                    bound_regular.as_mut(),
                )?;
                gamepkg::diagnostic(1, 0x544b4500, &values)?;
            }
            if mode == 81 {
                let values = crate::rtl_image_probe::rtl_image_calls(ram, [10, 11, 12, 13, 14])?;
                gamepkg::diagnostic(1, 0x544b3f00, &values)?;
            }
            if mode == 82 {
                let (table, native_events, semaphores) = owner.startup_sync_objects(ram)?;
                let values = crate::mixed_image_probe::mixed_image_calls(
                    ram,
                    &table,
                    &native_events,
                    &semaphores,
                    [6, 7, 8, 9],
                )?;
                gamepkg::diagnostic(1, 0x544b4100, &values)?;
            }
            if mode == 70 {
                let (table, _, semaphores) = owner.startup_sync_objects(ram)?;
                crate::semaphore_import_probe::semaphore_import_controls(
                    ram,
                    &table,
                    &semaphores,
                    [8, 9],
                )?;
                let bytes = crate::semaphore_import_probe::semaphore_import_vectors(
                    ram,
                    &table,
                    &semaphores,
                    [8, 9],
                )?;
                gamepkg::diagnostic_bytes(1, 0x544b3c00, &bytes)?;
            }
            sync_owned.push(crate::sync_startup_probe::sync_startup_jobs(
                ram, &mut owner,
            )?);
        } else {
            object_schema(ram, &owner, mode)?;
        }
        if mode == 68 {
            let (map, domain) = owner.startup_objects(ram)?;
            crate::event_import_probe::event_import_controls(ram, &map, &domain, [6, 7])?;
            let bytes =
                crate::event_import_probe::event_import_vectors(ram, &map, &domain, [6, 7])?;
            gamepkg::diagnostic_bytes(1, 0x544b3b00, &bytes)?;
        }

        owned.push(object_jobs(ram, &mut owner)?);
        let (map, domain) = owner.startup_objects(ram)?;
        let mut incoming = domain.events_begin(ram, &map)?;
        incoming.event_create(ram, 0, 1)?;
        owned.push(incoming.event_creation_id());
        let output = ram.read_u32(0x4008)?.checked_add(256).ok_or(-1)?;
        let token = incoming.event_publish(ram, output)?;
        object_need(331, token == 0x48000001 && ram.read_u32(output)? == token)?;
        incoming.event_creation_close(ram)?;
        let alias = domain.events_alias(ram, &map, token, true)?;
        object_need(
            332,
            alias == 0x48000002 && domain.events_state(ram, &map, alias)? == [true, true],
        )?;
        if mode == 62 {
            let mut guard = map.handles_lock(ram)?;
            object_need(
                317,
                owner.startup_close(ram) == Err(-2)
                    && owner.startup_phase() == 4
                    && !owner.startup_closed(),
            )?;
            for id in &owned {
                object_need(318, gamepkg::local_event_state(*id).is_ok())?;
            }
            guard.handles_guard_close(ram)?;
        }
        if mode == 63 {
            let services = ram.services_handle()?;
            ram.services_link(services, 0)?;
        }
    } else {
        object_need(
            319,
            owner.startup_phase() == 3
                && owner.startup_error() == expected
                && owner.startup_ids(ram).is_err()
                && owner.startup_objects(ram).is_err()
                && ram.read_u32(image.image_thunk())? == before,
        )?;
    }
    if mode >= 91 && mode != 94 {
        let mut rejected_file_imports = std::vec::Vec::new();
        rejected_file_imports.resize(64 + 379 * 4, 0u8);
        ram.read_bytes(image.image_thunk() - 64, &mut rejected_file_imports)?;
        object_need(
            925,
            original_file_imports == rejected_file_imports && ram.kernel_handle() == 0,
        )?;
    }
    if mode >= 69 && expected == 0 {
        crate::sync_startup_probe::sync_startup_before_close(ram, &mut owner, mode)?;
    }
    if let Some(regular_session) = bound_regular.as_mut() {
        let task = regular_session.regular_session_ids()?[1];
        if mode == 94 {
            object_need(928, owner.startup_close(ram) == Err(-2))?;
        }
        regular_session.regular_session_stop()?;
        let mut finished = None;
        for _ in 0..4096 {
            if mode == 94 {
                match owner.startup_join_task(ram, task) {
                    Ok(proof) => {
                        regular_session.regular_session_accept_join(&proof)?;
                        finished = regular_session.regular_session_finish()?;
                        break;
                    }
                    Err(-2) => {}
                    Err(code) => return Err(code),
                }
            } else if let Some(status) = regular_session.regular_session_finish()? {
                finished = Some(status);
                break;
            }
            memory::Memory::call_retry_pause()?;
        }
        object_need(927, finished == Some(0))?;
    }
    let close_expected = if mode == 63 || mode == 75 {
        Err(-4)
    } else {
        Ok(())
    };
    object_need(
        320,
        owner.startup_close(ram) == close_expected
            && owner.startup_closed()
            && owner.startup_phase() == 5,
    )?;
    object_need(
        321,
        owner.startup_close(ram) == close_expected
            && owner.startup_objects(ram).is_err()
            && ram.kernel_handle() == 0
            && ram.services_handle()? == 0,
    )?;
    for id in owned {
        object_need(322, gamepkg::local_event_state(id).err() == Some(-4))?;
    }
    for id in sync_owned {
        object_need(333, gamepkg::local_semaphore_state(id).err() == Some(-4))?;
    }
    if mode >= 69 {
        let mut recovered = std::vec::Vec::new();
        for _ in 0..128u32 {
            recovered.push(gamepkg::local_semaphore_create(0, 1)?);
        }
        object_need(334, gamepkg::local_semaphore_create(0, 1) == Err(-2))?;
        for id in recovered {
            gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])?;
        }
    }
    if mode == 54 {
        object_need(323, object_read_byte(scratch, 5 * 16384)? == 0x5a)?;
        object_write_byte(scratch, 5 * 16384, 0)?;
    }
    if mode == 53 {
        gamepkg::local_call(
            3,
            &[
                u64::to_le_bytes(scratch).as_slice(),
                &u64::to_le_bytes(0),
                &u64::to_le_bytes(1048576),
                &u32::to_le_bytes(2),
            ]
            .concat(),
            &mut [],
        )?;
    }
    let mut bytes = std::vec::Vec::new();
    bytes.resize(4096, 0u8);
    for page in 0..64u64 {
        object_need(
            324,
            gamepkg::local_call(
                5,
                &[
                    u64::to_le_bytes(scratch).as_slice(),
                    &u64::to_le_bytes(page * 4096),
                    &u32::to_le_bytes(4096),
                ]
                .concat(),
                &mut bytes,
            )? == 4096,
        )?;
        for value in &bytes {
            object_need(325, *value == 0)?;
        }
    }
    if object_free != 0 {
        let mut recovered = std::vec::Vec::new();
        for _ in 0..64u32 {
            match gamepkg::local_record_create(&[0u8; 8]) {
                Ok(id) => recovered.push(id),
                Err(-2) => break,
                Err(code) => return Err(code),
            }
        }
        object_need(326, recovered.len() == object_free)?;
        for id in recovered {
            gamepkg::local_record_release(id)?;
        }
    }
    if mode == 61 {
        let one = object_native_event()?;
        object_need(327, object_native_event() == Err(-2))?;
        object_release_event(one)?;
    }
    for id in held {
        gamepkg::local_record_release(id)?;
    }
    for id in native {
        object_release_event(id)?;
    }
    // Verify the entire native synchronization budget after all owned cleanup.
    let mut budget = std::vec::Vec::new();
    for _ in 0..128u32 {
        budget.push(object_native_event()?);
    }
    object_need(328, object_native_event() == Err(-2))?;
    for id in budget {
        object_release_event(id)?;
    }
    gamepkg::diagnostic(
        1,
        0x544b0f00,
        &[
            u64::from(mode),
            expected as i64 as u64,
            if mode == 94 || mode == 95 {
                31
            } else if mode == 92 {
                18
            } else if mode == 90 || mode == 91 {
                17
            } else if mode == 89 {
                16
            } else if mode == 88 {
                14
            } else if mode == 83 || mode == 85 || mode == 86 || mode == 87 {
                13
            } else if mode == 81 {
                15
            } else if mode >= 69 {
                10
            } else if mode == 68 {
                8
            } else {
                6
            },
            0,
        ],
    )?;
    if mode == 83 {
        object_need(
            898,
            gamepkg::local_mutex_state(critical_owned[0]) == Err(-4),
        )?;
        object_need(
            899,
            gamepkg::local_record_snapshot(critical_owned[1], 152).is_err()
                && gamepkg::local_record_snapshot(critical_owned[2], 2624).is_err(),
        )?;
    }
    Ok(())
}
fn object_actor(argument: u64) -> Result<u64, i32> {
    let (_, b) = gamepkg::local_record_snapshot(argument, 16)?;
    object_need(329, b.len() == 16)?;
    let mut ram = memory::Memory::with_call_handle(object_word(&b, 0)?)?;
    let context = kernel_pool::KernelContext::kernel_context_borrow(&ram, object_word(&b, 8)?)?;
    let ids = context.kernel_context_objects(&ram)?;
    let map = handles::Handles::handles_borrow(&ram, ids[0])?;
    let domain = typed_events::Events::events_borrow(&ram, &map, ids[1])?;
    let mut incoming = domain.events_begin(&ram, &map)?;
    incoming.event_create(&ram, 0, 1)?;
    let id = incoming.event_creation_id();
    incoming.event_publish(&mut ram, 0)?;
    incoming.event_creation_close(&ram)?;
    Ok(id)
}
pub fn object_worker(argument: u64) -> u64 {
    match object_actor(argument) {
        Ok(id) => id,
        Err(code) => code as i64 as u64,
    }
}
