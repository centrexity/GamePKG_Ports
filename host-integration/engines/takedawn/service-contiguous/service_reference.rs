// Maintainer-only comparison; native references never enter admitted game code.
fn contig_word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn run_shared(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}

fn shared_spawn(inbox: u64) -> Result<u64, i32> {
    let mut req = std::vec::Vec::new();
    req.extend(u32::to_le_bytes(0));
    req.extend(u64::to_le_bytes(inbox));
    req.extend(u32::to_le_bytes(0));
    let mut out = [0u8; 8];
    if gamepkg::local_call(10, &req, &mut out)? != 8 {
        return Err(-3);
    }
    Ok(u64::from_le_bytes(out))
}
fn shared_join(job: u64) -> Result<(), i32> {
    let pause = gamepkg::local_mailbox_create(1, 1)?;
    let mut out = [0u8; 8];
    let mut byte = [0u8; 1];
    loop {
        match gamepkg::local_call(11, &u64::to_le_bytes(job), &mut out) {
            Ok(8) => {
                gamepkg::local_mailbox_release(pause)?;
                if u64::from_le_bytes(out) != 0 {
                    return Err(-3);
                }
                return Ok(());
            }
            Err(-2) => {
                if gamepkg::local_mailbox_receive(pause, 1, &mut byte) != Err(-2) {
                    return Err(-3);
                }
            }
            Ok(_) => return Err(-3),
            Err(code) => return Err(code),
        }
    }
}
fn shared_stats(state: &mut heap_service::HeapClient) -> Result<[u32; 6], i32> {
    let mut stats = [0u32; 6];
    for number in 0..6u32 {
        stats[number as usize] = state.heap_contiguous_counter(number)?;
    }
    Ok(stats)
}
fn shared_record(state: &mut heap_service::HeapClient, page: u32) -> Result<[u32; 2], i32> {
    let copied = state.heap_contiguous_pages(page, 1)?;
    Ok([u32::from(copied[0]), state.heap_contiguous_length(page)?])
}

fn run_combined(_: local::runtime::TaskContext, arg: u64) -> u64 {
    shared_checks::combined_client(arg)
}
fn run_fake(_: local::runtime::TaskContext, arg: u64) -> u64 {
    shared_checks::combined_fake(arg)
}
fn run_pending(_: local::runtime::TaskContext, arg: u64) -> u64 {
    shared_checks::combined_pending(arg)
}
fn run_controls() {
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
        vec![run_shared, run_combined, run_fake, run_pending],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    assert_eq!(shared_checks::combined_verify(), Ok(1000));
    local::shutdown();
    println!("PASS: mixed allocator protocol, one shared owner, 1000 concurrent heap/contiguous lifetimes, fixed claims, fallback, denial and poisoned-peer controls");
}
fn main() {
    run_controls();
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
        vec![run_shared],
        256 * 1024 * 1024,
        2,
        1048576,
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
        let inbox = heap_service::heap_inbox().unwrap();
        let job = shared_spawn(inbox).unwrap();
        let mut state = heap_service::HeapClient::heap_client(inbox, job).unwrap();
        state
            .heap_initialize(
                ram.call_handle(),
                owner.windows().unwrap()[0].0,
                16777216,
                67108864,
            )
            .unwrap();
        state.heap_contiguous_initialize(window).unwrap();
        for (number, vector) in fixture.bytes[8..].chunks_exact(48).enumerate() {
            let [op, a, b, c, d] = std::array::from_fn(|n| contig_word(vector, n * 4));
            let result = match op {
                0 => state.heap_contiguous_allocate(a, b, c, d),
                1 => state.heap_contiguous_claim(a, b).map(|()| 0),
                2 => state.heap_contiguous_free(a).map(u32::from),
                3 => state.heap_contiguous_query(a),
                4 => {
                    gamepkg::local_memory_fill(window, u64::from(a), u64::from(b), 0x7b).map(|()| 0)
                }
                5 => state.heap_contiguous_allocate(a, b, 0, u32::MAX),
                6 => state.heap_contiguous_clear_claim(a, b).map(|()| 0),
                _ => panic!("operation {op}"),
            };
            assert_eq!(
                result,
                Ok(contig_word(vector, 20)),
                "case {index} vector {number}: {op} {a:x} {b:x} {c:x} {d:x}"
            );
            assert_eq!(
                shared_stats(&mut state).unwrap(),
                std::array::from_fn(|n| contig_word(vector, 24 + n * 4)),
                "case {index} vector {number}"
            );
        }
        assert_eq!(shared_stats(&mut state).unwrap(), fixture.stats);
        for (number, expected) in fixture.records.iter().enumerate() {
            assert_eq!(
                shared_record(&mut state, number as u32).unwrap(),
                *expected,
                "page {number}, case {index}"
            );
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
        state.heap_stop().unwrap();
        state.heap_abandon().unwrap();
        shared_join(job).unwrap();
        println!("PASS case {index}: {} original shared-owner contiguous operations, all 64 MiB RAM bytes, all 16384 page records and per-operation state",fixture.operations);
    }
    local::shutdown();
}
