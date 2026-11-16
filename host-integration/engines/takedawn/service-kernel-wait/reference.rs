// Maintainer-only platform shell. Never part of admitted game sources.
mod gamepkg {
    pub use crate::client::*;
    pub static FAULT:std::sync::Mutex<Option<[u64;3]>>=std::sync::Mutex::new(None);
    pub fn local_record_snapshot(id:u64,maximum:usize)->Result<(u64,Vec<u8>),i32>{
        let injection={
            let mut fault=FAULT.lock().unwrap();
            if let Some(value)=fault.as_mut(){
                if id==value[0]{value[2]-=1;if value[2]==0{fault.take()}else{None}}else{None}
            }else{None}
        };
        if let Some([root,services,0])=injection{
            let (version,bytes)=crate::client::local_record_snapshot(root,288)?;
            let mut state=crate::call_state::CallState::call_decode(&bytes)?;
            state.call_set_services(services,0)?;
            crate::client::local_record_compare_exchange(root,version,&state.call_encode())?;
            crate::client::local_record_release(services)?;
        }
        crate::client::local_record_snapshot(id,maximum)
    }
    pub fn diagnostic(_:u32,code:u32,values:&[u64])->Result<u64,i32>{eprintln!("wait stage {code:x}: {values:?}");Ok(1)}
    pub fn fullscreen(_:bool)->Result<(),i32>{std::thread::sleep(std::time::Duration::from_millis(60));Err(-5)}
}
unsafe extern "C" { fn wait_reference(counter:u64,frequency:u64,skew:u64,host:*mut u64,guest:*mut u64); }
fn worker(_:local::runtime::TaskContext,arg:u64)->u64{probe::wait_worker(arg)}
static MOVED:std::sync::Mutex<Option<(wait_service::Sleeper,u64)>>=std::sync::Mutex::new(None);
fn foreign_task(_:local::runtime::TaskContext,_:u64)->u64{
    let (mut sleeper,root)=MOVED.lock().unwrap().take().unwrap();
    let ram=memory::Memory::with_call_handle(root).unwrap();
    let observed=sleeper.sleep_observed();
    assert_eq!(sleeper.sleep_pending(&ram),Err(-4));
    assert_eq!(sleeper.sleep_ns(&ram,0),Err(-4));
    assert_eq!(sleeper.sleep_observed(),observed);
    23
}
fn forced_revocation_controls(){
    for mode in 0..2u32 {
        let ram=memory::Memory::new().unwrap();
        let services=kernel_pool::kernel_services_initialize(&ram,0x10000,2).unwrap();
        let mut sleeper=wait_service::Sleeper::sleep_new(&ram).unwrap();
        let observed=sleeper.sleep_observed();
        assert_eq!(wait_service::sleep_wake(&ram),Ok(observed+1));
        *gamepkg::FAULT.lock().unwrap()=Some([ram.call_handle(),services.service_handle(),2]);
        if mode==0{assert_eq!(sleeper.sleep_pending(&ram),Err(-4));}
        else{assert_eq!(sleeper.sleep_ns(&ram,0),Err(-4));}
        assert!(gamepkg::FAULT.lock().unwrap().is_none());
        assert_eq!(sleeper.sleep_observed(),observed);
        assert_eq!(client::local_clock_state().unwrap().generation,observed+1);
        services.service_close(&ram).unwrap();client::local_record_release(ram.call_handle()).unwrap();
    }
}
fn main(){
    let mut vectors=Vec::new();
    for index in 0..4096u32 {
        let [counter,frequency,skew]=cases::wait_case(index);
        let mut host=0;let mut guest=0;
        unsafe{wait_reference(counter,frequency,skew,&mut host,&mut guest);}
        assert_eq!(wait_service::host_time_value(counter,frequency),Ok(host),"{index}");
        assert_eq!(wait_service::guest_time_value(host,skew),guest,"{index}");
        vectors.extend(host.to_le_bytes());vectors.extend(guest.to_le_bytes());
        for ns in [0u64,1,99,100,999999,1000000,16666667,999999999,1000000000] {
            match wait_service::sleep_deadline(counter,frequency,ns) {
                Ok(deadline)=>{
                    assert!(deadline>=counter && deadline<=i64::MAX as u64);
                    let counts=deadline-counter;
                    assert!(counts<=frequency);
                    assert!(u128::from(counts)*1000000000>=u128::from(ns)*u128::from(frequency));
                    assert!(counts==0 || u128::from(counts-1)*1000000000<u128::from(ns)*u128::from(frequency));
                }
                Err(-1)=>assert!(u128::from(counter)*1000000000+u128::from(ns)*u128::from(frequency)>u128::from(i64::MAX as u64)*1000000000),
                other=>panic!("{other:?}"),
            }
        }
    }
    for (counter,frequency,ns) in [(u64::MAX,1,0),(0,0,0),(0,u64::MAX,0),(0,1,1000000001)] {
        assert_eq!(wait_service::sleep_deadline(counter,frequency,ns),Err(-1));
    }
    assert_eq!(wait_service::host_time_value(0,0),Err(-1));
    assert_eq!(wait_service::host_time_value(u64::MAX,1),Err(-1));
    assert_eq!(wait_service::host_time_value(0,u64::MAX),Err(-1));
    if let Some(path)=std::env::args_os().nth(1){std::fs::write(path,&vectors).unwrap();}
    local::install(local::runtime::Runtime::new(&[],vec![worker,foreign_task],4*1024*1024,4,65536).unwrap()).unwrap();
    if std::env::var_os("GAMEPKG_WAIT_REGRESSION_ONLY").is_some(){
        forced_revocation_controls();local::shutdown();return;
    }
    let repeats=std::env::var("GAMEPKG_WAIT_REPEATS").unwrap_or_else(|_|"1".into()).parse::<u32>().unwrap();
    assert!((1..=64).contains(&repeats));
    for round in 0..repeats {
        eprintln!("wait round {round}");
        probe::wait_probe().unwrap();
        probe::wait_blocked_probe().unwrap();
    }
    let ram=memory::Memory::new().unwrap();
    let services=kernel_pool::kernel_services_initialize(&ram,0x10000,2).unwrap();
    let sleeper=wait_service::Sleeper::sleep_new(&ram).unwrap();
    let observed=sleeper.sleep_observed();
    *MOVED.lock().unwrap()=Some((sleeper,ram.call_handle()));
    let request=[1u32.to_le_bytes().as_slice(),&0u64.to_le_bytes(),&0u32.to_le_bytes()].concat();
    let task=local::call(10,&request,8).unwrap();
    let end=std::time::Instant::now()+std::time::Duration::from_secs(5);
    loop {
        match local::call(11,&task,8){Ok(bytes)=>{assert_eq!(bytes,23u64.to_le_bytes());break;},Err(local::runtime::Error::Busy)=>{},other=>panic!("{other:?}")}
        assert!(std::time::Instant::now()<end);
        std::thread::yield_now();
    }
    assert_eq!(client::local_clock_state().unwrap().generation,observed);
    services.service_close(&ram).unwrap();client::local_record_release(ram.call_handle()).unwrap();
    forced_revocation_controls();
    local::shutdown();
    println!("PASS: 4096 unchanged original host/guest clock vectors, 36864 raw deadline bound/rounding properties, 64 coalesced-wake cycles, 64 reviewed worker wake handoffs, cancellation, foreign/revoked/malformed owners and stopped worker cleanup; synthetic window delay only");
}
