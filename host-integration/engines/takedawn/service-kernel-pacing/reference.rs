// Maintainer-only original C comparison, never game source.
unsafe extern "C" {
    fn pacing_reference_init(mode:i32);
    fn pacing_reference(operation:u32,first:u64,second:u64,output:*mut u8);
}
fn main(){
    let args:Vec<_>=std::env::args_os().collect();
    let mode=args[1].to_str().unwrap().parse::<i32>().unwrap();
    unsafe{pacing_reference_init(mode);}
    let mut state=pacing::Pacing::pacing_new(mode).unwrap();
    let mut vectors=Vec::new();let mut diagnostics=0u64;
    for index in 0..4096u32 {
        let [op,first,second]=cases::pacing_case(index);
        let mut reference=[0u8;136];
        unsafe{pacing_reference(op as u32,first,second,reference.as_mut_ptr());}
        let result=cases::pacing_apply(&mut state,op,first,second).unwrap();
        if op==1{diagnostics|=result[0];}
        let mut actual=Vec::new();
        for value in result{actual.extend(value.to_le_bytes());}
        actual.extend(state.pacing_encode());
        assert_eq!(actual,reference,"mode {mode} vector {index}: {op}/{first}/{second}");
        assert_eq!(pacing::Pacing::pacing_decode(&actual[32..]).unwrap().pacing_encode(),actual[32..]);
        vectors.extend(actual);
    }
    assert_eq!(diagnostics,if mode==0{0}else{7});
    let initial=pacing::Pacing::pacing_new(mode).unwrap().pacing_encode();
    for length in 0..104{assert!(pacing::Pacing::pacing_decode(&initial[..length]).is_err());}
    assert!(pacing::Pacing::pacing_decode(&[initial.clone(),vec![0]].concat()).is_err());
    for offset in [0usize,4,8,12,16,20,24,28,32,36,44,56,64]{
        let mut invalid=initial.clone();
        let value=if offset==56 || offset==64{1u32}else{u32::MAX};
        invalid[offset..offset+4].copy_from_slice(&value.to_le_bytes());
        if offset==4{invalid[offset..offset+4].copy_from_slice(&2u32.to_le_bytes());}
        assert!(pacing::Pacing::pacing_decode(&invalid).is_err(),"invalid offset {offset}");
    }
    let mut invalid=pacing::Pacing::pacing_new(1).unwrap().pacing_encode();
    invalid[48..56].copy_from_slice(&u64::MAX.to_le_bytes());
    invalid[56..64].copy_from_slice(&16666666u64.to_le_bytes());
    let mut overflow=pacing::Pacing::pacing_decode(&invalid).unwrap();
    assert_eq!(overflow.pacing_step(u64::MAX),Err(-1));
    assert_eq!(overflow.pacing_encode(),invalid);
    if let Some(path)=args.get(2){std::fs::write(path,&vectors).unwrap();}
    println!("PASS mode {mode}: 4096 original C state/output vectors; display learning, auto/forced lock, irregular-report recovery, rate changes, stale reports, restart/next-edge schedule and checked wire/error atomicity");
}
