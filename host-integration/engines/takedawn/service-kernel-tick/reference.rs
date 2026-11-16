// Maintainer-only synthetic host shell; admitted fixtures use the actual SDK.
mod gamepkg {
    pub use crate::client::*;
    pub fn diagnostic(_:u32,code:u32,values:&[u64])->Result<u64,i32>{eprintln!("tick stage {code:x}: {values:?}");Ok(1)}
    pub fn fullscreen(_:bool)->Result<(),i32>{std::thread::sleep(std::time::Duration::from_millis(50));Err(-5)}
}
unsafe extern "C" { fn tick_reference(milliseconds:u64,nanoseconds:u64)->u32; }
fn worker(_:local::runtime::TaskContext,arg:u64)->u64{tick_service::tick_worker(arg)}
fn main(){
    let edges=[0,1,999999,1000000,1000001,u32::MAX as u64,1u64<<32,(1u64<<32)+1,i64::MAX as u64,u64::MAX];
    let mut seed=0xaabcdedfu64;
    for index in 0..2048usize {
        seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let uptime=if index<100{edges[index%10]}else{seed};
        let skew=if index<100{edges[index/10]}else{seed.rotate_left(29)};
        assert_eq!(tick_service::tick_value(uptime,skew),unsafe{tick_reference(uptime,skew)},"{index}");
    }
    local::install(local::runtime::Runtime::new(&[local::runtime::Window{address:0x20000000,bytes:65536}],vec![worker],4*1024*1024,4,65536).unwrap()).unwrap();
    probe::tick_probe().unwrap();
    local::shutdown();
    println!("PASS: 2048 exact original tick-word arithmetic vectors; actual child-local periodic worker, stopped-join/no-scribble checks, live raw-uptime initialization, skew changes, read-only and foreign-root denials, 132 successful/partial-start cleanup cycles, record/event/task quota exhaustion, malformed metadata and revoked-root cleanup; synthetic window delay only, not real broker or title gameplay");
}
