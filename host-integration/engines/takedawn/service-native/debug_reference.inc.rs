// Maintainer comparison only: the port gets no pointers, C or stdio bindings.
static ORIGINAL_BYTE_EVENTS: Mutex<Vec<(u32,u32,Vec<u8>)>>=Mutex::new(Vec::new());
#[no_mangle]
pub unsafe extern "C" fn gamepkg_reference_debug_bytes(level:u32,code:u32,bytes:*const u8,length:usize) {
    assert!(level<=3 && length<=128*1024);
    ORIGINAL_BYTE_EVENTS.lock().unwrap().push((level,code,std::slice::from_raw_parts(bytes,length).to_vec()));
}
extern "C" {fn gamepkg_debug_service_ref(service:u32,argument:u32,base:usize);}
fn compare_debug_service(ram:&mut memory::Memory, window:u64, output:Option<std::path::PathBuf>) {
    debug_checks::verify(ram,window).unwrap();
    let mut original=vec![0u8;8*1024*1024];
    for (index,chunk) in original.chunks_mut(1024*1024).enumerate() {
        ram.read_bytes((index*1024*1024) as u32,chunk).unwrap();
    }
    for [service,argument] in debug_checks::requests() {
        unsafe {gamepkg_debug_service_ref(service,argument,original.as_ptr() as usize);}
    }
    assert_eq!(*PORTED_BYTE_EVENTS.lock().unwrap(),*ORIGINAL_BYTE_EVENTS.lock().unwrap(),"exact original counted ANSI bytes/newline/unknown-service behavior");
    assert_eq!(original[0x13000..0x13005],*b"A\0BCD");
    if let Some(path)=output {
        use std::io::Write;
        let mut file=std::fs::File::create(path).unwrap();
        let events=ORIGINAL_BYTE_EVENTS.lock().unwrap();
        file.write_all(b"GPB1").unwrap();file.write_all(&(events.len() as u32).to_le_bytes()).unwrap();
        for (level,code,bytes) in events.iter() {
            for word in [*level,*code,bytes.len() as u32] {file.write_all(&word.to_le_bytes()).unwrap();}
            file.write_all(bytes).unwrap();
        }
    }
    println!("PASS: exact pinned C debug-service comparison / 7 byte events, quiet and denied-read cases; no console output");
}
