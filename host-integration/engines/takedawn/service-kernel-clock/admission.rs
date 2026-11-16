// Maintainer-only qualification. No oracle or sample injection in admitted source.
use gamepkg::port_host::{
    admission::CheckedSources,
    owner::{Grants, Owner},
    preparation::{self, LocalLayout, Plan, Target},
    Backend, Error, Frame, Result, WindowCommand,
};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};
struct Silent;
impl Backend for Silent {
    fn window(&mut self, _: WindowCommand) -> Result<()> {
        Err(Error::Unsupported)
    }
    fn present(&mut self, _: Frame) -> Result<()> {
        Err(Error::Unsupported)
    }
    fn audio(&mut self, _: &[i16], _: u32) -> Result<()> {
        Err(Error::Unsupported)
    }
    fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
        Err(Error::Unsupported)
    }
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert_eq!(args.len(), 4);
    let app = Path::new(&args[1]);
    let here = Path::new(&args[2]);
    let vectors = fs::read(&args[3]).unwrap();
    assert_eq!(vectors.len(), 256 * 8);
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("package");
    fs::create_dir_all(source.join("source/src")).unwrap();
    for name in [
        "manifest.toml",
        "thumbnail.png",
        "LICENSE",
        "NOTICE",
        "CREDITS.md",
        "source/Cargo.toml",
        "source/Cargo.lock",
    ] {
        fs::copy(app.join("examples/minimal").join(name), source.join(name)).unwrap();
    }
    for (name, path) in [
        ("memory.rs", here.join("../service-native/memory.rs")),
        ("registers.rs", here.join("../service-native/registers.rs")),
        (
            "call_state.rs",
            here.join("../service-native/call_state.rs"),
        ),
        ("heap.rs", here.join("../service-heap/heap.rs")),
        (
            "contiguous.rs",
            here.join("../service-contiguous/contiguous.rs"),
        ),
        ("heap_service.rs", here.join("../service-heap/service.rs")),
        (
            "kernel_pool.rs",
            here.join("../service-kernel-pool/kernel_pool.rs"),
        ),
        ("cases.rs", here.join("cases.rs")),
        ("probe.rs", here.join("probe.rs")),
        ("controls.rs", here.join("controls.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod registers;mod call_state;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod cases;mod probe;mod controls;
pub fn clock_worker(arg:u64)->u64{controls::clock_worker(arg)}
fn run()->Result<(),i32>{
    let mut bytes=std::vec::Vec::new();
    for mode in 0..256u32{
        let v=cases::clock_case(mode);let ordinal=v[0] as u32;
        let base=if ordinal==125{v[3]}else if ordinal==126{v[1]}else if ordinal==127{v[2]}else{v[4]};
        let result=if ordinal==128 && v[6]==0{0}else{kernel_pool::kernel_time_value(ordinal,base,v[2],v[5])?};
        bytes.extend(u64::to_le_bytes(result));
    }
    gamepkg::diagnostic_bytes(1,0x544b1400,&bytes)?;
    controls::clock_live_controls(true)
}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b14ff,&[code]);}
"#).unwrap();
    let mut manifest =
        gamepkg::manifest::Manifest::parse(&fs::read(source.join("manifest.toml")).unwrap())
            .unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    fs::write(
        source.join("manifest.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let mut license = fs::read(source.join("LICENSE")).unwrap();
    license.extend(fs::read(here.join("../service-native/UPSTREAM_LICENSES.txt")).unwrap());
    fs::write(source.join("LICENSE"), license).unwrap();
    let package = root.path().join("clock.gamepkg");
    gamepkg::pack::pack(&source, &package, &[183; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec!["clock_worker".into()],
            LocalLayout::TakedawnRam,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        println!("PASS {target:?}: complete clock/helper source admission and every-object audit before trusted linking; not complete title");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            || {},
        )
        .unwrap();
        // Initial corpus diagnostic waits for dispatch: establish the host
        // calendar before the fixture brackets any actual clock calls.
        let before = owner.host.timing().sample().unwrap();
        let end = Instant::now() + Duration::from_secs(120);
        let mut events = Vec::new();
        loop {
            owner.dispatch(1);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(Instant::now() < end, "{}", owner.diagnostics());
            std::thread::sleep(Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        let after = owner.host.timing().sample().unwrap();
        assert_eq!(before.frequency, after.frequency);
        assert_eq!(events.len(), 94, "{events:?}");
        assert_eq!(events[0].code, 0x544b1400);
        assert_eq!(events[0].bytes.as_deref().unwrap(), vectors);
        assert_eq!(events[92].code, 0x544b1401);
        assert_eq!(events[92].values, vec![80]);
        assert_eq!(events[93].code, 0x544b14ff);
        assert_eq!(events[93].values, vec![0]);
        let mut origin = None;
        let mut tsc_views = 0u32;
        for event in events
            .iter()
            .filter(|e| (0x544b1421..=0x544b1424).contains(&e.code))
        {
            assert_eq!(event.values.len(), 5);
            let v = &event.values;
            assert!((before.counter..=after.counter).contains(&v[1]));
            assert_eq!(v[2], before.frequency);
            assert!(v[3] >= before.counter && v[4] >= v[3] && v[4] <= after.counter);
            if let Some(first) = origin {
                assert_eq!(v[1], first);
            } else {
                origin = Some(v[1]);
            }
            let low = 12345678901u128 * 733333333 / 1000000000;
            let high = u128::from(after.counter - v[1]) * 733333333 / u128::from(v[2]) + low;
            assert!((low..=high).contains(&u128::from(v[0])));
            tsc_views |= 1 << (event.code - 0x544b1421);
        }
        assert_eq!(tsc_views, 15);
        let mut counts = [[0u32; 4]; 5];
        let stages: Vec<_> = events
            .iter()
            .filter(|e| e.code == 0x544b1402)
            .map(|e| e.values.clone())
            .collect();
        assert_eq!(stages, (0u64..=6).map(|s| vec![s]).collect::<Vec<_>>());
        for event in events
            .iter()
            .filter(|e| (0x544b1410..=0x544b1414).contains(&e.code))
        {
            assert!((0x544b1410..=0x544b1414).contains(&event.code));
            assert_eq!(event.values.len(), 2);
            let ordinal = event.values[0];
            let value = event.values[1];
            assert!((125..=128).contains(&ordinal));
            counts[(event.code - 0x544b1410) as usize][(ordinal - 125) as usize] += 1;
            let bound = |sample: gamepkg::port_host::timing::Sample| -> u64 {
                let skew = 12345678901u128;
                let n = match ordinal {
                    125 => u128::from(sample.uptime_ms) * 10000 + skew / 100,
                    126 => {
                        u128::from(sample.counter)
                            + skew * u128::from(sample.frequency) / 1000000000
                    }
                    127 => u128::from(sample.frequency),
                    128 => u128::from(sample.utc_100ns) + skew / 100,
                    _ => unreachable!(),
                };
                u64::try_from(n).unwrap()
            };
            assert!(
                (bound(before)..=bound(after)).contains(&value),
                "{ordinal} value {value}"
            );
        }
        assert_eq!(counts, [[4; 4]; 5]);
        owner.stop().unwrap();
        println!("PASS {target:?}: 256 original C arithmetic witnesses, 80 actual kernel calls on five views plus four shared-origin TSC reads, full CPU checks, independent parent clock bounds and denial/overflow/null controls");
    }
}
