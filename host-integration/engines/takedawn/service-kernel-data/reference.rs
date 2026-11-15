// Maintainer-only exact C comparison; no reference C enters admitted objects.
unsafe extern "C" {
    fn gamepkg_data_run(
        ram: *mut u8,
        values: *const u32,
        cmd: *const u8,
        hd: *const u8,
        cert: *const u8,
    );
    fn gamepkg_data_keys(out: *mut u8);
    fn gamepkg_data_address(ordinal: u32) -> u32;
    fn gamepkg_data_signature(cert: *const u8, out: *mut u8);
    fn gamepkg_heap_stats(out: *mut u32);
}
fn owner(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn main() {
    let output = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    if let Some(path) = &output {
        std::fs::create_dir_all(path).unwrap();
    }
    let zero = [0u8; 16];
    let known = [
        111, 46, 134, 55, 53, 108, 226, 29, 174, 122, 215, 209, 237, 152, 32, 2,
    ];
    let mut original_zero = [0u8; 16];
    unsafe {
        gamepkg_data_signature(zero.as_ptr(), original_zero.as_mut_ptr());
    }
    assert_eq!(original_zero, known);
    assert_eq!(signature::console_signature(&zero), known);
    for mode in 0..1024u32 {
        let mut key = [0u8; 16];
        for (i, byte) in key.iter_mut().enumerate() {
            *byte = (mode.wrapping_mul(197).wrapping_add(i as u32 * 71) >> (i % 3)) as u8;
        }
        let mut expected = [0u8; 16];
        unsafe {
            gamepkg_data_signature(key.as_ptr(), expected.as_mut_ptr());
        }
        assert_eq!(
            signature::console_signature(&key),
            expected,
            "signature {mode}"
        );
    }
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
        ],
        vec![owner],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    let windows = runtime.windows().unwrap();
    local::install(runtime).unwrap();
    for mode in 0..24u32 {
        let case = cases::Case::data_case(mode).unwrap();
        let mut ram = vec![0xa5u8; 67108864];
        let mut command = case.command.clone();
        command.push(0);
        let hd = case.hd.unwrap_or([0; 16]);
        let values = [
            mode,
            case.region,
            case.tick,
            case.video,
            case.top,
            case.magic,
            case.certificate,
            u32::from(case.hd.is_some()),
        ];
        unsafe {
            gamepkg_data_run(
                ram.as_mut_ptr(),
                values.as_ptr(),
                command.as_ptr(),
                hd.as_ptr(),
                case.cert_key.as_ptr(),
            );
        }
        let base = case.region + 0x40000;
        let launch = u32::from_le_bytes(
            ram[base as usize + 0x30..base as usize + 0x34]
                .try_into()
                .unwrap(),
        );
        let mut counters = [0u32; 3];
        let mut keys = [0u8; 32];
        unsafe {
            gamepkg_heap_stats(counters.as_mut_ptr());
            gamepkg_data_keys(keys.as_mut_ptr());
        }
        let mut expected = Vec::new();
        for value in [mode, base, launch, counters[0], counters[1], counters[2]] {
            expected.extend(value.to_le_bytes());
        }
        expected.extend(keys);
        expected.extend(&ram[base as usize..base as usize + 4096]);
        if launch != 0 {
            expected.extend(&ram[launch as usize..launch as usize + 7168]);
        }
        for ordinal in 0..400u32 {
            expected.extend(unsafe { gamepkg_data_address(ordinal) }.to_le_bytes());
        }
        let actual = probe::data_sample(mode)
            .unwrap_or_else(|code| panic!("data case {mode}: error {code}"));
        assert_eq!(
            actual.len(),
            expected.len(),
            "data case {mode}: witness length"
        );
        if actual != expected {
            let at = actual
                .iter()
                .zip(&expected)
                .position(|(a, b)| a != b)
                .unwrap();
            panic!(
                "data case {mode}: witness offset {at}: {} != {}",
                actual[at], expected[at]
            );
        }
        if let Some(path) = &output {
            std::fs::write(path.join(format!("{mode}.bin")), &expected).unwrap();
        }
        // The sample subsequently checks the explicit tick-write operation.
        ram[base as usize + 0x20..base as usize + 0x24]
            .copy_from_slice(&case.tick.wrapping_add(123).to_le_bytes());
        let mut copied = vec![0u8; 1048576];
        for chunk in 0..64usize {
            let mut req = windows[0].0.to_le_bytes().to_vec();
            req.extend((chunk as u64 * 1048576).to_le_bytes());
            req.extend(1048576u32.to_le_bytes());
            assert_eq!(client::local_call(5, &req, &mut copied), Ok(1048576));
            assert_eq!(
                copied,
                ram[chunk * 1048576..(chunk + 1) * 1048576],
                "whole RAM case {mode} chunk {chunk}"
            );
        }
    }
    assert_eq!(probe::data_denials(), Ok(()));
    local::shutdown();
    println!("PASS: 24 exact original kernel-data initializations, all 400 ordinal probes, full 64 MiB RAM and heap counters; 1024 original certificate signatures; synthetic only");
}
