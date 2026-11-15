// Maintainer-only original C vs checked Rust pool bridge comparison.
unsafe extern "C" {
    fn gamepkg_heap_reset(ram: *mut u8, base: u32, top: u32);
    fn gamepkg_heap_stats(out: *mut u32);
    fn gamepkg_pool_init(table: u32, count: u32) -> u32;
    fn gamepkg_pool_call(slot: u32, cpu: *mut u32) -> u32;
}
fn heap_task(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn random(seed: &mut u32) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 17;
    *seed ^= *seed << 5;
    *seed
}
fn put(ram: &mut memory::Memory, expected: &mut [u8], address: u32, value: u32) {
    ram.write_u32(address, value).unwrap();
    expected[address as usize..address as usize + 4].copy_from_slice(&value.to_le_bytes());
}
fn spawn(inbox: u64) -> u64 {
    let mut req = 0u32.to_le_bytes().to_vec();
    req.extend(inbox.to_le_bytes());
    req.extend(0u32.to_le_bytes());
    let mut out = [0; 8];
    assert_eq!(client::local_call(10, &req, &mut out), Ok(8));
    u64::from_le_bytes(out)
}
fn join(job: u64) -> u64 {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        let mut out = [0; 8];
        match client::local_call(11, &job.to_le_bytes(), &mut out) {
            Ok(8) => return u64::from_le_bytes(out),
            Err(-2) => {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            value => panic!("{value:?}"),
        }
    }
}
fn main() {
    let owner = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 67108864,
        }],
        vec![heap_task],
        128 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    let window = owner.windows().unwrap()[0].0;
    owner.commit(window, 0, 67108864, 2).unwrap();
    local::install(owner).unwrap();
    let mut total = 0;
    for scenario in 0..5 {
        let mut ram = memory::Memory::new().unwrap();
        ram.attach(0, 67108864, window, 0, true).unwrap();
        ram.fill(0, 0xa5, 67108864).unwrap();
        let mut expected = vec![0xa5u8; 67108864];
        let base = 0x1000000u32;
        let top = match scenario {
            1 => base + 256,
            2 => base,
            _ => 67108864,
        };
        unsafe { gamepkg_heap_reset(expected.as_mut_ptr(), base, top) };
        let inbox = heap_service::heap_inbox().unwrap();
        let job = spawn(inbox);
        let mut actor = heap_service::HeapClient::heap_client(inbox, job).unwrap();
        actor
            .heap_initialize(ram.call_handle(), window, base, top)
            .unwrap();
        let mut primary = Some(actor);
        ram.heap_install(&mut primary).unwrap();
        assert!(primary.is_none());
        let count = if scenario == 3 { 378 } else { 5 };
        for slot in 0..count {
            put(
                &mut ram,
                &mut expected,
                0x20000 + slot * 4,
                if slot < 4 {
                    0x80000000 | [14, 15, 17, 23][slot as usize]
                } else {
                    0
                },
            );
        }
        assert_eq!(unsafe { gamepkg_pool_init(0x20000, count) }, 8);
        kernel_pool::kernel_prepare(&mut ram, 0x20000, count).unwrap();
        let iterations = if scenario == 0 {
            2048
        } else if scenario == 3 {
            128
        } else {
            256
        };
        let mut seed = 0x12345678u32;
        let mut addresses = Vec::new();
        for turn in 0..iterations {
            let slot = random(&mut seed) & 3;
            let mut state = registers::Registers::new();
            state.eax = random(&mut seed);
            state.ebx = random(&mut seed);
            state.ecx = random(&mut seed);
            state.edx = random(&mut seed);
            state.esi = random(&mut seed);
            state.edi = random(&mut seed);
            state.ebp = random(&mut seed);
            state.esp = if scenario == 4 && turn % 4 == 0 {
                0
            } else {
                0x30000 + (turn & 3)
            };
            state.fs_base = 0x4000;
            state.fp_top = (turn & 7) as i32;
            state.fp_stack[1] = f64::from_bits(0x7ff8000012345678);
            state.xmm3 = [seed, 0x80000000, 0xffffffff, 0x7fc12345];
            let first = if slot < 2 {
                [0, 1, 15, 16, 17, 64, 256, 4096, 8192][random(&mut seed) as usize % 9]
            } else if addresses.is_empty() {
                0
            } else {
                addresses[random(&mut seed) as usize % addresses.len()]
            };
            let sp = state.esp;
            put(&mut ram, &mut expected, sp, 0x12345678);
            put(&mut ram, &mut expected, sp + 4, first);
            put(&mut ram, &mut expected, sp + 8, random(&mut seed));
            let mut cpu = [
                state.eax, state.ebx, state.ecx, state.edx, state.esi, state.edi, state.ebp,
                state.esp,
            ];
            let before = state.encode();
            let caller = unsafe { gamepkg_pool_call(slot, cpu.as_mut_ptr()) };
            kernel_pool::kernel_invoke(&mut state, &mut ram, slot).unwrap();
            let mut wanted = registers::Registers::new();
            wanted.decode(&before).unwrap();
            wanted.eax = cpu[0];
            wanted.esp = cpu[7];
            assert_eq!(
                state.encode(),
                wanted.encode(),
                "scenario {scenario}, turn {turn}, slot {slot}"
            );
            assert_eq!(ram.kernel_caller(), caller);
            if slot < 2 && state.eax != 0 {
                addresses.push(state.eax);
            }
            let mut stats = [0; 3];
            unsafe { gamepkg_heap_stats(stats.as_mut_ptr()) };
            let mut actor = ram.heap_take().unwrap();
            let actual = actor.heap_status().unwrap();
            ram.heap_put(actor);
            assert_eq!(actual, stats, "scenario {scenario}, turn {turn}");
            total += 1;
        }
        let mut bytes = vec![0; 1048576];
        for (number, chunk) in expected.chunks_exact(1048576).enumerate() {
            ram.read_bytes(number as u32 * 1048576, &mut bytes).unwrap();
            assert_eq!(
                bytes, chunk,
                "whole RAM chunk {number}, scenario {scenario}"
            );
        }
        ram.kernel_clear().unwrap();
        ram.heap_close().unwrap();
        assert_eq!(join(job), 0);
        client::local_record_release(ram.call_handle()).unwrap();
        println!("PASS scenario {scenario}: exact original pool bridge/register/stack/table behavior and all 64 MiB RAM; no kernel/data fallback");
    }
    local::shutdown();
    println!("PASS: {total} original pool bridge calls, per-call registers/allocator state, five whole-RAM scenarios; no title entry");
}
