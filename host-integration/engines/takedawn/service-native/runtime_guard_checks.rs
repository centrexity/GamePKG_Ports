// Maintainer-only route checks using the real checked Memory and local runtime.
fn compare_runtime_guards() {
    let mut ram=memory::Memory::new().unwrap();
    let mut state=registers::Registers::new();state.eax=0x12345678;state.esp=0x87654321;
    for target in [0x001537c0,0x001d2807,0xfe000000,0xfe7f5700,0x00772c66] {
        assert_ne!(targets::original_target(target),0);
        let before=state.encode();
        assert_eq!(calls_runtime::invoke_call(&mut state,&mut ram,target,0x4444),Err(-5));
        assert_eq!(state.encode(),before,"unmigrated known target must not become successful fallback");
        let trace=ram.call_snapshot().unwrap();
        assert_eq!(calls_runtime::invoke_jump(&mut state,&mut ram,target),Err(-5));
        assert_eq!(state.encode(),before,"untraced known target must not become successful absence");
        assert_eq!(ram.call_snapshot().unwrap(),trace,"untraced rejection must not record a call");
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
    let events=PORTED_EVENTS.lock().unwrap().len();
    crate::jump_checks::jump_verify().unwrap();
    let events=PORTED_EVENTS.lock().unwrap()[events..].to_vec();
    let mut values=vec![0x8000,0,0];values.resize(19,0);
    assert_eq!(events,vec![(2,0x544b0002,values)]);
    println!("PASS: known migration faults, distinct traced/untraced absence contracts, exact untraced diagnostics and revoked-ledger atomicity; no kernel/manual handler implementations");
}
