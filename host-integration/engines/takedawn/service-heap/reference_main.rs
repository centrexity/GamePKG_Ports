// Maintainer-only direct comparison. Paths/modules are added by check.py.
fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn main() {
    let owner = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 67108864,
        }],
        vec![],
        128 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let window = owner.windows().unwrap()[0].0;
    owner.commit(window, 0, 67108864, 2).unwrap();
    local::install(owner.clone()).unwrap();
    let mut ram = memory::Memory::new().unwrap();
    ram.attach(0, 67108864, window, 0, true).unwrap();
    for index in 0..reference_cases::CASES {
        let fixture = reference_cases::make_case(index);
        ram.fill(0, 0xa5, 67108864).unwrap();
        denials::heap_denials(&mut ram, window).unwrap();
        let mut state =
            heap::Heap::heap_new(&ram, word(&fixture.bytes, 4), word(&fixture.bytes, 8)).unwrap();
        for vector in fixture.bytes[16..].chunks_exact(32) {
            let op = word(vector, 0);
            let first = word(vector, 4);
            let second = word(vector, 8);
            let result = match op {
                0 => state.heap_allocate(&mut ram, first, second),
                1 => state.heap_free(&ram, first).map(|()| 0),
                2 => state.heap_size(&ram, first),
                3 => ram.fill(first, 0x7b, second).map(|()| 0),
                _ => panic!("unexpected operation"),
            };
            let expected_error = word(vector, 16) as i32;
            assert_eq!(
                result,
                if expected_error == 0 {
                    Ok(word(vector, 12))
                } else {
                    Err(expected_error)
                },
                "case {index}, op {op}"
            );
            assert_eq!(
                state.heap_stats(),
                [word(vector, 20), word(vector, 24), word(vector, 28)]
            );
        }
        assert_eq!(state.heap_stats(), fixture.stats);
        for (number, record) in fixture.records.iter().enumerate() {
            assert_eq!(state.heap_record(number as u32).unwrap(), *record);
        }
        assert_eq!(state.heap_record(fixture.stats[1]), Err(-1));
        let mut bytes = vec![0u8; 1048576];
        for (number, expected) in fixture.ram.chunks_exact(1048576).enumerate() {
            ram.read_bytes(number as u32 * 1048576, &mut bytes).unwrap();
            assert_eq!(bytes, expected, "RAM chunk {number}, case {index}");
        }
        println!("PASS case {index}: {} operations, all 64 MiB RAM bytes, every block record and per-operation state match pinned original",fixture.operations);
    }
    local::shutdown();
}
