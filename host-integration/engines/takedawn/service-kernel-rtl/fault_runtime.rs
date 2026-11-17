// Maintainer-only one-shot SDK read failure; never allowed as game source.
mod gamepkg {
    pub use crate::client::*;
    static FAILURE: std::sync::Mutex<i32> = std::sync::Mutex::new(0);
    pub fn arm(code: i32) {
        *FAILURE.lock().unwrap() = code;
    }
    pub fn done() -> bool {
        *FAILURE.lock().unwrap() == 0
    }
    pub fn local_call(op: u32, request: &[u8], output: &mut [u8]) -> Result<usize, i32> {
        let mut failure = FAILURE.lock().unwrap();
        if *failure != 0
            && op == 5
            && request.len() == 20
            && u64::from_le_bytes(request[8..16].try_into().unwrap()) == 0x18000
            && u32::from_le_bytes(request[16..20].try_into().unwrap()) == 256
        {
            let code = *failure;
            *failure = 0;
            return Err(code);
        }
        drop(failure);
        crate::client::local_call(op, request, output)
    }
}
fn main() {
    let mode = std::env::args().nth(1).unwrap().parse::<i32>().unwrap();
    assert!((1..=5).contains(&mode));
    let runtime = local::runtime::Runtime::new(
        &[local::runtime::Window {
            address: 0x20000000,
            bytes: 67108864,
        }],
        vec![],
        256 * 1024 * 1024,
        4,
        1048576,
    )
    .unwrap();
    local::install(runtime).unwrap();
    let mut fixture = setup::Fixture::fixture_new().unwrap();
    fixture.ram.write_bytes(0x18000, &[65u8; 512]).unwrap();
    fixture.ram.write_u8(0x18200, 0).unwrap();
    fixture.ram.write_bytes(0x3000, &[0xccu8; 8]).unwrap();
    gamepkg::arm(-mode);
    rtl_probe::rtl_probe_error(&mut fixture.ram, 2, &[0x3000, 0x18000], -mode).unwrap();
    assert!(gamepkg::done());
    let mut bytes = [0u8; 8];
    fixture.ram.read_bytes(0x3000, &mut bytes).unwrap();
    assert_eq!(bytes, [0xcc; 8]);
    fixture.fixture_close().unwrap();
    local::shutdown();
    println!("PASS one-shot SDK failure {mode} is fatal without smaller-read retry or CPU/output success");
}
