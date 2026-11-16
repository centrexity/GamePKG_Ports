// Maintainer-only C comparison and diagnostic capture, not game source.
mod gamepkg {
    pub use crate::client::*;
    pub static LOGS: std::sync::Mutex<Vec<(u32, u32, Vec<u64>)>> =
        std::sync::Mutex::new(Vec::new());
    pub fn diagnostic(level: u32, code: u32, values: &[u64]) -> Result<u64, i32> {
        let mut logs = LOGS.lock().unwrap();
        logs.push((level, code, values.to_vec()));
        Ok(logs.len() as u64)
    }
}
unsafe extern "C" {
    fn settings_reference_reset(bytes: *mut u8, video: u32, speakers: u32);
    fn settings_reference_call(eax: *mut u32, esp: *mut u32);
    fn settings_reference_logs(out: *mut u32);
}
fn worker(_: local::runtime::TaskContext, root: u64) -> u64 {
    controls::settings_worker(root)
}
fn trace_worker(_: local::runtime::TaskContext, root: u64) -> u64 {
    controls::settings_trace_worker(root)
}
fn main() {
    let owner = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 262144,
        }],
        vec![worker, trace_worker],
        4 * 1024 * 1024,
        4,
        65536,
    )
    .unwrap();
    local::install(owner).unwrap();
    let mut vectors = Vec::new();
    for mode in 0..96u32 {
        let (mut ram, services) = probe::settings_memory(mode).unwrap();
        let config = cases::settings_config(mode);
        let step = cases::settings_step(mode).unwrap();
        probe::settings_arguments(&mut ram, step).unwrap();
        let mut expected = vec![0u8; 262144];
        ram.read_bytes(0, &mut expected).unwrap();
        unsafe { settings_reference_reset(expected.as_mut_ptr(), config[0], config[1]) };
        let mut state = probe::settings_state(mode, 0);
        let mut eax = state.eax;
        let mut esp = state.esp;
        let mut cpu = state.encode();
        gamepkg::LOGS.lock().unwrap().clear();
        unsafe { settings_reference_call(&mut eax, &mut esp) };
        kernel_pool::kernel_invoke(&mut state, &mut ram, 0).unwrap();
        cpu[..4].copy_from_slice(&eax.to_le_bytes());
        cpu[28..32].copy_from_slice(&esp.to_le_bytes());
        assert_eq!(state.encode(), cpu, "CPU mode {mode}");
        let mut actual = vec![0u8; 262144];
        ram.read_bytes(0, &mut actual).unwrap();
        if actual != expected {
            let at = actual
                .iter()
                .zip(&expected)
                .position(|(a, b)| a != b)
                .unwrap();
            panic!(
                "RAM mode {mode} offset {at:x} actual {:02x} expected {:02x}",
                actual[at], expected[at]
            );
        }
        let mut original_logs = [0u32; 6];
        unsafe { settings_reference_logs(original_logs.as_mut_ptr()) };
        let mut expected_logs = Vec::new();
        if original_logs[0] != 0 {
            expected_logs.push((
                original_logs[1],
                0x544b1300,
                vec![u64::from(original_logs[2]), u64::from(original_logs[3])],
            ));
        }
        if original_logs[0] == 2 {
            expected_logs.push((
                original_logs[4],
                0x544b1301,
                vec![u64::from(original_logs[5])],
            ));
        }
        assert_eq!(
            *gamepkg::LOGS.lock().unwrap(),
            expected_logs,
            "logs mode {mode}"
        );
        let mut result = expected[0x11000..0x11010].to_vec();
        result.extend(&expected[0x12000..0x13100]);
        result.extend(&expected[0x14000..0x14010]);
        result.extend(&expected[0x18000..0x18020]);
        result.extend(&cpu);
        assert_eq!(probe::settings_witness(&ram, &state).unwrap(), result);
        vectors.extend((result.len() as u32).to_le_bytes());
        vectors.extend(result);
        services.service_close(&ram).unwrap();
        ram.kernel_clear().unwrap();
        client::local_record_release(ram.call_handle()).unwrap();
    }
    if let Err(code) = controls::settings_denials() {
        let logs = gamepkg::LOGS.lock().unwrap();
        panic!(
            "controls {code}: last logs {:?}",
            &logs[logs.len().saturating_sub(8)..]
        );
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, &vectors).unwrap();
    }
    println!("PASS: 96 original NV calls, every 262144 RAM byte, full CPU, ordered numeric diagnostics and shared-root/concurrency/ownership/error controls; no title entry");
}
