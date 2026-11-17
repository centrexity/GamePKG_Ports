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
        let mut domain = events::Events::events_new(&ram, &map).unwrap();
        let mut incoming = domain.events_begin(&ram, &map).unwrap();
        incoming.event_create(&ram, 0, 1).unwrap();
        let id = incoming.event_creation_id();
        if mode == 1 || mode == 2 || mode == 6 {
            gamepkg::fault_arm(mode, domain.events_id(), window);
            assert_eq!(
                incoming.event_publish(&mut ram, 0x10000),
                Err(if mode == 6 { -4 } else { -3 })
            );
            assert_eq!(
                incoming.event_creation_phase(),
                if mode == 1 { 1 } else { 2 }
            );
            assert!(gamepkg::fault_done());
            incoming.event_creation_close(&ram).unwrap();
            assert_eq!(gamepkg::local_event_state(id).is_ok(), mode != 1);
        } else if mode == 5 {
            gamepkg::fault_arm(mode, domain.events_id(), window);
            assert_eq!(incoming.event_creation_close(&ram), Err(-3));
            assert!(gamepkg::fault_done());
            assert!(!incoming.event_creation_closed());
            assert_eq!(gamepkg::local_event_state(id).err(), Some(-4));
            assert_eq!(domain.events_close(&ram, &map), Err(-2));
            incoming.event_creation_close(&ram).unwrap();
        } else {
            let token = incoming.event_publish(&mut ram, 0x10000).unwrap();
            incoming.event_creation_close(&ram).unwrap();
            gamepkg::fault_arm(mode, domain.events_id(), window);
            assert_eq!(domain.events_remove(&ram, &map, token), Err(-3));
            assert!(gamepkg::fault_done());
            assert_eq!(gamepkg::local_event_state(id).err(), Some(-4));
            assert!(domain.events_state(&ram, &map, token).is_err());
        }
        assert_eq!(
            domain.events_close(&ram, &map),
            if mode == 6 { Err(-4) } else { Ok(()) }
        );
        assert!(domain.events_closed());
        assert_eq!(gamepkg::local_event_state(id).err(), Some(-4));
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
        let mut domain = events::Events::events_new(&ram, &map).unwrap();
        let mut incoming = domain.events_begin(&ram, &map).unwrap();
        incoming.event_create(&ram, 0, 0).unwrap();
        let id = incoming.event_creation_id();
        let token = incoming.event_publish(&mut ram, 0x10000).unwrap();
        incoming.event_creation_close(&ram).unwrap();
        let mut lease = domain.events_wait_begin(&ram, &map, token).unwrap();
        let record = lease.event_wait_id();
        if mode == 7 {
            gamepkg::fault_arm(mode, domain.events_id(), window);
            assert_eq!(lease.event_wait(&ram, 0), Err(-3));
            assert!(gamepkg::fault_done());
            assert!(!lease.event_wait_published());
            lease.event_wait_close(&ram).unwrap();
            assert_eq!(gamepkg::local_record_snapshot(record, 80).err(), Some(-4));
        } else if mode == 8 {
            gamepkg::fault_arm(mode, record, window);
            assert_eq!(lease.event_wait(&ram, 0), Err(-3));
            assert!(gamepkg::fault_done());
            assert!(lease.event_wait_published());
            assert_eq!(domain.events_remove(&ram, &map, token), Err(-2));
            assert!(gamepkg::local_event_state(id).is_ok());
            lease.event_wait_close(&ram).unwrap();
            let (_, w) = gamepkg::local_record_snapshot(record, 80).unwrap();
            assert_eq!(
                u64::from_le_bytes(w[64..72].try_into().unwrap()),
                (-2i64) as u64
            );
        } else {
            assert_eq!(lease.event_wait(&ram, 0), Err(-2));
            lease.event_wait_close(&ram).unwrap();
            gamepkg::fault_arm(mode, domain.events_id(), window);
            assert_eq!(domain.events_remove(&ram, &map, token), Err(-3));
            assert!(gamepkg::fault_done());
            assert!(gamepkg::local_event_state(id).is_ok());
            let (_, b) = gamepkg::local_record_snapshot(domain.events_id(), 2624).unwrap();
            assert_eq!(
                u64::from_le_bytes(b[2120..2128].try_into().unwrap()),
                if mode == 9 { 1 } else { 2 }
            );
            assert_eq!(
                gamepkg::local_record_snapshot(record, 80).is_ok(),
                mode == 9
            );
        }
        domain.events_remove(&ram, &map, token).unwrap();
        assert_eq!(gamepkg::local_record_snapshot(record, 80).err(), Some(-4));
        assert_eq!(gamepkg::local_event_state(id).err(), Some(-4));
        domain.events_close(&ram, &map).unwrap();
        map.handles_close(&ram).unwrap();
        services.service_close(&ram).unwrap();
        gamepkg::local_record_release(ram.call_handle()).unwrap();
    }
    local::shutdown();
    println!(
        "PASS ten actual SDK ownership/wait-interference cases; maintainer shim only, not gameplay"
    );
}
