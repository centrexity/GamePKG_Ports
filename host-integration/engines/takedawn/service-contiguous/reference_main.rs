// Maintainer-only comparison; native references never enter admitted game code.
fn contig_word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn main() {
    let owner = local::runtime::Runtime::new(
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
        vec![],
        256 * 1024 * 1024,
        0,
        65536,
    )
    .unwrap();
    let window = owner.windows().unwrap()[1].0;
    owner.commit(window, 0, 67108864, 2).unwrap();
    local::install(owner.clone()).unwrap();
    let ram = memory::Memory::new().unwrap();
    denials::contiguous_denials(&ram, window).unwrap();
    for index in 0..reference_cases::CASES {
        let fixture = reference_cases::make_case(index);
        gamepkg::local_memory_fill(window, 0, 67108864, 0xa5).unwrap();
        let mut state = contiguous::Contiguous::contiguous_new(&ram, window).unwrap();
        for (number, vector) in fixture.bytes[8..].chunks_exact(48).enumerate() {
            let [op, a, b, c, d] = std::array::from_fn(|n| contig_word(vector, n * 4));
            let result = match op {
                0 => state.contiguous_allocate(&ram, a, b, c, d),
                1 => state.contiguous_claim(&ram, a, b).map(|()| 0),
                2 => state.contiguous_free(&ram, a).map(u32::from),
                3 => state.contiguous_size(&ram, a),
                4 => {
                    gamepkg::local_memory_fill(window, u64::from(a), u64::from(b), 0x7b).map(|()| 0)
                }
                5 => state.contiguous_allocate(&ram, a, b, 0, u32::MAX),
                6 => state.contiguous_clear_claim(&ram, a, b).map(|()| 0),
                _ => panic!("operation {op}"),
            };
            assert_eq!(
                result,
                Ok(contig_word(vector, 20)),
                "case {index} vector {number}: {op} {a:x} {b:x} {c:x} {d:x}"
            );
            assert_eq!(
                state.contiguous_stats(),
                std::array::from_fn(|n| contig_word(vector, 24 + n * 4)),
                "case {index} vector {number}"
            );
        }
        assert_eq!(state.contiguous_stats(), fixture.stats);
        for (number, expected) in fixture.records.iter().enumerate() {
            assert_eq!(
                state.contiguous_record(number as u32).unwrap(),
                *expected,
                "page {number}, case {index}"
            );
        }
        let mut table = vec![0u8; 4096];
        for page in (0..16384u32).step_by(4096) {
            state.contiguous_pages(page, &mut table).unwrap();
            for (at, byte) in table.iter().enumerate() {
                assert_eq!(u32::from(*byte), fixture.records[page as usize + at][0]);
            }
        }
        let mut actual = vec![0u8; 1048576];
        for (number, expected) in fixture.ram.chunks_exact(1048576).enumerate() {
            gamepkg::local_memory_read_segments(
                &[[window, number as u64 * 1048576, 1048576]],
                &mut actual,
            )
            .unwrap();
            assert_eq!(actual, expected, "RAM chunk {number}, case {index}");
        }
        println!("PASS case {index}: {} original contiguous operations, all 64 MiB RAM bytes, all 16384 page records and per-operation state",fixture.operations);
    }
    local::shutdown();
}
