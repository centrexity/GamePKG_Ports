// Maintainer actual-SDK interference; never admitted, even unused.
mod gamepkg {
    pub use crate::client::*;
    static FAULT: std::sync::Mutex<[u64; 6]> = std::sync::Mutex::new([0; 6]);
    static WAITS: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    pub fn fault_arm(mode: u64, root: u64, events: u64, semaphores: u64) {
        *FAULT.lock().unwrap() = [
            mode,
            root,
            events,
            semaphores,
            0,
            if mode == 8 || mode == 12 { 64 } else { 1 },
        ];
    }
    pub fn fault_done() -> bool {
        FAULT.lock().unwrap()[0] == 0
    }
    pub fn fault_last() -> u64 {
        FAULT.lock().unwrap()[4]
    }
    pub fn wait_calls() -> u32 {
        WAITS.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn revoke(root: u64) {
        let ram = crate::memory::Memory::with_call_handle(root).unwrap();
        ram.services_link(ram.services_handle().unwrap(), 0)
            .unwrap();
    }
    pub fn local_wait_many(
        objects: &[LocalWaitObject],
        all: bool,
        ms: u32,
    ) -> Result<LocalWaitResult, i32> {
        WAITS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let mut fault = FAULT.lock().unwrap();
        let mode = fault[0];
        let root = fault[1];
        if mode == 5 || mode == 6 || (20..=24).contains(&mode) || mode == 31 {
            fault[0] = 0;
        }
        drop(fault);
        if (20..=24).contains(&mode) {
            return Err(-((mode - 19) as i32));
        }
        let result = crate::client::local_wait_many(objects, all, ms);
        if mode == 35 && result == Err(-2) {
            FAULT.lock().unwrap()[5] = 2;
        }
        if result.is_ok() && mode == 5 {
            revoke(root);
        }
        if result == Err(-2) && mode == 31 {
            revoke(root);
        }
        if result.is_ok() && mode == 6 {
            return Err(-3);
        }
        result
    }
    pub fn local_record_snapshot(record: u64, maximum: usize) -> Result<(u64, Vec<u8>), i32> {
        let mut fault = FAULT.lock().unwrap();
        if record == fault[1]
            && maximum == 288
            && (fault[0] == 34 || (fault[0] == 35 && fault[5] == 2))
        {
            fault[0] = 0;
            return Err(-2);
        }
        drop(fault);
        crate::client::local_record_snapshot(record, maximum)
    }
    pub fn local_record_compare_exchange(
        record: u64,
        version: u64,
        bytes: &[u8],
    ) -> Result<u64, i32> {
        let mut fault = FAULT.lock().unwrap();
        let mode = fault[0];
        let root = fault[1];
        let magic = if bytes.len() >= 8 {
            u64::from_le_bytes(bytes[..8].try_into().unwrap())
        } else {
            0
        };
        let mut hit = false;
        let mut after_revoke = false;
        if bytes.len() == 2624 {
            let active = u64::from_le_bytes(bytes[2120..2128].try_into().unwrap()) == 1;
            hit =
                active && ((mode == 1 && record == fault[2]) || (mode == 2 && record == fault[3]));
            after_revoke =
                active && ((mode == 9 && record == fault[2]) || (mode == 10 && record == fault[3]));
        } else if bytes.len() == 80 {
            let done = u64::from_le_bytes(bytes[56..64].try_into().unwrap()) == 1;
            hit =
                done && ((mode == 3 && magic == 0x31544c57) || (mode == 4 && magic == 0x314c5753));
            if mode == 33 && done && magic == 0x31544c57 {
                fault[0] = 0;
                return Err(-2);
            }
            let unlocked =
                magic == 0x32525448 && u64::from_le_bytes(bytes[32..40].try_into().unwrap()) == 0;
            hit |= mode == 8 && unlocked;
            after_revoke |= mode == 11 && unlocked;
        }
        if bytes.len() == 152
            && mode == 12
            && record == fault[2]
            && u64::from_le_bytes(bytes[32..40].try_into().unwrap()) == 0
        {
            hit = true;
        }
        if (26..=30).contains(&mode)
            && ((bytes.len() == 80 && magic == 0x32525448)
                || (bytes.len() == 152 && magic == 0x3153484b))
            && u64::from_le_bytes(bytes[32..40].try_into().unwrap()) == 0
        {
            fault[0] = 0;
            fault[4] = record;
            return Err([-1, -3, -4, -5, -6][(mode - 26) as usize]);
        }
        if hit || after_revoke {
            fault[4] = record;
            if (mode == 8 || mode == 12) && fault[5] > 1 {
                fault[5] -= 1;
            } else {
                fault[0] = 0;
                fault[5] = 0;
            }
        }
        drop(fault);
        if hit {
            let (current, old) = crate::client::local_record_snapshot(record, bytes.len())?;
            crate::client::local_record_compare_exchange(record, current, &old)?;
        }
        let result = crate::client::local_record_compare_exchange(record, version, bytes);
        if result.is_ok() && after_revoke {
            revoke(root);
        }
        result
    }
    pub fn local_call(operation: u32, request: &[u8], output: &mut [u8]) -> Result<usize, i32> {
        let mut fault = FAULT.lock().unwrap();
        let staged = if fault[0] == 14 {
            let (_, e) = crate::client::local_record_snapshot(fault[2], 2624)?;
            let (_, s) = crate::client::local_record_snapshot(fault[3], 2624)?;
            u64::from_le_bytes(e[2120..2128].try_into().unwrap()) == 1
                && u64::from_le_bytes(s[2120..2128].try_into().unwrap()) == 1
        } else {
            false
        };
        if operation == 24 && (fault[0] == 13 || (fault[0] == 14 && staged)) {
            fault[0] = 0;
            return Err(-3);
        }
        drop(fault);
        crate::client::local_call(operation, request, output)
    }
    pub fn local_record_release(record: u64) -> Result<(), i32> {
        let mut fault = FAULT.lock().unwrap();
        if fault[0] == 7 {
            let (_, bytes) = crate::client::local_record_snapshot(record, 80)?;
            if bytes.len() == 80 && u64::from_le_bytes(bytes[..8].try_into().unwrap()) == 0x31544c57
            {
                fault[0] = 0;
                fault[4] = record;
                return Err(-3);
            }
        }
        drop(fault);
        crate::client::local_record_release(record)
    }
}
fn main() {
    let mode = std::env::args().nth(1).unwrap().parse::<u64>().unwrap();
    let runtime = local::runtime::Runtime::new(
        &[
            local::runtime::Window {
                address: 0x20000000,
                bytes: 67108864,
            },
            local::runtime::Window {
                address: 0x24000000,
                bytes: 67108864,
            },
            local::runtime::Window {
                address: 0x28000000,
                bytes: 1048576,
            },
            local::runtime::Window {
                address: 0x29000000,
                bytes: 1048576,
            },
        ],
        vec![],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let mut f = setup::Fixture::mixed_fixture_new().unwrap();
    let event = f.mixed_event(false, 1).unwrap();
    let sem = f.mixed_semaphore(1, 2).unwrap();
    let root = f.ram.call_handle();
    let events = f.events.events_id();
    let sems = f.semaphores.semaphores_id();
    let ksv = f.services.service_handle();
    let mut mixed =
        kernel_pool::MixedWait::mixed_wait_new(&f.ram, &f.table, &f.events, &f.semaphores).unwrap();
    if (40..=54).contains(&mode) {
        f.events
            .events_set(&f.ram, &f.table, event[1] as u32, false)
            .unwrap();
        mixed
            .mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32])
            .unwrap();
        assert_eq!(mixed.mixed_wait_slice(&f.ram, true, 0), Err(-2));
        assert_eq!(gamepkg::wait_calls(), 1);
        assert_eq!(controls::mixed_graph(events).unwrap(), [1, 0]);
        assert_eq!(controls::mixed_graph(sems).unwrap(), [1, 0]);
        if mode != 49 && mode != 54 {
            f.events
                .events_set(&f.ram, &f.table, event[1] as u32, true)
                .unwrap();
        }
        let injected = match mode {
            40 => 3,
            41 => 4,
            42 => 5,
            43 => 6,
            44..=48 => mode - 24,
            49 => 31,
            50 => 3,
            51 => 4,
            52 => 33,
            53 => 34,
            _ => 35,
        };
        gamepkg::fault_arm(injected, root, events, sems);
        let completion = if mode >= 50 && mode <= 53 {
            assert_eq!(
                mixed.mixed_wait_timeout(&f.ram),
                Err(if mode == 53 { -2 } else { -3 })
            );
            assert_eq!(gamepkg::wait_calls(), 1);
            -2
        } else {
            let expected = match mode {
                42 | 49 => -4,
                54 => -2,
                44..=48 => -((mode - 43) as i32),
                _ => -3,
            };
            assert_eq!(mixed.mixed_wait_slice(&f.ram, true, 0), Err(expected));
            assert_eq!(gamepkg::wait_calls(), 2);
            if mode == 45 {
                assert_eq!(controls::mixed_graph(events).unwrap(), [1, 0]);
                assert_eq!(controls::mixed_graph(sems).unwrap(), [1, 0]);
                mixed.mixed_wait_timeout(&f.ram).unwrap();
            }
            if mode <= 42 {
                if mode == 42 {
                    -4
                } else {
                    0
                }
            } else {
                expected
            }
        };
        assert!(gamepkg::fault_done());
        let calls = gamepkg::wait_calls();
        // Liveness restored below cannot authorize replay of an already terminal result.
        if mode == 42 || mode == 49 {
            f.ram.services_link(0, ksv).unwrap();
        }
        assert_eq!(mixed.mixed_wait_slice(&f.ram, true, 0), Err(-1));
        assert_eq!(mixed.mixed_wait_timeout(&f.ram), Err(-1));
        assert_eq!(gamepkg::wait_calls(), calls);
        assert_eq!(
            mixed.mixed_wait_native_result(),
            if mode <= 42 {
                Ok(client::LocalWaitResult {
                    index: 0,
                    abandoned: false,
                })
            } else if mode >= 49 {
                Err(-2)
            } else {
                Err(if mode == 43 {
                    -3
                } else {
                    -((mode - 43) as i32)
                })
            }
        );
        mixed.mixed_wait_close(&f.ram).unwrap();
        controls::mixed_completion(events, event[0], completion).unwrap();
        controls::mixed_completion(sems, sem[0], completion).unwrap();
        assert_eq!(
            gamepkg::local_event_state(event[0]).unwrap().signaled,
            mode >= 44 && mode != 49 && mode != 54
        );
        assert_eq!(
            gamepkg::local_semaphore_state(sem[0]).unwrap().count,
            if mode <= 43 { 0 } else { 1 }
        );
        f.events
            .events_remove(&f.ram, &f.table, event[1] as u32)
            .unwrap();
        f.semaphores
            .semaphores_remove(&f.ram, &f.table, sem[1] as u32)
            .unwrap();
        f.mixed_fixture_close().unwrap();
        local::shutdown();
        println!("PASS retained logical slice fault {mode}");
        return;
    }
    if (26..=30).contains(&mode) {
        let expected = [-1, -3, -4, -5, -6][(mode - 26) as usize];
        let mut guard = f.table.handles_lock(&f.ram).unwrap();
        gamepkg::fault_arm(mode, root, events, sems);
        assert_eq!(guard.handles_guard_close(&f.ram), Err(expected));
        assert!(gamepkg::fault_done() && !guard.handles_guard_closed());
        assert_eq!(f.table.handles_close(&f.ram), Err(-2));
        guard.handles_guard_close(&f.ram).unwrap();
        assert!(guard.handles_guard_closed());
        let mut shadow = kernel_pool::Shadow::shadow_new(&f.ram).unwrap();
        let mut guard = shadow.shadow_lock(&f.ram).unwrap();
        gamepkg::fault_arm(mode, root, shadow.shadow_id(), 0);
        assert_eq!(guard.shadow_guard_close(&f.ram), Err(expected));
        assert!(gamepkg::fault_done() && !guard.shadow_guard_closed());
        assert_eq!(shadow.shadow_close(&f.ram), Err(-2));
        guard.shadow_guard_close(&f.ram).unwrap();
        assert!(guard.shadow_guard_closed());
        shadow.shadow_close(&f.ram).unwrap();
        mixed.mixed_wait_close(&f.ram).unwrap();
        f.events
            .events_remove(&f.ram, &f.table, event[1] as u32)
            .unwrap();
        f.semaphores
            .semaphores_remove(&f.ram, &f.table, sem[1] as u32)
            .unwrap();
        f.mixed_fixture_close().unwrap();
        local::shutdown();
        println!("PASS non-stale count error {expected} for both guards");
        return;
    }
    if mode == 12 || mode == 13 {
        let mut shadow = kernel_pool::Shadow::shadow_new(&f.ram).unwrap();
        let mut guard = shadow.shadow_lock(&f.ram).unwrap();
        gamepkg::fault_arm(mode, root, shadow.shadow_id(), 0);
        assert_eq!(
            guard.shadow_guard_close(&f.ram),
            Err(if mode == 12 { -2 } else { -3 })
        );
        assert!(gamepkg::fault_done());
        assert!(!guard.shadow_guard_closed());
        assert_eq!(shadow.shadow_close(&f.ram), Err(-2));
        assert!(!shadow.shadow_closed());
        if mode == 12 {
            let mut other = shadow.shadow_lock(&f.ram).unwrap();
            guard.shadow_guard_close(&f.ram).unwrap();
            assert!(guard.shadow_guard_closed());
            other.shadow_guard_close(&f.ram).unwrap();
        } else {
            guard.shadow_guard_close(&f.ram).unwrap();
            assert!(guard.shadow_guard_closed());
        }
        guard.shadow_guard_close(&f.ram).unwrap();
        shadow.shadow_close(&f.ram).unwrap();
        mixed.mixed_wait_close(&f.ram).unwrap();
        f.events
            .events_remove(&f.ram, &f.table, event[1] as u32)
            .unwrap();
        f.semaphores
            .semaphores_remove(&f.ram, &f.table, sem[1] as u32)
            .unwrap();
        f.mixed_fixture_close().unwrap();
        local::shutdown();
        println!("PASS retained shadow guard fault {mode}");
        return;
    }
    let initial_mode = if mode == 7 { 1 } else { mode };
    if [1, 2, 7, 8, 9, 10, 11, 14].contains(&mode) {
        gamepkg::fault_arm(initial_mode, root, events, sems);
        assert_eq!(
            mixed.mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32]),
            Err(if mode == 8 {
                -2
            } else if mode >= 9 && mode <= 11 {
                -4
            } else {
                -3
            })
        );
        assert!(gamepkg::fault_done());
        assert_eq!(gamepkg::wait_calls(), 0);
        assert!(!mixed.mixed_wait_closed());
        assert_eq!(mixed.mixed_wait_native_result(), Err(-1));
        let event_count = controls::mixed_graph(events).unwrap();
        let sem_count = controls::mixed_graph(sems).unwrap();
        assert_eq!(event_count, [if mode == 1 || mode == 7 { 0 } else { 1 }, 0]);
        assert_eq!(
            sem_count,
            [
                if [8, 10, 11, 14].contains(&mode) {
                    1
                } else {
                    0
                },
                0
            ]
        );
        if mode == 7 {
            gamepkg::fault_arm(7, root, events, sems);
            assert_eq!(mixed.mixed_wait_close(&f.ram), Err(-3));
            assert!(gamepkg::fault_done());
            let record = gamepkg::fault_last();
            assert!(record != 0 && gamepkg::local_record_snapshot(record, 80).is_ok());
            assert!(!mixed.mixed_wait_closed());
            mixed.mixed_wait_close(&f.ram).unwrap();
            assert_eq!(gamepkg::local_record_snapshot(record, 80).err(), Some(-4));
        } else if mode == 8 {
            assert_eq!(f.table.handles_close(&f.ram), Err(-2));
            let mut other = f.table.handles_lock(&f.ram).unwrap();
            mixed.mixed_wait_close(&f.ram).unwrap();
            other.handles_guard_close(&f.ram).unwrap();
        } else if mode == 14 {
            assert_eq!(f.table.handles_close(&f.ram), Err(-2));
            mixed.mixed_wait_close(&f.ram).unwrap();
        } else {
            mixed.mixed_wait_close(&f.ram).unwrap();
        }
        if mode >= 9 && mode <= 11 {
            f.ram.services_link(0, ksv).unwrap();
        }
        if event_count[0] == 1 {
            controls::mixed_completion(events, event[0], -4).unwrap();
        }
        if sem_count[0] == 1 {
            controls::mixed_completion(sems, sem[0], -4).unwrap();
        }
        assert!(gamepkg::local_event_state(event[0]).unwrap().signaled);
        assert_eq!(gamepkg::local_semaphore_state(sem[0]).unwrap().count, 1);
    } else {
        mixed
            .mixed_wait_prepare(&f.ram, &[event[1] as u32, sem[1] as u32])
            .unwrap();
        gamepkg::fault_arm(mode, root, events, sems);
        let expected = if mode == 5 {
            -4
        } else if mode >= 20 {
            -((mode - 19) as i32)
        } else {
            -3
        };
        assert_eq!(mixed.mixed_wait(&f.ram, true, 0), Err(expected));
        assert!(gamepkg::fault_done());
        assert_eq!(gamepkg::wait_calls(), 1);
        assert_eq!(
            mixed.mixed_wait_native_result(),
            if mode == 3 || mode == 4 || mode == 5 {
                Ok(client::LocalWaitResult {
                    index: 0,
                    abandoned: false,
                })
            } else {
                Err(expected)
            }
        );
        if mode == 3 || mode == 4 {
            assert!(!mixed.mixed_wait_closed());
            assert_eq!(
                controls::mixed_graph(if mode == 3 { events } else { sems }).unwrap(),
                [1, 0]
            );
        }
        assert_eq!(mixed.mixed_wait(&f.ram, true, 0), Err(-1));
        assert_eq!(gamepkg::wait_calls(), 1);
        mixed.mixed_wait_close(&f.ram).unwrap();
        if mode == 5 {
            f.ram.services_link(0, ksv).unwrap();
        }
        let completed = if mode == 3 || mode == 4 { 0 } else { expected };
        controls::mixed_completion(events, event[0], completed).unwrap();
        controls::mixed_completion(sems, sem[0], completed).unwrap();
        assert_eq!(
            gamepkg::local_event_state(event[0]).unwrap().signaled,
            mode >= 20
        );
        assert_eq!(
            gamepkg::local_semaphore_state(sem[0]).unwrap().count,
            if mode >= 20 { 1 } else { 0 }
        );
    }
    assert!(mixed.mixed_wait_closed());
    mixed.mixed_wait_close(&f.ram).unwrap();
    assert_eq!(controls::mixed_graph(events).unwrap()[0], 0);
    assert_eq!(controls::mixed_graph(sems).unwrap()[0], 0);
    f.events
        .events_remove(&f.ram, &f.table, event[1] as u32)
        .unwrap();
    f.semaphores
        .semaphores_remove(&f.ram, &f.table, sem[1] as u32)
        .unwrap();
    f.mixed_fixture_close().unwrap();
    local::shutdown();
    println!("PASS mixed ownership actual-SDK fault {mode}; provider never admitted");
}
