// Maintainer-only deterministic interference through actual SDK calls.
// Never injected into game objects; packaged even unused, this shim must reject.
mod gamepkg {
    pub use crate::client::*;
    static FAULT: std::sync::Mutex<[u64; 4]> = std::sync::Mutex::new([0; 4]);
    pub fn fault_arm(mode: u64, record: u64, window: u64, offset: u64) {
        *FAULT.lock().unwrap() = [mode, record, window, offset];
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
        if fault[0] == 1
            && fault[1] == record
            && bytes.len() == 80
            && u64::from_le_bytes(bytes[64..72].try_into().unwrap()) == 3
        {
            let (current, old) = crate::client::local_record_snapshot(record, 80)?;
            crate::client::local_record_compare_exchange(record, current, &old)?;
            fault[0] = 0;
        }
        drop(fault);
        crate::client::local_record_compare_exchange(record, version, bytes)
    }
    fn revoke(root: u64) {
        let ram = crate::memory::Memory::with_call_handle(root).unwrap();
        let services = ram.services_handle().unwrap();
        ram.services_link(services, 0).unwrap();
    }
    pub fn local_call(operation: u32, request: &[u8], output: &mut [u8]) -> Result<usize, i32> {
        let result = crate::client::local_call(operation, request, output)?;
        let mut fault = FAULT.lock().unwrap();
        let mut root = 0;
        if fault[0] == 3
            && operation == 5
            && request.len() == 20
            && u64::from_le_bytes(request[..8].try_into().unwrap()) == fault[2]
            && u64::from_le_bytes(request[8..16].try_into().unwrap()) == fault[3]
        {
            root = fault[1];
            fault[0] = 0;
        }
        drop(fault);
        if root != 0 {
            revoke(root);
        }
        Ok(result)
    }
    pub fn local_memory_compare_exchange(
        window: u64,
        offset: u64,
        expected: &[u8],
        replacement: &[u8],
    ) -> Result<bool, i32> {
        let mut fault = FAULT.lock().unwrap();
        let mut root = 0;
        if fault[0] == 2 && fault[2] == window && fault[3] == offset && expected.len() == 16384 {
            let mut other = expected.to_vec();
            let version = u64::from_le_bytes(other[16..24].try_into().unwrap());
            other[16..24].copy_from_slice(&(version + 1).to_le_bytes());
            assert!(crate::client::local_memory_compare_exchange(
                window, offset, expected, &other
            )?);
            fault[0] = 0;
        } else if fault[0] == 4
            && fault[2] == window
            && fault[3] == offset
            && expected.len() == 16384
        {
            root = fault[1];
            fault[0] = 0;
        }
        drop(fault);
        let result =
            crate::client::local_memory_compare_exchange(window, offset, expected, replacement)?;
        if root != 0 && result {
            revoke(root);
        }
        Ok(result)
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
        0,
        65536,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let ram = memory::Memory::new().unwrap();
    let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
    let mut inventory = [0u8; 64];
    assert_eq!(gamepkg::local_call(1, &[], &mut inventory), Ok(64));
    let window = u64::from_le_bytes(inventory[48..56].try_into().unwrap());
    let commit = [
        window.to_le_bytes().as_slice(),
        &0u64.to_le_bytes(),
        &1048576u64.to_le_bytes(),
        &2u32.to_le_bytes(),
    ]
    .concat();
    gamepkg::local_call(2, &commit, &mut []).unwrap();
    let mut staged = handles::Handles::handles_new(&ram).unwrap();
    gamepkg::fault_arm(1, staged.handles_id(), 0, 0);
    assert_eq!(staged.handles_bind(&ram, window, 0), Err(-3));
    assert!(gamepkg::fault_done());
    assert_eq!(staged.handles_progress(), [1, 3]);
    let (_, b) = gamepkg::local_record_snapshot(staged.handles_id(), 80).unwrap();
    assert_eq!(u64::from_le_bytes(b[64..72].try_into().unwrap()), 2);
    staged.handles_close(&ram).unwrap();
    assert!(staged.handles_closed());
    let mut active = handles::Handles::handles_new(&ram).unwrap();
    active.handles_bind(&ram, window, 0).unwrap();
    let mut guard = active.handles_lock(&ram).unwrap();
    gamepkg::fault_arm(2, 0, window, 0);
    assert_eq!(guard.handle_guard_insert(&ram, 17, 1, false), Err(-3));
    assert!(gamepkg::fault_done());
    assert_eq!(guard.handle_guard_lookup(&ram, 0x48000001), Ok(None));
    assert_eq!(
        guard.handle_guard_insert(&ram, 17, 1, false),
        Ok(0x48000001)
    );
    assert_eq!(
        guard.handle_guard_remove(&ram, 0x48000001),
        Ok(Some([17, 1]))
    );
    guard.handles_guard_close(&ram).unwrap();
    gamepkg::fault_arm(2, 0, window, 15 * 16384);
    assert_eq!(active.handles_close(&ram), Err(-3));
    assert!(gamepkg::fault_done());
    assert_eq!(active.handles_progress(), [3, 16]);
    assert!(!active.handles_closed());
    active.handles_close(&ram).unwrap();
    assert!(active.handles_closed());
    let mut data = vec![0u8; 262144];
    let read = [
        window.to_le_bytes().as_slice(),
        &0u64.to_le_bytes(),
        &262144u32.to_le_bytes(),
    ]
    .concat();
    assert_eq!(gamepkg::local_call(5, &read, &mut data), Ok(data.len()));
    assert!(data.iter().all(|v| *v == 0));
    services.service_close(&ram).unwrap();
    gamepkg::local_record_release(ram.call_handle()).unwrap();
    for mode in [3u64, 4] {
        let root = memory::Memory::new().unwrap();
        let service = kernel_pool::kernel_services_initialize(&root, 0x10000, 2).unwrap();
        let mut table = handles::Handles::handles_new(&root).unwrap();
        if mode == 4 {
            gamepkg::fault_arm(4, root.call_handle(), window, 2 * 16384);
            assert_eq!(table.handles_bind(&root, window, 0), Err(-4));
            assert!(gamepkg::fault_done());
            assert_eq!(table.handles_progress(), [1, 3]);
        } else {
            table.handles_bind(&root, window, 0).unwrap();
            let mut guard = table.handles_lock(&root).unwrap();
            gamepkg::fault_arm(3, root.call_handle(), window, 15 * 16384);
            assert_eq!(guard.handle_guard_insert(&root, 17, 1, false), Err(-4));
            assert!(gamepkg::fault_done());
            guard.handles_guard_close(&root).unwrap();
            let mut cleanup = table.handles_cleanup_lock(&root).unwrap();
            assert_eq!(
                cleanup.handle_guard_slots(&root).unwrap(),
                vec![[0, 0]; 16383]
            );
            cleanup.handles_guard_close(&root).unwrap();
        }
        assert_eq!(table.handles_close(&root), Err(-4));
        assert!(table.handles_closed());
        assert_eq!(table.handles_close(&root), Err(-4));
        service.service_close(&root).unwrap();
        gamepkg::local_record_release(root.call_handle()).unwrap();
    }
    local::shutdown();
    println!("PASS actual SDK stale progress after accepted RAM prefix, stale insert without false publication, stale clear without premature decrement, revocation during scan/binding; maintainer-only shim, not title gameplay");
}
