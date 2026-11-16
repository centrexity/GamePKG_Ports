// Maintainer-only silent local runtime shell; the actual fixture is also admitted separately.
mod gamepkg {
    pub use crate::client::*;
    pub static FAULT:std::sync::Mutex<Option<[u64;2]>>=std::sync::Mutex::new(None);
    pub fn local_performance_sample()->Result<PerformanceSample,i32>{
        let fault=FAULT.lock().unwrap().take();
        if let Some([root,services])=fault{
            let (version,bytes)=crate::client::local_record_snapshot(root,288)?;
            let mut state=crate::call_state::CallState::call_decode(&bytes)?;
            state.call_set_services(services,0)?;
            crate::client::local_record_compare_exchange(root,version,&state.call_encode())?;
            crate::client::local_record_release(services)?;
        }
        crate::client::local_performance_sample()
    }
    pub fn diagnostic(_:u32,code:u32,values:&[u64])->Result<u64,i32>{eprintln!("pacing {code:x}: {values:?}");Ok(1)}
    pub fn fullscreen(_:bool)->Result<(),i32>{std::thread::sleep(std::time::Duration::from_millis(60));Err(-5)}
}
fn worker(_:local::runtime::TaskContext,arg:u64)->u64{probe::pacing_worker(arg)}
fn injected_worker(_:local::runtime::TaskContext,arg:u64)->u64{
    let (_,context)=client::local_record_snapshot(arg,16).unwrap();
    let root=u64::from_le_bytes(context[..8].try_into().unwrap());let id=u64::from_le_bytes(context[8..16].try_into().unwrap());
    let ram=memory::Memory::with_call_handle(root).unwrap();let schedule=shared::Schedule::schedule_borrow(&ram,id).unwrap();
    let before=client::local_record_snapshot(id,144).unwrap();
    *gamepkg::FAULT.lock().unwrap()=Some([root,ram.services_handle().unwrap()]);
    assert_eq!(schedule.schedule_step(&ram),Err(-4));
    assert!(gamepkg::FAULT.lock().unwrap().is_none());
    assert_eq!(client::local_record_snapshot(id,144).unwrap(),before);
    23
}
fn forced_revocation(){
    let ram=memory::Memory::new().unwrap();let services=kernel_pool::kernel_services_initialize(&ram,0x10000,2).unwrap();
    let mut owner=shared::Schedule::schedule_new(&ram,0).unwrap();
    let context=client::local_record_create(&[ram.call_handle().to_le_bytes(),owner.schedule_id().to_le_bytes()].concat()).unwrap();
    let request=[1u32.to_le_bytes().as_slice(),&context.to_le_bytes(),&1u32.to_le_bytes()].concat();
    let task=local::call(10,&request,8).unwrap();let task=u64::from_le_bytes(task.try_into().unwrap());
    owner.schedule_assign(&ram,task).unwrap();local::call(13,&task.to_le_bytes(),4).unwrap();
    let end=std::time::Instant::now()+std::time::Duration::from_secs(5);
    loop{
        match owner.schedule_join(&ram){Ok(value)=>{assert_eq!(value,23);break;},Err(-2)=>std::thread::yield_now(),value=>panic!("{value:?}")}
        assert!(std::time::Instant::now()<end);
    }
    assert_eq!(owner.schedule_close(&ram),Err(-4));assert!(owner.schedule_closed());
    services.service_close(&ram).unwrap();client::local_record_release(context).unwrap();client::local_record_release(ram.call_handle()).unwrap();
}
fn main(){
    local::install(local::runtime::Runtime::new(&[],vec![worker,injected_worker],4*1024*1024,4,65536).unwrap()).unwrap();
    if std::env::var_os("GAMEPKG_PACING_REGRESSION_ONLY").is_some(){forced_revocation();local::shutdown();return;}
    let repeats=std::env::var("GAMEPKG_PACING_REPEATS").unwrap_or_else(|_|"1".into()).parse::<u32>().unwrap();assert!((1..=32).contains(&repeats));
    for round in 0..repeats{eprintln!("pacing round {round}");probe::pacing_probe().unwrap();}
    forced_revocation();local::shutdown();
    println!("PASS: actual reviewed pacing worker, copied root/config-bound state, concurrent refresh updates, pause restart, role/metadata denials, cancellation, joined cleanup and revocation; synthetic window stall only");
}
