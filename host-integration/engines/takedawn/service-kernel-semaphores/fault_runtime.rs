// Maintainer-only interference via actual SDK; never admitted as game source.
mod gamepkg {
    pub use crate::client::*;
    static FAULT: std::sync::Mutex<[u64; 3]> = std::sync::Mutex::new([0; 3]);
    pub fn fault_arm(mode: u64, record: u64, window: u64) {
        *FAULT.lock().unwrap() = [mode, record, window];
    }
    pub fn fault_done() -> bool {
        FAULT.lock().unwrap()[0] == 0
    }
    fn accepted_revoke(mode: u64) {
        let root = {
            let mut fault = FAULT.lock().unwrap();
            if fault[0] != mode {
                return;
            }
            fault[0] = 0;
            fault[1]
        };
        let ram = crate::memory::Memory::with_call_handle(root).unwrap();
        ram.services_link(ram.services_handle().unwrap(), 0)
            .unwrap();
    }
    pub fn local_semaphore_wait(id: u64, ms: u32) -> Result<(), i32> {
        let result = crate::client::local_semaphore_wait(id, ms);
        if result.is_ok() {
            accepted_revoke(11);
        }
        result
    }
    pub fn local_semaphore_release(id: u64, permits: u32) -> Result<u32, i32> {
        let result = crate::client::local_semaphore_release(id, permits);
        if result.is_ok() {
            accepted_revoke(12);
        }
        result
    }
    pub fn local_semaphore_create(initial: u32, maximum: u32) -> Result<u64, i32> {
        let result = crate::client::local_semaphore_create(initial, maximum);
        if result.is_ok() {
            accepted_revoke(13);
        }
        result
    }
    pub fn local_record_compare_exchange(
        record: u64,
        version: u64,
        bytes: &[u8],
    ) -> Result<u64, i32> {
        let mut fault = FAULT.lock().unwrap();
        let mode = fault[0];
        let mut revoke = 0;
        if fault[1] == record && bytes.len() == 2624 {
            let state = u64::from_le_bytes(bytes[72..80].try_into().unwrap());
            if (mode == 1 && state == 2) || ((mode == 4 || mode == 5) && state == 0) {
                let (current, old) = crate::client::local_record_snapshot(record, 2624)?;
                crate::client::local_record_compare_exchange(record, current, &old)?;
                fault[0] = 0;
            } else if mode == 6 && state == 2 {
                revoke = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
                fault[0] = 0;
            }
        }
        if mode >= 7 && fault[1] == record {
            let hit = if bytes.len() == 80 {
                mode == 8 && u64::from_le_bytes(bytes[56..64].try_into().unwrap()) == 1
            } else if bytes.len() == 2624 {
                let slot = u64::from_le_bytes(bytes[2120..2128].try_into().unwrap());
                (mode == 7 && slot == 1) || (mode == 9 && slot == 2) || (mode == 10 && slot == 0)
            } else {
                false
            };
            if hit {
                let (current, old) = crate::client::local_record_snapshot(record, bytes.len())?;
                crate::client::local_record_compare_exchange(record, current, &old)?;
                fault[0] = 0;
            }
        }
        drop(fault);
        let result = crate::client::local_record_compare_exchange(record, version, bytes);
        if result.is_ok() && revoke != 0 {
            let ram = crate::memory::Memory::with_call_handle(revoke).unwrap();
            ram.services_link(ram.services_handle().unwrap(), 0)
                .unwrap();
        }
        result
    }
    pub fn local_memory_compare_exchange(
        window: u64,
        offset: u64,
        expected: &[u8],
        replacement: &[u8],
    ) -> Result<bool, i32> {
        let mut fault = FAULT.lock().unwrap();
        if (fault[0] == 2 || fault[0] == 3)
            && window == fault[2]
            && offset == 0
            && expected.len() == 16384
        {
            let mut other = expected.to_vec();
            let version = u64::from_le_bytes(other[16..24].try_into().unwrap());
            other[16..24].copy_from_slice(&(version + 1).to_le_bytes());
            assert!(crate::client::local_memory_compare_exchange(
                window, offset, expected, &other
            )?);
            fault[0] = 0;
        }
        drop(fault);
        crate::client::local_memory_compare_exchange(window, offset, expected, replacement)
    }
}
fn main() {
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
    for mode in 1..7u64 {
        let mut ram = memory::Memory::new().unwrap();
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
        let mut inventory = [0u8; 64];
        gamepkg::local_call(1, &[], &mut inventory).unwrap();
        let window = u64::from_le_bytes(inventory[48..56].try_into().unwrap());
        let main = u64::from_le_bytes(inventory[..8].try_into().unwrap());
        for (id, bytes) in if mode == 1 {
            vec![(window, 1048576u64), (main, 4096)]
        } else {
            vec![]
        } {
            gamepkg::local_call(
                2,
                &[
                    id.to_le_bytes().as_slice(),
                    &0u64.to_le_bytes(),
                    &bytes.to_le_bytes(),
                    &2u32.to_le_bytes(),
                ]
                .concat(),
                &mut [],
            )
            .unwrap();
        }
        ram.attach(0x10000, 4096, main, 0, true).unwrap();
        let mut map = handles::Handles::handles_new(&ram).unwrap();
        map.handles_bind(&ram, window, 0).unwrap();
        let mut domain = semaphores::Semaphores::semaphores_new(&ram, &map).unwrap();
        let mut incoming = domain.semaphores_begin(&ram, &map).unwrap();
        incoming.semaphore_create(&ram, 0, 1).unwrap();
        let id = incoming.semaphore_creation_id();
        if mode == 1 || mode == 2 || mode == 6 {
            gamepkg::fault_arm(mode, domain.semaphores_id(), window);
            assert_eq!(
                incoming.semaphore_publish(&mut ram, 0x10000),
                Err(if mode == 6 { -4 } else { -3 })
            );
            assert_eq!(
                incoming.semaphore_creation_phase(),
                if mode == 1 { 1 } else { 2 }
            );
            assert!(gamepkg::fault_done());
            incoming.semaphore_creation_close(&ram).unwrap();
            assert_eq!(gamepkg::local_semaphore_state(id).is_ok(), mode != 1);
        } else if mode == 5 {
            gamepkg::fault_arm(mode, domain.semaphores_id(), window);
            assert_eq!(incoming.semaphore_creation_close(&ram), Err(-3));
            assert!(gamepkg::fault_done());
            assert!(!incoming.semaphore_creation_closed());
            assert_eq!(gamepkg::local_semaphore_state(id).err(), Some(-4));
            assert_eq!(domain.semaphores_close(&ram, &map), Err(-2));
            incoming.semaphore_creation_close(&ram).unwrap();
        } else {
            let token = incoming.semaphore_publish(&mut ram, 0x10000).unwrap();
            incoming.semaphore_creation_close(&ram).unwrap();
            gamepkg::fault_arm(mode, domain.semaphores_id(), window);
            assert_eq!(domain.semaphores_remove(&ram, &map, token), Err(-3));
            assert!(gamepkg::fault_done());
            assert_eq!(gamepkg::local_semaphore_state(id).err(), Some(-4));
            assert!(domain.semaphores_state(&ram, &map, token).is_err());
        }
        assert_eq!(
            domain.semaphores_close(&ram, &map),
            if mode == 6 { Err(-4) } else { Ok(()) }
        );
        assert!(domain.semaphores_closed());
        assert_eq!(gamepkg::local_semaphore_state(id).err(), Some(-4));
        assert_eq!(
            map.handles_close(&ram),
            if mode == 6 { Err(-4) } else { Ok(()) }
        );
        assert!(map.handles_closed());
        services.service_close(&ram).unwrap();
        gamepkg::local_record_release(ram.call_handle()).unwrap();
    }
    for mode in 7..11u64 {
        let mut ram = memory::Memory::new().unwrap();
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
        let mut inventory = [0u8; 64];
        gamepkg::local_call(1, &[], &mut inventory).unwrap();
        let window = u64::from_le_bytes(inventory[48..56].try_into().unwrap());
        let main = u64::from_le_bytes(inventory[..8].try_into().unwrap());
        ram.attach(0x10000, 4096, main, 0, true).unwrap();
        let mut map = handles::Handles::handles_new(&ram).unwrap();
        map.handles_bind(&ram, window, 0).unwrap();
        let mut domain = semaphores::Semaphores::semaphores_new(&ram, &map).unwrap();
        let mut incoming = domain.semaphores_begin(&ram, &map).unwrap();
        incoming.semaphore_create(&ram, 0, 1).unwrap();
        let id = incoming.semaphore_creation_id();
        let token = incoming.semaphore_publish(&mut ram, 0x10000).unwrap();
        incoming.semaphore_creation_close(&ram).unwrap();
        let mut lease = domain.semaphores_wait_begin(&ram, &map, token).unwrap();
        let record = lease.semaphore_wait_id();
        if mode == 7 {
            gamepkg::fault_arm(mode, domain.semaphores_id(), window);
            assert_eq!(lease.semaphore_wait(&ram, 0), Err(-3));
            assert!(gamepkg::fault_done());
            assert!(!lease.semaphore_wait_published());
            lease.semaphore_wait_close(&ram).unwrap();
            assert_eq!(gamepkg::local_record_snapshot(record, 80).err(), Some(-4));
        } else if mode == 8 {
            gamepkg::fault_arm(mode, record, window);
            assert_eq!(lease.semaphore_wait(&ram, 0), Err(-3));
            assert!(gamepkg::fault_done());
            assert!(lease.semaphore_wait_published());
            assert_eq!(domain.semaphores_remove(&ram, &map, token), Err(-2));
            assert!(gamepkg::local_semaphore_state(id).is_ok());
            lease.semaphore_wait_close(&ram).unwrap();
            let (_, w) = gamepkg::local_record_snapshot(record, 80).unwrap();
            assert_eq!(
                u64::from_le_bytes(w[64..72].try_into().unwrap()),
                (-2i64) as u64
            );
        } else {
            assert_eq!(lease.semaphore_wait(&ram, 0), Err(-2));
            lease.semaphore_wait_close(&ram).unwrap();
            gamepkg::fault_arm(mode, domain.semaphores_id(), window);
            assert_eq!(domain.semaphores_remove(&ram, &map, token), Err(-3));
            assert!(gamepkg::fault_done());
            assert!(gamepkg::local_semaphore_state(id).is_ok());
            let (_, b) = gamepkg::local_record_snapshot(domain.semaphores_id(), 2624).unwrap();
            assert_eq!(
                u64::from_le_bytes(b[2120..2128].try_into().unwrap()),
                if mode == 9 { 1 } else { 2 }
            );
            assert_eq!(
                gamepkg::local_record_snapshot(record, 80).is_ok(),
                mode == 9
            );
        }
        domain.semaphores_remove(&ram, &map, token).unwrap();
        assert_eq!(gamepkg::local_record_snapshot(record, 80).err(), Some(-4));
        assert_eq!(gamepkg::local_semaphore_state(id).err(), Some(-4));
        domain.semaphores_close(&ram, &map).unwrap();
        map.handles_close(&ram).unwrap();
        services.service_close(&ram).unwrap();
        gamepkg::local_record_release(ram.call_handle()).unwrap();
    }
    for mode in 11..14u64 {
        let mut ram = memory::Memory::new().unwrap();
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
        let mut inventory = [0u8; 64];
        gamepkg::local_call(1, &[], &mut inventory).unwrap();
        let window = u64::from_le_bytes(inventory[48..56].try_into().unwrap());
        let main = u64::from_le_bytes(inventory[..8].try_into().unwrap());
        ram.attach(0x10000, 4096, main, 0, true).unwrap();
        let mut map = handles::Handles::handles_new(&ram).unwrap();
        map.handles_bind(&ram, window, 0).unwrap();
        let mut domain = semaphores::Semaphores::semaphores_new(&ram, &map).unwrap();
        let mut incoming = domain.semaphores_begin(&ram, &map).unwrap();
        let id;
        if mode == 13 {
            gamepkg::fault_arm(mode, ram.call_handle(), window);
            assert_eq!(incoming.semaphore_create(&ram, 0, 2), Err(-4));
            assert!(gamepkg::fault_done());
            id = incoming.semaphore_creation_id();
            assert_ne!(id, 0);
            assert_eq!(incoming.semaphore_creation_phase(), 1);
            assert_eq!(gamepkg::local_semaphore_state(id).unwrap().count, 0);
            incoming.semaphore_creation_close(&ram).unwrap();
            assert_eq!(gamepkg::local_semaphore_state(id), Err(-4));
        } else {
            incoming.semaphore_create(&ram, 0, 2).unwrap();
            id = incoming.semaphore_creation_id();
            let token = incoming.semaphore_publish(&mut ram, 0x10000).unwrap();
            incoming.semaphore_creation_close(&ram).unwrap();
            if mode == 11 {
                assert_eq!(domain.semaphores_release(&ram, &map, token, 1), Ok(0));
                let mut lease = domain.semaphores_wait_begin(&ram, &map, token).unwrap();
                let record = lease.semaphore_wait_id();
                gamepkg::fault_arm(mode, ram.call_handle(), window);
                assert_eq!(lease.semaphore_wait(&ram, 0), Err(-4));
                assert!(gamepkg::fault_done());
                assert_eq!(gamepkg::local_semaphore_state(id).unwrap().count, 0);
                lease.semaphore_wait_close(&ram).unwrap();
                let (_, bytes) = gamepkg::local_record_snapshot(record, 80).unwrap();
                assert_eq!(u64::from_le_bytes(bytes[56..64].try_into().unwrap()), 1);
                assert_eq!(
                    u64::from_le_bytes(bytes[64..72].try_into().unwrap()),
                    (-4i64) as u64
                );
            } else {
                gamepkg::fault_arm(mode, ram.call_handle(), window);
                assert_eq!(domain.semaphores_release(&ram, &map, token, 2), Err(-4));
                assert!(gamepkg::fault_done());
                assert_eq!(gamepkg::local_semaphore_state(id).unwrap().count, 2);
            }
        }
        assert_eq!(domain.semaphores_close(&ram, &map), Err(-4));
        assert!(domain.semaphores_closed());
        assert_eq!(gamepkg::local_semaphore_state(id), Err(-4));
        assert_eq!(map.handles_close(&ram), Err(-4));
        services.service_close(&ram).unwrap();
        gamepkg::local_record_release(ram.call_handle()).unwrap();
    }
    let mut records = Vec::new();
    for _ in 0..64 {
        records.push(gamepkg::local_record_create(&[0u8; 8]).unwrap());
    }
    assert_eq!(gamepkg::local_record_create(&[0u8; 8]), Err(-2));
    for id in records {
        gamepkg::local_record_release(id).unwrap();
    }
    let mut sync = Vec::new();
    for _ in 0..128 {
        sync.push(gamepkg::local_semaphore_create(0, 1).unwrap());
    }
    assert_eq!(gamepkg::local_semaphore_create(0, 1), Err(-2));
    for id in sync {
        gamepkg::local_call(25, &id.to_le_bytes(), &mut []).unwrap();
    }
    local::shutdown();
    println!(
        "PASS thirteen actual SDK semaphore ownership/wait-interference cases; maintainer shim only, not gameplay"
    );
}
