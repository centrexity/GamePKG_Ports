// Maintainer-only qualification, separate from every admitted game source.
use gamepkg::port_host::{
    admission::CheckedSources,
    owner::{Grants, Owner},
    preparation::{self, LocalLayout, Plan, Target},
    Backend, Error, Frame, Result, WindowCommand,
};
use std::{
    fs,
    path::Path,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
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
    assert_eq!(vectors.len(), 4096 * 16);
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
        (
            "call_state.rs",
            here.join("../service-native/call_state.rs"),
        ),
        ("registers.rs", here.join("../service-native/registers.rs")),
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
        ("wait_service.rs", here.join("wait.rs")),
        ("cases.rs", here.join("cases.rs")),
        ("probe.rs", here.join("probe.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod call_state;mod registers;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod wait_service;mod cases;mod probe;
pub fn wait_worker(arg:u64)->u64{probe::wait_worker(arg)}
fn run()->Result<(),i32>{
    let mut bytes=std::vec::Vec::new();
    for index in 0..4096u32 {
        let values=cases::wait_case(index);
        let host=wait_service::host_time_value(values[0],values[1])?;
        bytes.extend(u64::to_le_bytes(host));
        bytes.extend(u64::to_le_bytes(wait_service::guest_time_value(host,values[2])));
    }
    gamepkg::diagnostic_bytes(1,0x544b1800,&bytes)?;
    probe::wait_probe()?;
    probe::wait_blocked_probe()
}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b18ff,&[code]);}
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
    let package = root.path().join("wait.gamepkg");
    gamepkg::pack::pack(&source, &package, &[187; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    fs::write(
        source.join("source/src/unused.rs"),
        fs::read(here.join("reference.rs")).unwrap(),
    )
    .unwrap();
    assert!(gamepkg::pack::pack(&source, &root.path().join("denied.gamepkg"), &[187; 32]).is_err());
    fs::remove_file(source.join("source/src/unused.rs")).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec!["wait_worker".into()],
            LocalLayout::None,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(
            artifact.receipt().game.sdk,
            gamepkg::port_host::build_identity()
        );
        assert_eq!(artifact.receipt().game.source_kit, checked.identity());
        assert_eq!(
            artifact.receipt().raw_objects.len(),
            artifact.receipt().game.objects.len()
        );
        println!("PASS {target:?}: complete host-time/sleeper/helper sources scanned before bindings, every separately optimized game object audited before trusted runtime linking; unsafe unused reference rejected");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let wakes = Arc::new(AtomicUsize::new(0));
        let notify = wakes.clone();
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            256 * 1024 * 1024,
            move || {
                notify.fetch_add(1, Ordering::Release);
            },
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(120);
        let mut events = Vec::new();
        let mut dispatched = 0;
        let mut withholding = false;
        let mut held_at = None;
        let mut held = false;
        loop {
            let dispatch = if withholding {
                if held_at.is_none() && wakes.load(Ordering::Acquire) > dispatched {
                    held_at = Some(Instant::now());
                }
                if held_at.is_some_and(|start| start.elapsed() >= Duration::from_millis(60)) {
                    withholding = false;
                    held = true;
                    true
                } else {
                    false
                }
            } else {
                true
            };
            if dispatch {
                dispatched += owner.dispatch(1);
            }
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            if batch.iter().any(|e| e.code == 0x544b1801) {
                assert!(!withholding && !held);
                withholding = true;
            }
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
        assert!(held);
        assert_eq!(
            events
                .iter()
                .filter(|e| e.level == 0)
                .map(|e| e.code)
                .collect::<Vec<_>>(),
            (0x544b1810..=0x544b1815).collect::<Vec<_>>()
        );
        events.retain(|e| e.level != 0);
        assert_eq!(events.len(), 5);
        assert_eq!(events[0].code, 0x544b1800);
        assert_eq!(events[0].bytes.as_ref().unwrap(), &vectors);
        assert_eq!(events[1].code, 0x544b1802);
        assert_eq!(events[1].values, [64, 64]);
        assert_eq!(events[2].code, 0x544b1801);
        assert_eq!(events[3].code, 0x544b1803);
        assert_eq!(events[3].values.len(), 2);
        assert!(events[3].values[1] > events[3].values[0]);
        assert_eq!(events[4].code, 0x544b18ff);
        assert_eq!(events[4].values, [0]);
        owner.stop().unwrap();
        println!("PASS {target:?}: 4096 original C clock witnesses, 64 coalesced notifications and 64 reviewed-task handoffs, malformed/foreign/revoked/cancelled owner controls; local deadlines and pending-wake consumption continue during a >=60ms real queued-window stall; no physical pacing claim");
    }
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}
