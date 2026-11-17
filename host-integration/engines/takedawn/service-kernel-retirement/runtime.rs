// Maintainer-only deterministic interference causing actual public task cancellation.
// Never admitted as game source; outer runtime teardown owns the remaining root.
mod gamepkg {
    pub use crate::client::*;
    static TARGET: std::sync::Mutex<[u64; 2]> = std::sync::Mutex::new([0; 2]);
    pub fn arm(id: u64, kind: u64) {
        *TARGET.lock().unwrap() = [id, kind];
    }
    fn interfere(id: u64, kind: u64) {
        let mut v = TARGET.lock().unwrap();
        if *v == [id, kind] {
            *v = [0; 2];
            drop(v);
            let task = crate::client::local_task_status().unwrap().0;
            assert_ne!(task, 0);
            crate::client::local_call(12, &task.to_le_bytes(), &mut []).unwrap();
        }
    }
    pub fn local_event_state(id: u64) -> Result<LocalEventState, i32> {
        interfere(id, 1);
        crate::client::local_event_state(id)
    }
    pub fn local_semaphore_state(id: u64) -> Result<LocalSemaphoreState, i32> {
        interfere(id, 3);
        crate::client::local_semaphore_state(id)
    }
}
fn worker(_: local::runtime::TaskContext, box_id: u64) -> u64 {
    let mut f = setup::Fixture::mixed_fixture_new().unwrap();
    let mode = client::local_mailbox_receive(box_id, 1000, &mut [0u8; 1]).unwrap();
    assert_eq!(mode, 1);
    let kind = std::env::args().nth(1).unwrap().parse::<u64>().unwrap();
    let item = if kind == 1 {
        f.mixed_event(false, 1).unwrap()
    } else {
        f.mixed_semaphore(1, 2).unwrap()
    };
    client::local_mailbox_send(box_id, &item[0].to_le_bytes()).unwrap();
    gamepkg::arm(item[0], kind);
    let result = if kind == 1 {
        f.events.events_close(&f.ram, &f.table)
    } else {
        f.semaphores.semaphores_close(&f.ram, &f.table)
    };
    assert_eq!(result, Err(-4));
    assert!(if kind == 1 {
        f.events.events_closed()
    } else {
        f.semaphores.semaphores_closed()
    });
    71
}
fn main() {
    let r = local::runtime::Runtime::new(
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
            local::runtime::Window {
                address: 0x29000000,
                bytes: 1048576,
            },
        ],
        vec![worker],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(r).unwrap();
    let mailbox = client::local_mailbox_create(8, 2).unwrap();
    client::local_mailbox_send(mailbox, &[0]).unwrap();
    let mut out = [0u8; 8];
    client::local_call(
        10,
        &[
            0u32.to_le_bytes().as_slice(),
            &mailbox.to_le_bytes(),
            &0u32.to_le_bytes(),
        ]
        .concat(),
        &mut out,
    )
    .unwrap();
    let task = u64::from_le_bytes(out);
    client::local_wait_many(
        &[client::LocalWaitObject { id: task, kind: 4 }],
        false,
        1000,
    )
    .unwrap();
    assert_eq!(client::local_task_join(task).unwrap().result(), 71);
    assert_eq!(client::local_mailbox_receive(mailbox, 0, &mut out), Ok(8));
    let id = u64::from_le_bytes(out);
    let kind = std::env::args().nth(1).unwrap().parse::<u64>().unwrap();
    let actual = if kind == 1 {
        client::local_event_state(id).map(|_| ())
    } else {
        client::local_semaphore_state(id).map(|_| ())
    };
    assert_eq!(
        actual,
        Err(-4),
        "typed retirement must remove the actual object"
    );
    println!("Cancelled creator kind {kind}: domain closed with observed Denied; actual native object is gone");
    local::shutdown();
}
