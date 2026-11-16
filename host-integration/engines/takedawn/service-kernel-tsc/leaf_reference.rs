#[allow(unused_imports)]
mod gamepkg {
    pub use crate::client::*;
}
static TSC: std::sync::Mutex<u64> = std::sync::Mutex::new(0);
mod kernel_pool {
    pub use crate::real_kernel_pool::*;
    pub fn kernel_tsc_read(_: &crate::memory::Memory) -> Result<u64, i32> {
        Ok(*crate::TSC.lock().unwrap())
    }
}
mod original {
    type uint32_t = u32;
    type uint64_t = u64;
    type int32_t = i32;
    type uintptr_t = usize;
    static mut EAX: u32 = 0;
    static mut EBX: u32 = 0;
    static mut ECX: u32 = 0;
    static mut EDX: u32 = 0;
    static mut ESP: u32 = 0;
    static mut g_xbox_mem_offset: usize = 0;
    unsafe fn gamepkg_tls_g_eax() -> *mut u32 {
        &raw mut EAX
    }
    unsafe fn gamepkg_tls_g_ebx() -> *mut u32 {
        &raw mut EBX
    }
    unsafe fn gamepkg_tls_g_ecx() -> *mut u32 {
        &raw mut ECX
    }
    unsafe fn gamepkg_tls_g_edx() -> *mut u32 {
        &raw mut EDX
    }
    unsafe fn gamepkg_tls_g_esp() -> *mut u32 {
        &raw mut ESP
    }
    unsafe fn xbox_ReadTimeStampCounter() -> u64 {
        *crate::TSC.lock().unwrap()
    }
    pub unsafe fn run(kind: u32, bytes: &mut [u8], cpu: &[u8]) -> Vec<u8> {
        g_xbox_mem_offset = bytes.as_mut_ptr() as usize;
        EAX = u32::from_le_bytes(cpu[..4].try_into().unwrap());
        EBX = u32::from_le_bytes(cpu[4..8].try_into().unwrap());
        ECX = u32::from_le_bytes(cpu[8..12].try_into().unwrap());
        EDX = u32::from_le_bytes(cpu[12..16].try_into().unwrap());
        ESP = u32::from_le_bytes(cpu[28..32].try_into().unwrap());
        match kind {
            0 => sub_001D20AC(),
            1 => sub_001FC960(),
            2 => sub_00356AC0(),
            3 => sub_001FC930(),
            4 => sub_001FC980(),
            _ => unreachable!(),
        }
        let mut out = cpu.to_vec();
        out[..4].copy_from_slice(&EAX.to_le_bytes());
        out[4..8].copy_from_slice(&EBX.to_le_bytes());
        out[8..12].copy_from_slice(&ECX.to_le_bytes());
        out[12..16].copy_from_slice(&EDX.to_le_bytes());
        out[28..32].copy_from_slice(&ESP.to_le_bytes());
        out
    }
    /* PINNED_ORACLE */
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 262144,
        }],
        vec![],
        4 * 1024 * 1024,
        4,
        65536,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let mut count = 0;
    for (turn, value) in [
        0u64,
        1,
        0xffffffff,
        0x100000000,
        0x100000001,
        0x7fffffffffffffff,
        0x8000000000000000,
        u64::MAX,
        0x123456789abcdef,
        733333333,
        0xabcdef0198765432,
        0xffff0000ffff0000,
    ]
    .into_iter()
    .enumerate()
    {
        for kind in 0..5u32 {
            for alias in 0..4u32 {
                let (mut ram, services) = leaf_probe::leaf_memory(count == 0).unwrap();
                let pointer = [0x11000u32, 0x11001, 0x11ffc, 0x18004][alias as usize];
                ram.write_u32(0x18000, 0x12345678).unwrap();
                ram.write_u32(0x18004, pointer).unwrap();
                let mut state = leaf_probe::leaf_state(turn as u32);
                let before = state.encode();
                let mapped = leaf_probe::leaf_snapshot(&ram).unwrap();
                let mut expected = vec![0xa5u8; 0x420000];
                expected[..131072].copy_from_slice(&mapped[..131072]);
                expected[0x400000..].copy_from_slice(&mapped[131072..]);
                *TSC.lock().unwrap() = value;
                let cpu = unsafe { original::run(kind, &mut expected, &before) };
                leaf_probe::leaf_invoke(kind, &mut state, &mut ram).unwrap();
                assert_eq!(
                    state.encode(),
                    cpu,
                    "CPU kind {kind} turn {turn} alias {alias}"
                );
                let mut actual = leaf_probe::leaf_snapshot(&ram).unwrap();
                let mut witness = expected[..131072].to_vec();
                witness.extend(&expected[0x400000..]);
                assert_eq!(actual, witness, "RAM kind {kind} turn {turn} alias {alias}");
                // The oracle's unmapped gap is not read or mirrored into game memory.
                assert!(expected[131072..0x400000].iter().all(|b| *b == 0xa5));
                actual.clear();
                services.service_close(&ram).unwrap();
                client::local_record_release(ram.call_handle()).unwrap();
                count += 1;
            }
        }
    }
    println!("PASS: {count} exact original generated-Rust TSC leaf comparisons, full CPU and all 262144 mapped RAM bytes; controlled service results only, not actual time or complete callers");
}
