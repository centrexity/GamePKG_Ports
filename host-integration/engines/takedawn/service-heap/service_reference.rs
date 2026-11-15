// Maintainer-only runtime table. The admitted build injects its own fixed table.
fn run_heap(_: local::runtime::TaskContext, arg: u64) -> u64 {
    heap_service::heap_serve(arg)
}
fn run_client(_: local::runtime::TaskContext, arg: u64) -> u64 {
    service_checks::heap_test_client(arg)
}
fn run_fake(_: local::runtime::TaskContext, arg: u64) -> u64 {
    service_checks::heap_test_fake(arg)
}
fn run_waiting_peer(_: local::runtime::TaskContext, arg: u64) -> u64 {
    service_checks::heap_test_waiting_peer(arg)
}
fn run_cancelled_caller(_: local::runtime::TaskContext, arg: u64) -> u64 {
    service_checks::heap_test_cancelled_caller(arg)
}
fn run_abandoned_client(_: local::runtime::TaskContext, arg: u64) -> u64 {
    service_checks::heap_test_abandoned_client(arg)
}
fn main() {
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 67108864,
        }],
        vec![
            run_heap,
            run_client,
            run_fake,
            run_waiting_peer,
            run_cancelled_caller,
            run_abandoned_client,
        ],
        128 * 1024 * 1024,
        32,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    assert_eq!(service_checks::heap_service_verify(), Ok(2000));
    local::shutdown();
    println!("PASS: one heap owner, 2000 concurrent allocation lifetimes, correlation/replay/membership/backpressure controls, setup/revocation/failed-peer and cancellation cleanup; no title entry");
}
