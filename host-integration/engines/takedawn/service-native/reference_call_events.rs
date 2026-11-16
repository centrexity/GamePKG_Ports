// Maintainer reference collector only. Not admitted game source or SDK code.
use std::sync::Mutex;
static ORIGINAL_EVENTS: Mutex<Vec<(u32,u32,Vec<u64>)>>=Mutex::new(Vec::new());
static PORTED_EVENTS: Mutex<Vec<(u32,u32,Vec<u64>)>>=Mutex::new(Vec::new());
static PORTED_BYTE_EVENTS: Mutex<Vec<(u32,u32,Vec<u8>)>>=Mutex::new(Vec::new());
mod gamepkg {
    pub use crate::client::{clock_sample,local_performance_sample,local_memory_page_states,zeroize,local_call,local_memory_copy,local_memory_fill,local_memory_read_segments,local_memory_write_segments,local_record_create,local_record_snapshot,local_record_compare_exchange,local_record_release,local_task_status,local_task_finished,local_mailbox_create,local_mailbox_send,local_mailbox_receive,local_mailbox_release};
    pub fn diagnostic_bytes(level:u32,code:u32,bytes:&[u8])->Result<u64,i32> {
        assert!(level<=3 && bytes.len()<=128*1024);
        let mut events=crate::PORTED_BYTE_EVENTS.lock().unwrap();events.push((level,code,bytes.to_vec()));Ok(events.len() as u64)
    }
    pub fn diagnostic(level:u32,code:u32,values:&[u64])->Result<u64,i32> {
        assert!(level<=3 && values.len()<=32);
        let mut events=crate::PORTED_EVENTS.lock().unwrap();events.push((level,code,values.to_vec()));Ok(events.len() as u64)
    }
}
#[no_mangle]
pub extern "C" fn gamepkg_reference_noncode(target:u32,hits:u64,total:u64) {
    ORIGINAL_EVENTS.lock().unwrap().push((2,0x544b0001,vec![u64::from(target),hits,total]));
}
#[no_mangle]
pub extern "C" fn gamepkg_reference_missing(target:u32,total:u64) {
    let (trace,index,observed)=unsafe{crate::oracle::call_snapshot()};assert_eq!(observed,total);
    let mut words=vec![u64::from(target),total,u64::from(index)];words.extend(trace.map(u64::from));
    ORIGINAL_EVENTS.lock().unwrap().push((2,0x544b0002,words));
}
fn compare_call_events() {
    assert_eq!(*PORTED_EVENTS.lock().unwrap(),*ORIGINAL_EVENTS.lock().unwrap(),"original ICALL diagnostic fields/order");
}
fn write_call_events(path:&std::path::Path) {
    use std::io::Write;
    let events=ORIGINAL_EVENTS.lock().unwrap();let mut file=std::fs::File::create(path).unwrap();
    file.write_all(&(events.len() as u32).to_le_bytes()).unwrap();
    for (level,code,values) in &*events {
        for field in [*level,*code,values.len() as u32] {file.write_all(&field.to_le_bytes()).unwrap();}
        for value in values {file.write_all(&value.to_le_bytes()).unwrap();}
    }
}
