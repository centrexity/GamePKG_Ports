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
    assert_eq!(args.len(), 3);
    let app = Path::new(&args[1]);
    let here = Path::new(&args[2]);
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
        (
            "kernel_data.rs",
            here.join("../service-kernel-data/kernel_data.rs"),
        ),
        (
            "signature.rs",
            here.join("../service-kernel-data/signature.rs"),
        ),
        ("tick_service.rs", here.join("tick.rs")),
        ("probe.rs", here.join("probe.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod call_state;mod registers;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod kernel_data;mod signature;mod tick_service;mod probe;
pub fn tick_worker(arg:u64)->u64{tick_service::tick_worker(arg)}
pub fn game_main(){let code=match probe::tick_probe(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b16ff,&[code]);}
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
    let package = root.path().join("tick.gamepkg");
    gamepkg::pack::pack(&source, &package, &[181; 32]).unwrap();
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
            vec!["tick_worker".into()],
            LocalLayout::TakedawnRam,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        println!("PASS {target:?}: complete periodic-tick, data and memory-helper source scanned before bindings; every optimized game object audited before trusted linking, no engine/worker exemption");
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
                notify.fetch_add(1, Ordering::AcqRel);
            },
        )
        .unwrap();
        let before = owner.host.timing().performance().unwrap();
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
            if batch.iter().any(|e| e.code == 0x544b1601) {
                assert!(!held && !withholding);
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
        let after = owner.host.timing().performance().unwrap();
        assert!(held && !withholding);
        assert_eq!(events.len(), 11, "{events:?}");
        let stages: Vec<_> = events
            .iter()
            .filter(|e| e.code == 0x544b1610)
            .map(|e| e.values.clone())
            .collect();
        assert_eq!(stages, (0..8u64).map(|v| vec![v]).collect::<Vec<_>>());
        assert_eq!(events[10].code, 0x544b16ff);
        assert_eq!(events[10].values, vec![0]);
        let report = events.iter().find(|e| e.code == 0x544b1602).unwrap();
        assert_eq!(report.values.len(), 2);
        assert_eq!(report.values[1], 132);
        let low = (before.uptime_ms as u32).wrapping_add(7999);
        let stopped = report.values[0] as u32;
        assert!(u64::from(stopped.wrapping_sub(low)) <= after.uptime_ms - before.uptime_ms);
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined tick worker advances while a queued fullscreen request holds the broker client mutex for >=60ms; live init/skew, full 64KiB no-scribble, foreign/read-only/revocation, record/event/task quota exhaustion, malformed metadata and 132 success/partial-start lifecycle cycles; no visible graphics/audio/pacing claim");
    }
}
