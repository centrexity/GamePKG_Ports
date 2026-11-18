// Maintainer-only controlled native composition fixture, never production device code.
mod gamepkg {
    pub use crate::client::*;
}
fn worker(_: local::runtime::TaskContext, arg: u64) -> u64 {
    races::irq_race_worker(arg)
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 0x4000000,
        }],
        vec![worker],
        128 * 1048576,
        4,
        65536,
    )
    .unwrap();
    let window = runtime.windows().unwrap()[0].0;
    runtime.commit(window, 0x740000, 65536, 2).unwrap();
    runtime.commit(window, 0x780000, 1048576, 2).unwrap();
    local::install(runtime).unwrap();
    for _ in 0..32 {
        let mut ram = memory::Memory::new().unwrap();
        ram.attach(0, 0x4000000, window, 0, true).unwrap();
        let services = kernel_pool::kernel_services_initialize(&ram, 0x10000, 2).unwrap();
        probe::irq_probe(&mut ram, window).unwrap();
        races::irq_races(&mut ram, window).unwrap();
        controls::irq_controls(&mut ram, window).unwrap();
        options::irq_options(&mut ram, window).unwrap();
        callbacks::irq_callbacks_probe(&mut ram, window).unwrap();
        callbacks::irq_thread_patch_probe(&mut ram, window).unwrap();
        services.service_close(&ram).unwrap();
        client::local_record_release(ram.call_handle()).unwrap();
    }
    local::shutdown();
    println!("PASS: controlled IRQ composition, 128 interleavings, 192 option cases, 64 callback/new-post/cancel cases, 128 native thread-patch cases and lifecycle controls; checked fault seams, not production devices/gameplay");
}
