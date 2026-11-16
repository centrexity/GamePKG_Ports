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
    let vectors: Vec<Vec<u8>> = [-1, 0, 1]
        .into_iter()
        .map(|mode| fs::read(Path::new(&args[3]).join(format!("vectors-{mode}"))).unwrap())
        .collect();
    assert!(vectors.iter().all(|v| v.len() == 4096 * 136));
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
            "wait_service.rs",
            here.join("../service-kernel-wait/wait.rs"),
        ),
        ("pacing.rs", here.join("pacing.rs")),
        ("shared.rs", here.join("shared.rs")),
        ("cases.rs", here.join("cases.rs")),
        ("probe.rs", here.join("probe.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod call_state;mod registers;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod wait_service;mod pacing;mod shared;mod cases;mod probe;
pub fn pacing_worker(arg:u64)->u64{probe::pacing_worker(arg)}
fn run()->Result<(),i32>{
    for mode in [-1i32,0,1]{
        let mut state=pacing::Pacing::pacing_new(mode)?;
        for batch in 0..32u32 {
            let mut bytes=std::vec::Vec::new();
            for index in batch*128..(batch+1)*128{
                let input=cases::pacing_case(index);
                let result=cases::pacing_apply(&mut state,input[0],input[1],input[2])?;
                for value in result{bytes.extend(u64::to_le_bytes(value));}
                bytes.extend(state.pacing_encode());
            }
            gamepkg::diagnostic_bytes(1,0x544b1a00+(mode+1) as u32,&bytes)?;
        }
    }
    probe::pacing_probe()
}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b1aff,&[code]);}
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
    let package = root.path().join("pacing.gamepkg");
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
            vec!["pacing_worker".into()],
            LocalLayout::None,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        fs::create_dir_all(Path::new(&args[3]).join(format!("receipt-{target:?}"))).unwrap();
        fs::write(
            Path::new(&args[3]).join(format!("receipt-{target:?}/services.json")),
            serde_json::to_vec_pretty(artifact.receipt()).unwrap(),
        )
        .unwrap();
        assert_eq!(
            artifact.receipt().game.sdk,
            gamepkg::port_host::build_identity()
        );
        assert_eq!(artifact.receipt().game.source_kit, checked.identity());
        assert_eq!(
            artifact.receipt().raw_objects.len(),
            artifact.receipt().game.objects.len()
        );
        println!("PASS {target:?}: complete display/vblank/shared-worker/helper sources scanned before bindings, every separately optimized game object audited before trusted runtime linking; unsafe unused reference rejected");
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
            if batch.iter().any(|e| e.code == 0x544b1a10) {
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
        assert_eq!(events.last().unwrap().code, 0x544b1aff);
        assert_eq!(
            events.last().unwrap().values,
            [0],
            "{}",
            owner.diagnostics()
        );
        assert!(held);
        assert_eq!(events.len(), 99);
        for (slot, reference) in vectors.iter().enumerate() {
            let mut actual = Vec::<u8>::new();
            let mut batches = 0;
            for event in events.iter().filter(|e| e.code == 0x544b1a00 + slot as u32) {
                actual.extend(event.bytes.as_ref().unwrap());
                batches += 1;
            }
            assert_eq!(batches, 32);
            assert_eq!(&actual, reference);
        }
        let progress = events.iter().find(|e| e.code == 0x544b1a11).unwrap();
        assert_eq!(progress.values.len(), 4);
        assert!(progress.values[1] > progress.values[0]);
        assert!(progress.values[3] > progress.values[2]);
        assert_eq!(events.last().unwrap().code, 0x544b1aff);
        assert_eq!(events.last().unwrap().values, [0]);
        owner.stop().unwrap();
        println!("PASS {target:?}: 12288 original C state/output witnesses, live assigned pacing worker with concurrent display updates, pause restart and role/lifetime/error controls; progress during >=60ms actual queued-window stall; no hardware cadence or IRQ proof");
    }
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}
