// Maintainer-only actual SDK interference, rejected even unused as game source.
mod gamepkg {
    pub use crate::client::*;
    static FAULT: std::sync::Mutex<[u64; 4]> = std::sync::Mutex::new([0; 4]);
    pub fn fault_arm(mode: u64, root: u64, window: u64, record: u64) {
        *FAULT.lock().unwrap() = [mode, root, window, record];
    }
    pub fn fault_done() -> bool {
        FAULT.lock().unwrap()[0] == 0
    }
    pub fn local_call(op: u32, request: &[u8], output: &mut [u8]) -> Result<usize, i32> {
        let result = crate::client::local_call(op, request, output)?;
        let mut fault = FAULT.lock().unwrap();
        let mut revoke = 0;
        if fault[0] == 1
            && op == 6
            && request.len() == 20
            && u64::from_le_bytes(request[..8].try_into().unwrap()) == fault[2]
            && u64::from_le_bytes(request[8..16].try_into().unwrap()) == 0x780108
            && request[16..] == [0; 4]
        {
            revoke = fault[1];
            fault[0] = 0;
        }
        drop(fault);
        if revoke != 0 {
            let ram = crate::memory::Memory::with_call_handle(revoke).unwrap();
            ram.services_link(ram.services_handle().unwrap(), 0)
                .unwrap();
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
        if fault[0] == 2 && window == fault[2] && offset == 0 && expected.len() == 16384 {
            let mut changed = expected.to_vec();
            let version = u64::from_le_bytes(changed[16..24].try_into().unwrap());
            changed[16..24].copy_from_slice(&(version + 1).to_le_bytes());
            assert!(crate::client::local_memory_compare_exchange(
                window, offset, expected, &changed
            )?);
            fault[0] = 0;
        }
        drop(fault);
        crate::client::local_memory_compare_exchange(window, offset, expected, replacement)
    }
    pub fn local_record_compare_exchange(
        record: u64,
        version: u64,
        bytes: &[u8],
    ) -> Result<u64, i32> {
        let mut fault = FAULT.lock().unwrap();
        if fault[0] == 3
            && fault[3] == record
            && bytes.len() == 2624
            && u64::from_le_bytes(bytes[72..80].try_into().unwrap()) == 2
        {
            let (current, old) = crate::client::local_record_snapshot(record, 2624)?;
            crate::client::local_record_compare_exchange(record, current, &old)?;
            fault[0] = 0;
        }
        drop(fault);
        crate::client::local_record_compare_exchange(record, version, bytes)
    }
}
fn frame(ram: &mut memory::Memory, args: &[u32]) -> registers::Registers {
    let mut cpu = registers::Registers::new();
    cpu.esp = 0x782000;
    cpu.eax = 0xdeadbeef;
    ram.write_u32(cpu.esp, 0x12345678).unwrap();
    for (index, value) in args.iter().enumerate() {
        ram.write_u32(cpu.esp + 4 + index as u32 * 4, *value)
            .unwrap();
    }
    cpu
}
fn main() {
    let mode = std::env::args().nth(1).unwrap().parse::<u64>().unwrap();
    assert!((1..=3).contains(&mode));
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
    let mut fixture = setup::Fixture::fixture_new().unwrap();
    let backing = layout::Layout::layout_services(&fixture.ram).unwrap();
    let root = fixture.ram.call_handle();
    let services = fixture.ram.services_handle().unwrap();
    if mode == 1 {
        let mut cpu = frame(&mut fixture.ram, &[0x780100, 0xffffffff, 0, 0]);
        kernel_pool::kernel_invoke(&mut cpu, &mut fixture.ram, 0).unwrap();
        let token = fixture.ram.read_u32(0x780100).unwrap();
        let mut guard = fixture.table.handles_lock(&fixture.ram).unwrap();
        let id = guard
            .handle_guard_lookup(&fixture.ram, token)
            .unwrap()
            .unwrap()[0];
        guard.handles_guard_close(&fixture.ram).unwrap();
        fixture.ram.write_u32(0x780108, 0x55555555).unwrap();
        let mut cpu = frame(&mut fixture.ram, &[token, 0x780108]);
        let before = cpu.encode();
        gamepkg::fault_arm(
            mode,
            root,
            backing.layout_banks()[0],
            fixture.domain.events_id(),
        );
        assert_eq!(
            kernel_pool::kernel_invoke(&mut cpu, &mut fixture.ram, 1),
            Err(-4)
        );
        assert!(gamepkg::fault_done());
        assert_eq!(cpu.encode(), before);
        assert_eq!(fixture.ram.read_u32(0x780108).unwrap(), 0);
        assert!(!gamepkg::local_event_state(id).unwrap().signaled);
        assert_eq!(fixture.ram.services_handle().unwrap(), 0);
        fixture.ram.services_link(0, services).unwrap();
    } else {
        fixture.ram.write_u32(0x780100, 0x33333333).unwrap();
        let mut cpu = frame(&mut fixture.ram, &[0x780100, 0xffffffff, 0, 0]);
        let before = cpu.encode();
        gamepkg::fault_arm(
            mode,
            root,
            backing.layout_scratch().unwrap(),
            fixture.domain.events_id(),
        );
        assert_eq!(
            kernel_pool::kernel_invoke(&mut cpu, &mut fixture.ram, 0),
            Err(-3)
        );
        assert!(gamepkg::fault_done());
        assert_eq!(cpu.encode(), before);
        assert_eq!(fixture.ram.read_u32(0x780100).unwrap(), 0x33333333);
        let mut guard = fixture.table.handles_lock(&fixture.ram).unwrap();
        assert_eq!(
            guard.handle_guard_lookup(&fixture.ram, 0x48000001).unwrap(),
            None
        );
        guard.handles_guard_close(&fixture.ram).unwrap();
    }
    fixture.fixture_close().unwrap();
    local::shutdown();
    println!("PASS actual SDK import fault {mode}: accepted prefixes and fatal-error distinction retained; maintainer-only, not gameplay");
}
