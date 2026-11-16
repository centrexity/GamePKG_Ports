// Maintainer C-reference shell. Never admitted game code.
mod gamepkg {
    pub use crate::client::*;
}
unsafe extern "C" {
    fn callback_reference(index: u32, initial: *const u8, params: *const u32, out: *mut u8);
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    probe::callback_worker(arg)
}
fn main() {
    local::install(
        local::runtime::Runtime::new(
            &[local::runtime::Window {
                address: 0x20000000,
                bytes: 524288,
            }],
            vec![worker],
            8 * 1024 * 1024,
            4,
            65536,
        )
        .unwrap(),
    )
    .unwrap();
    // Deliberately fill the one-slot channel: backpressure is retryable and
    // still bounded. No capacity increase, dropped witness or direct game API.
    let mailbox = client::local_mailbox_create(1, 1).unwrap();
    client::local_mailbox_send(mailbox, &[1]).unwrap();
    assert_eq!(probe::callback_send(mailbox, &[2]), Err(-2));
    let consumer = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut value = [0u8; 1];
        assert_eq!(
            client::local_mailbox_receive(mailbox, 1000, &mut value),
            Ok(1)
        );
        assert_eq!(value, [1]);
    });
    probe::callback_send(mailbox, &[2]).unwrap();
    consumer.join().unwrap();
    let mut value = [0u8; 1];
    assert_eq!(
        client::local_mailbox_receive(mailbox, 1000, &mut value),
        Ok(1)
    );
    assert_eq!(value, [2]);
    client::local_mailbox_release(mailbox).unwrap();
    let actual = probe::callback_probe().unwrap();
    assert_eq!(actual.len(), 256 * 336);
    let mut vectors = Vec::new();
    for index in 0..256u32 {
        let initial = cases::callback_initial(0x7bfff0).encode();
        let params = cases::callback_params(index);
        let mut expected = [0u8; 336];
        unsafe {
            callback_reference(
                index,
                initial.as_ptr(),
                params.as_ptr(),
                expected.as_mut_ptr(),
            );
        }
        assert_eq!(
            &actual[index as usize * 336..index as usize * 336 + 336],
            &expected,
            "callback vector {index}"
        );
        vectors.extend(expected);
    }
    if let Some(path) = std::env::args_os().nth(1) {
        std::fs::write(path, vectors).unwrap();
    }
    local::shutdown();
    println!("PASS 256 original DPC CPU/full-RAM/return/profile-order witnesses; actual retained worker stack and IRQL cleanup, explicit unsupported and bad-return/body/observer/readonly controls. Synthetic finite callbacks, not title gameplay.");
}
