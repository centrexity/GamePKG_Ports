// Maintainer-only exact C wrapper comparison; no oracle is admitted game code.
use std::sync::Mutex;
#[derive(Debug, PartialEq)]
enum RefDiagnostic {
    Numeric(u32, u32, Vec<u64>),
    Bytes(u32, u32, Vec<u8>),
}
static LOGS: Mutex<Vec<RefDiagnostic>> = Mutex::new(Vec::new());
#[allow(unused_imports)]
mod gamepkg {
    pub use crate::client::*;
    pub fn diagnostic(level: u32, code: u32, values: &[u64]) -> Result<u64, i32> {
        assert!(level <= 3 && values.len() <= 32);
        crate::LOGS
            .lock()
            .unwrap()
            .push(crate::RefDiagnostic::Numeric(level, code, values.to_vec()));
        Ok(0)
    }
    pub fn diagnostic_bytes(level: u32, code: u32, bytes: &[u8]) -> Result<u64, i32> {
        assert!(level <= 3 && bytes.len() <= 128 * 1024);
        crate::LOGS
            .lock()
            .unwrap()
            .push(crate::RefDiagnostic::Bytes(level, code, bytes.to_vec()));
        Ok(0)
    }
}
unsafe extern "C" {
    fn gamepkg_memory_reset(main: *mut u8, physical: *mut u8, base: u32, top: u32);
    fn gamepkg_heap_stats(out: *mut u32);
    fn gamepkg_contig_stats(out: *mut u32);
    fn gamepkg_contig_record(page: u32, out: *mut u32);
    fn gamepkg_pool_init(table: u32, count: u32) -> u32;
    fn gamepkg_pool_call(slot: u32, cpu: *mut u32) -> u32;
    fn gamepkg_stack_trace(out: *mut u32) -> u32;
}
fn run_heap(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn spawn(inbox: u64) -> u64 {
    let mut request = 0u32.to_le_bytes().to_vec();
    request.extend(inbox.to_le_bytes());
    request.extend(0u32.to_le_bytes());
    let mut out = [0u8; 8];
    assert_eq!(client::local_call(10, &request, &mut out), Ok(8));
    u64::from_le_bytes(out)
}
fn join(job: u64) -> u64 {
    let end = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        let mut out = [0u8; 8];
        match client::local_call(11, &job.to_le_bytes(), &mut out) {
            Ok(8) => return u64::from_le_bytes(out),
            Err(-2) => {
                assert!(std::time::Instant::now() < end);
                std::thread::yield_now();
            }
            other => panic!("{other:?}"),
        }
    }
}
struct Oracle {
    ram: memory::Memory,
    main: Vec<u8>,
    physical: Vec<u8>,
    calls: u32,
    refusals: u32,
}
impl Oracle {
    fn put(&mut self, address: u32, value: u32) {
        self.ram.write_u32(address, value).unwrap();
        let (bytes, at) = if (0x80000000..0x84000000).contains(&address) {
            (&mut self.physical, (address - 0x80000000) as usize)
        } else {
            (&mut self.main, address as usize)
        };
        bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
    fn invoke(&mut self, slot: u32, args: [u32; 5], sp: u32) -> u32 {
        for number in 0..=40u32 {
            self.put(
                sp + number * 4,
                0x12000000u32.wrapping_add(self.calls * 64 + number),
            );
        }
        for (number, arg) in args.iter().enumerate() {
            self.put(sp + 4 + number as u32 * 4, *arg);
        }
        let mut state = registers::Registers::new();
        state.eax = 0x87654321;
        state.ebx = 0x12345678;
        state.ecx = 0x55667788;
        state.edx = 0xaabbccdd;
        state.esi = 0x55;
        state.edi = 0x66;
        state.ebp = 0x44556677;
        state.esp = sp;
        state.fs_base = 0x4000;
        state.fp_top = (self.calls & 7) as i32;
        state.fp_stack[1] = f64::from_bits(0x7ff8000012345678);
        state.xmm3 = [0x80000000, 0xffffffff, 0x7fc12345, self.calls];
        let before = state.encode();
        let mut cpu = [
            state.eax, state.ebx, state.ecx, state.edx, state.esi, state.edi, state.ebp, state.esp,
        ];
        LOGS.lock().unwrap().clear();
        let caller = unsafe { gamepkg_pool_call(slot, cpu.as_mut_ptr()) };
        kernel_pool::kernel_invoke(&mut state, &mut self.ram, slot).unwrap();
        let mut expected = registers::Registers::new();
        expected.decode(&before).unwrap();
        expected.eax = cpu[0];
        expected.esp = cpu[7];
        assert_eq!(
            state.encode(),
            expected.encode(),
            "call {} slot {slot}",
            self.calls
        );
        assert_eq!(self.ram.kernel_caller(), caller);
        let mut trace = [0u32; 40];
        let reads = unsafe { gamepkg_stack_trace(trace.as_mut_ptr()) };
        let diagnostics = std::mem::take(&mut *LOGS.lock().unwrap());
        if reads == 40 {
            assert_eq!(slot, 5);
            self.refusals += 1;
            let mut bytes = Vec::new();
            for value in &trace[5..] {
                bytes.extend(value.to_le_bytes());
            }
            assert_eq!(
                diagnostics,
                vec![
                    RefDiagnostic::Numeric(
                        2,
                        0x544b0a00,
                        vec![
                            u64::from(caller),
                            u64::from(args[0]),
                            u64::from(args[1]),
                            u64::from(args[2]),
                            u64::from(args[3].max(4096)),
                            u64::from(args[4])
                        ]
                    ),
                    RefDiagnostic::Bytes(2, 0x544b0a01, bytes)
                ]
            );
        } else {
            assert!(reads <= 5);
            assert!(
                diagnostics.is_empty(),
                "unexpected refusal read/log at call {}",
                self.calls
            );
        }
        let mut ordinary = [0u32; 3];
        let mut physical = [0u32; 6];
        unsafe {
            gamepkg_heap_stats(ordinary.as_mut_ptr());
            gamepkg_contig_stats(physical.as_mut_ptr());
        }
        let mut client = self.ram.heap_take().unwrap();
        assert_eq!(client.heap_status().unwrap(), ordinary);
        for index in 0..6 {
            assert_eq!(
                client.heap_contiguous_counter(index).unwrap(),
                physical[index as usize]
            );
        }
        self.ram.heap_put(client);
        self.calls += 1;
        state.eax
    }
}
fn main() {
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
        vec![run_heap],
        256 * 1024 * 1024,
        8,
        1048576,
    )
    .unwrap();
    let windows = runtime.windows().unwrap();
    for &(id, bytes) in &windows[..2] {
        runtime.commit(id, 0, bytes, 2).unwrap();
        runtime.fill(id, 0, bytes, 0xa5).unwrap();
    }
    local::install(runtime.clone()).unwrap();
    let mut ram = memory::Memory::new().unwrap();
    ram.attach(0, 67108864, windows[0].0, 0, true).unwrap();
    ram.attach(0x80000000, 67108864, windows[1].0, 0, true)
        .unwrap();
    ram.attach(0xf0000000, 67108864, windows[1].0, 0, true)
        .unwrap();
    let inbox = heap_service::heap_inbox().unwrap();
    let job = spawn(inbox);
    let mut client = heap_service::HeapClient::heap_client(inbox, job).unwrap();
    client
        .heap_initialize(ram.call_handle(), windows[0].0, 16777216, 67108864)
        .unwrap();
    client.heap_contiguous_initialize(windows[1].0).unwrap();
    let mut owner = Some(client);
    ram.heap_install(&mut owner).unwrap();
    let mut oracle = Oracle {
        ram,
        main: vec![0xa5; 67108864],
        physical: vec![0xa5; 67108864],
        calls: 0,
        refusals: 0,
    };
    unsafe {
        gamepkg_memory_reset(
            oracle.main.as_mut_ptr(),
            oracle.physical.as_mut_ptr(),
            16777216,
            67108864,
        );
    }
    for (slot, ordinal) in [14u32, 15, 17, 23, 165, 166, 168, 171, 180, 0]
        .iter()
        .enumerate()
    {
        oracle.put(
            0x20000 + slot as u32 * 4,
            if *ordinal == 0 {
                0
            } else {
                0x80000000 | ordinal
            },
        );
    }
    assert_eq!(unsafe { gamepkg_pool_init(0x20000, 10) }, 18);
    kernel_pool::kernel_prepare(&mut oracle.ram, 0x20000, 10).unwrap();
    for iteration in 0..256u32 {
        let sp = if iteration % 13 == 0 {
            0
        } else {
            0x30000 + (iteration & 3)
        };
        let ordinary = oracle.invoke(
            iteration & 1,
            [64 + (iteration & 31), 0xabcdef01, 0, 0, 0],
            sp,
        );
        let physical = oracle.invoke(4, [iteration % 8193, 0, 0, 0, 0], sp);
        oracle.invoke(8, [ordinary + 1, 0, 0, 0, 0], sp);
        oracle.invoke(8, [physical + 1, 0, 0, 0, 0], sp);
        oracle.invoke(7, [ordinary, 0, 0, 0, 0], sp);
        oracle.invoke(7, [physical, 0, 0, 0, 0], sp);
        oracle.invoke(8, [ordinary, 0, 0, 0, 0], sp);
        oracle.invoke(8, [physical, 0, 0, 0, 0], sp);
    }
    for (size, low, high, align) in [
        (4096u32, 0x200003u32, 0x201002u32, 1u32),
        (4096, 0x200003, 0x201002, 0x80000000),
        (0, 1, 1, 0),
        (4096, 0x3fff000, 0x3ffffff, 0),
        (8192, 0, 0xffffff, 4097),
        (4096, 0x100000, 0x80000, 4096),
    ] {
        let address = oracle.invoke(5, [size, low, high, align, 0x11223344], 0x31001);
        if address != 0 {
            oracle.invoke(8, [address, 0, 0, 0, 0], 0x31002);
            oracle.invoke(7, [address & !4095, 0, 0, 0, 0], 0x31003);
        }
    }
    let zero_stack = oracle.invoke(5, [4096, 0, u32::MAX, 4096, 0x9988], 0);
    oracle.invoke(7, [zero_stack, 0, 0, 0, 0], 0);
    oracle.invoke(6, [u32::MAX, 0x35000, 0, 0, 0], 0);
    for bytes in [
        0u32,
        1,
        4096,
        131072,
        67108864,
        67108865,
        0x80000000,
        0xfffffffe,
        u32::MAX,
    ] {
        for padding in [0u32, 0x35000, 0x35001, 0x80035000] {
            oracle.invoke(6, [bytes, padding, 0, 0, 0], 0x32000);
        }
    }
    for turn in 0..12u32 {
        oracle.invoke(
            5,
            [
                u32::MAX,
                0,
                u32::MAX,
                if turn & 1 == 0 { 0 } else { 4097 },
                0x55aa,
            ],
            if turn == 0 { 0 } else { 0x33000 + (turn & 3) },
        );
    }
    assert_eq!(oracle.refusals, 8);
    let mut actual = vec![0u8; 1048576];
    for (index, &(id, _)) in windows[..2].iter().enumerate() {
        let expected = if index == 0 {
            &oracle.main
        } else {
            &oracle.physical
        };
        for (number, chunk) in expected.chunks_exact(1048576).enumerate() {
            runtime
                .read(id, number as u64 * 1048576, &mut actual)
                .unwrap();
            assert_eq!(&actual, chunk, "bank {index} chunk {number}");
        }
    }
    let mut actor = oracle.ram.heap_take().unwrap();
    for page in 0..16384u32 {
        let mut record = [0u32; 2];
        unsafe {
            gamepkg_contig_record(page, record.as_mut_ptr());
        }
        assert_eq!(
            actor.heap_contiguous_pages(page, 1).unwrap()[0] as u32,
            record[0]
        );
        assert_eq!(actor.heap_contiguous_length(page).unwrap(), record[1]);
    }
    oracle.ram.heap_put(actor);
    oracle.ram.kernel_clear().unwrap();
    oracle.ram.heap_close().unwrap();
    assert_eq!(join(job), 0);
    local::shutdown();
    println!("PASS: {} original allocator kernel calls, full CPU/caller/stack results, both 64 MiB banks, every page record/counter and eight exact refusal dumps; no title entry",oracle.calls);
}
