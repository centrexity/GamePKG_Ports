// Maintainer-only route checks using the real checked Memory and local runtime.
fn compare_runtime_guards() {
    let mut ram=memory::Memory::new().unwrap();
    let mut state=registers::Registers::new();state.eax=0x12345678;state.esp=0x87654321;
    for target in [0x001537c0,0x001d2807,0xfe000000,0xfe7f5700,0x00772c66] {
        assert_ne!(targets::original_target(target),0);
        let before=state.encode();
        assert_eq!(calls_runtime::invoke_call(&mut state,&mut ram,target,0x4444),Err(-5));
        assert_eq!(state.encode(),before,"unmigrated known target must not become successful fallback");
    }
    ram.call_bounds(0x10000,0x20000).unwrap();
    calls_runtime::invoke_call(&mut state,&mut ram,0,0x4444).unwrap();
    assert_eq!((state.eax,state.esp),(0,0x4444));
    state.eax=123;state.esp=456;
    ram.call_bounds(0,0).unwrap();
    assert_eq!(targets::original_target(0x8000),0);
    calls_runtime::invoke_call(&mut state,&mut ram,0x8000,0x5555).unwrap();
    assert_eq!((state.eax,state.esp),(0,0x5555));
    let before=state.encode();
    client::local_record_release(ram.call_handle()).unwrap();
    assert_eq!(calls_runtime::invoke_call(&mut state,&mut ram,0x8000,0),Err(-4));
    assert_eq!(state.encode(),before,"revoked state must not execute fallback");
    println!("PASS: known generated/manual/entry/kernel/soundtrack migration faults, original absence/non-code fallbacks, and revoked-ledger atomicity; no handler implementations");
}
