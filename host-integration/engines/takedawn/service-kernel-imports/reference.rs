// Maintainer original-code oracle. Never admitted game source.
unsafe extern "C" {
    fn gamepkg_import_run(ram: *mut u8, table: u32, mode: u32, remap: *const u16, count: u32);
    fn gamepkg_import_count() -> u32;
    fn gamepkg_import_slots() -> u32;
    fn gamepkg_import_slot(slot: u32, out: *mut u32);
    fn gamepkg_import_invoke(out: *mut u32);
    fn gamepkg_heap_stats(out: *mut u32);
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn main() {
    let output = std::env::args_os().nth(1).map(std::path::PathBuf::from);
    if let Some(path) = &output {
        std::fs::create_dir_all(path).unwrap();
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
        vec![worker],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    let main = runtime.windows().unwrap()[0].0;
    local::install(runtime).unwrap();
    let mut chunk = vec![0u8; 1048576];
    for mode in 0..16u32 {
        let case = cases::Case::import_case(mode).unwrap();
        let (mut source, _, job, _, _) = probe::imports_environment(true).unwrap();
        case.import_seed(&mut source).unwrap();
        case.import_states(&source).unwrap();
        let mut ram = vec![0u8; 67108864];
        for index in 0..64usize {
            source
                .read_bytes(
                    index as u32 * 1048576,
                    &mut ram[index * 1048576..(index + 1) * 1048576],
                )
                .unwrap();
        }
        probe::imports_cleanup(&mut source, job).unwrap();
        unsafe {
            gamepkg_import_run(
                ram.as_mut_ptr(),
                case.table,
                mode,
                case.remap.as_ptr(),
                case.remap.len() as u32,
            );
        }
        let count = unsafe { gamepkg_import_count() };
        let slots = unsafe { gamepkg_import_slots() };
        assert_eq!(count, case.count);
        let mut expected = Vec::new();
        for value in [mode, case.table, count, slots] {
            expected.extend(value.to_le_bytes());
        }
        let low = case.table as usize - 64;
        let len = 64 + count as usize * 4 + if mode == 4 { 0 } else { 4 };
        expected.extend(&ram[low..low + len]);
        for slot in 0..378u32 {
            let mut values = [0u32; 3];
            unsafe {
                gamepkg_import_slot(slot, values.as_mut_ptr());
            }
            expected.extend(values[0].to_le_bytes());
            expected.extend(values[1].to_le_bytes());
            expected.push(values[2] as u8);
        }
        let mut route = [0u32; 3];
        unsafe {
            gamepkg_import_invoke(route.as_mut_ptr());
        }
        for word in route {
            expected.extend(word.to_le_bytes());
        }
        let mut counts = [0u32; 3];
        unsafe {
            gamepkg_heap_stats(counts.as_mut_ptr());
        }
        for word in counts {
            expected.extend(word.to_le_bytes());
        }
        let actual = probe::imports_case(mode).unwrap_or_else(|e| panic!("case {mode}: {e}"));
        assert_eq!(actual.len(), expected.len(), "length case {mode}");
        if actual != expected {
            let at = actual
                .iter()
                .zip(&expected)
                .position(|(a, b)| a != b)
                .unwrap();
            panic!(
                "case {mode} offset {at}: {} != {}",
                actual[at], expected[at]
            );
        }
        if let Some(path) = &output {
            std::fs::write(path.join(format!("{mode}.bin")), &expected).unwrap();
        }
        for index in 0..64usize {
            let mut req = main.to_le_bytes().to_vec();
            req.extend((index as u64 * 1048576).to_le_bytes());
            req.extend(1048576u32.to_le_bytes());
            assert_eq!(client::local_call(5, &req, &mut chunk), Ok(1048576));
            if chunk != ram[index * 1048576..(index + 1) * 1048576] {
                let at = chunk
                    .iter()
                    .zip(&ram[index * 1048576..(index + 1) * 1048576])
                    .position(|(a, b)| a != b)
                    .unwrap();
                panic!("RAM case {mode}, byte {}", index * 1048576 + at);
            }
        }
    }
    assert_eq!(controls::imports_controls(), Ok(()));
    local::shutdown();
    println!("PASS: 16 original complete import preparations, all 378 routing slots, full 64 MiB RAM, actual traced/untraced pool calls, copied per-page restoration and strict failure controls; no title entry");
}
