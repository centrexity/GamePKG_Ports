// Maintainer-only full-memory and CPU differential oracle.
unsafe extern "C" {
    fn crypto_reference_reset(bytes: *mut u8);
    fn crypto_reference_call(ordinal: u32, eax: *mut u32, esp: *mut u32);
}
fn main() {
    let owner = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 262144,
        }],
        vec![],
        4 * 1024 * 1024,
        0,
        1048576,
    )
    .unwrap();
    local::install(owner).unwrap();
    let mut vectors = Vec::new();
    let mut calls = 0;
    for mode in 0..96u32 {
        let mut ram = probe::crypto_memory(mode).unwrap();
        let mut expected = vec![0; 262144];
        ram.read_bytes(0, &mut expected).unwrap();
        unsafe { crypto_reference_reset(expected.as_mut_ptr()) };
        let mut witness = Vec::new();
        for (turn, step) in cases::crypto_steps(mode).unwrap().iter().enumerate() {
            let mut state = probe::crypto_state(mode, turn as u32);
            probe::crypto_arguments(&mut ram, *step).unwrap();
            ram.read_bytes(0x18000, &mut expected[0x18000..0x18020])
                .unwrap();
            let before = state.encode();
            let mut eax = state.eax;
            let mut esp = state.esp;
            unsafe { crypto_reference_call(step[0], &mut eax, &mut esp) };
            kernel_pool::kernel_invoke(&mut state, &mut ram, step[0] - 338).unwrap();
            let mut cpu = before;
            cpu[0..4].copy_from_slice(&eax.to_le_bytes());
            cpu[28..32].copy_from_slice(&esp.to_le_bytes());
            assert_eq!(state.encode(), cpu, "CPU mode={mode} turn={turn}");
            let mut actual = vec![0; 262144];
            ram.read_bytes(0, &mut actual).unwrap();
            if actual != expected {
                let at = actual
                    .iter()
                    .zip(&expected)
                    .position(|(a, b)| a != b)
                    .unwrap();
                panic!(
                    "RAM mode={mode} turn={turn} address={at:x}: actual={:02x} expected={:02x}",
                    actual[at], expected[at]
                );
            }
            let mut result = expected[0x10fc0..0x11200].to_vec();
            result.extend(&expected[0x12000..0x12040]);
            result.extend(&expected[0x20000..0x20020]);
            result.extend(&expected[0x21000..0x21020]);
            result.extend(&expected[0x22000..0x24000]);
            result.extend(&expected[0x25000..0x25080]);
            result.extend(&cpu);
            assert_eq!(probe::crypto_witness(&ram, &state).unwrap(), result);
            witness.extend(result);
            calls += 1;
        }
        vectors.extend((witness.len() as u32).to_le_bytes());
        vectors.extend(&witness);
        ram.kernel_clear().unwrap();
        client::local_record_release(ram.call_handle()).unwrap();
    }
    controls::crypto_denials().unwrap();
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, &vectors).unwrap();
    }
    println!("PASS: 96 RC4/HMAC setups/{calls} exact original bridge calls, all 262144 RAM bytes after every call, complete CPU and retained RC4 context and aliased output; no title entry");
}
