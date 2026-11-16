// Maintainer-only ownership/cancellation shell. Never admitted game source.
mod gamepkg {
    pub use crate::client::*;
}
static MOVED: std::sync::Mutex<Option<queue::Take>> = std::sync::Mutex::new(None);
static BARRIER: std::sync::OnceLock<std::sync::Barrier> = std::sync::OnceLock::new();
fn word(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap())
}
fn worker(_: local::runtime::TaskContext, context: u64) -> u64 {
    let (_, bytes) = client::local_record_snapshot(context, 32).unwrap();
    let mut ram = memory::Memory::with_call_handle(word(&bytes, 0)).unwrap();
    ram.attach(0x740000, 65536, word(&bytes, 8), 0, true)
        .unwrap();
    let pending = queue::Queue::queue_borrow(&ram, word(&bytes, 16)).unwrap();
    let mut observation = pending.queue_take_begin(&ram).unwrap().unwrap();
    if word(&bytes, 24) == 0 {
        *MOVED.lock().unwrap() = Some(observation);
        BARRIER.get().unwrap().wait();
        BARRIER.get().unwrap().wait();
        observation = MOVED.lock().unwrap().take().unwrap();
        assert_eq!(observation.take_depth(&ram), Ok(1));
        assert_eq!(observation.take_publish(&mut ram), Ok([0x740100, 7, 8, 0]));
    } else {
        BARRIER.get().unwrap().wait();
        BARRIER.get().unwrap().wait();
        assert!(client::local_task_status().unwrap().1);
        assert_eq!(observation.take_publish(&mut ram), Err(-4));
    }
    assert!(observation.take_closed(), "owned cancellation cleanup");
    0
}
fn main() {
    BARRIER.set(std::sync::Barrier::new(2)).unwrap();
    local::install(
        local::runtime::Runtime::new(
            &[local::runtime::Window {
                address: 0x20000000,
                bytes: 65536,
            }],
            vec![worker],
            4 * 1024 * 1024,
            4,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    let mut windows = [0u8; 16];
    assert_eq!(client::local_call(1, &[], &mut windows), Ok(16));
    let window = word(&windows, 0);
    client::local_call(
        2,
        &[
            window.to_le_bytes().as_slice(),
            &0u64.to_le_bytes(),
            &65536u64.to_le_bytes(),
            &2u32.to_le_bytes(),
        ]
        .concat(),
        &mut [],
    )
    .unwrap();
    for _ in 0..32 {
        for mode in 0..2u64 {
            let mut ram = memory::Memory::new().unwrap();
            ram.attach(0x740000, 65536, window, 0, true).unwrap();
            ram.write_u8(0x740102, 0).unwrap();
            let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
            let mut pending = queue::Queue::queue_new(&ram, false).unwrap();
            assert_eq!(pending.queue_insert(&mut ram, 0x740100, 7, 8), Ok([1, 0]));
            let before = pending.queue_state(&ram).unwrap();
            let mutex = word(&before, 24);
            let context = client::local_record_create(
                &[
                    ram.call_handle().to_le_bytes(),
                    window.to_le_bytes(),
                    pending.queue_id().to_le_bytes(),
                    mode.to_le_bytes(),
                ]
                .concat(),
            )
            .unwrap();
            let mut id = [0u8; 8];
            client::local_call(
                10,
                &[
                    0u32.to_le_bytes().as_slice(),
                    &context.to_le_bytes(),
                    &1u32.to_le_bytes(),
                ]
                .concat(),
                &mut id,
            )
            .unwrap();
            let id = u64::from_le_bytes(id);
            pending.queue_assign(&ram, id).unwrap();
            let before = pending.queue_state(&ram).unwrap();
            let mut reply = [0u8; 4];
            client::local_call(13, &id.to_le_bytes(), &mut reply).unwrap();
            BARRIER.get().unwrap().wait();
            if mode == 0 {
                let mut observation = MOVED.lock().unwrap().take().unwrap();
                assert_eq!(
                    observation.take_depth(&ram),
                    Err(-4),
                    "wrong-task observation"
                );
                assert_eq!(observation.take_entry(&ram), Err(-4), "wrong-task entry");
                assert_eq!(
                    observation.take_publish(&mut ram),
                    Err(-4),
                    "wrong-task publication"
                );
                assert_eq!(observation.take_abort(), Err(-4), "wrong-task unlock");
                assert!(!observation.take_closed());
                assert_eq!(pending.queue_state(&ram).unwrap(), before);
                *MOVED.lock().unwrap() = Some(observation);
            } else {
                assert_eq!(client::local_call(12, &id.to_le_bytes(), &mut []), Ok(0));
            }
            assert_eq!(client::local_mutex_lock(mutex, 0), Err(-2));
            assert_eq!(pending.queue_close(&ram), Err(-2));
            BARRIER.get().unwrap().wait();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while !client::local_task_finished(id).unwrap() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::yield_now();
            }
            assert_eq!(pending.queue_join(&ram), Ok(0), "lifecycle join");
            client::local_mutex_lock(mutex, 0).unwrap();
            client::local_call(24, &mutex.to_le_bytes(), &mut []).unwrap();
            assert_eq!(pending.queue_nonempty(&ram), Ok(mode == 1));
            assert_eq!(ram.read_u8(0x740102), Ok(mode as u8));
            pending.queue_close(&ram).unwrap();
            services.service_close(&ram).unwrap();
            client::local_record_release(context).unwrap();
            client::local_record_release(ram.call_handle()).unwrap();
        }
    }
    local::shutdown();
    println!("PASS 64 actual worker lifetimes: moved observation rejects foreign-task read/publish/unlock; cancellation denies publication but releases owned depth. Maintainer-only shell, not confined game or gameplay.");
}
