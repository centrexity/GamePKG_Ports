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
    assert_eq!(vectors.len(), 768 * (316 + 36));
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
        ("sha256.rs", here.join("../service-image/sha256.rs")),
        ("cases.rs", here.join("cases.rs")),
        ("probe.rs", here.join("probe.rs")),
        ("controls.rs", here.join("controls.rs")),
        (
            "kernel_data.rs",
            here.join("../service-kernel-data/kernel_data.rs"),
        ),
        (
            "signature.rs",
            here.join("../service-kernel-data/signature.rs"),
        ),
        (
            "tick_service.rs",
            here.join("../service-kernel-tick/tick.rs"),
        ),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod call_state;mod registers;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod sha256;mod cases;mod probe;mod controls;mod kernel_data;mod signature;mod tick_service;
pub fn dispatcher_worker(arg:u64)->u64{controls::dispatcher_worker(arg)}
pub fn dispatcher_tick_worker(arg:u64)->u64{tick_service::tick_worker(arg)}
pub fn game_main(){let code=match probe::dispatcher_probe(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b17ff,&[code]);}
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
    let package = root.path().join("dispatcher.gamepkg");
    gamepkg::pack::pack(&source, &package, &[179; 32]).unwrap();
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
            vec!["dispatcher_worker".into(), "dispatcher_tick_worker".into()],
            LocalLayout::TakedawnRam,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        println!("PASS {target:?}: complete initializer/helper sources scanned before bindings, every separately optimized game object audited before trusted runtime linking; no object/IRQ exemption");
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
        assert_eq!(events.len(), 20, "{events:?}");
        assert_eq!(events.last().unwrap().code, 0x544b17ff);
        assert_eq!(events.last().unwrap().values, vec![0]);
        let mut actual: Vec<u8> = Vec::new();
        for e in events.iter().filter(|e| e.code == 0x544b1701) {
            assert_eq!(e.bytes.as_ref().unwrap().len(), 64 * (316 + 36));
            actual.extend(e.bytes.as_ref().unwrap());
        }
        assert_eq!(actual, vectors);
        let initial: Vec<_> = events
            .iter()
            .take(2)
            .map(|e| (e.level, e.code, e.values.clone()))
            .collect();
        assert_eq!(
            initial,
            vec![
                (2, 0x544b1700, vec![107, 0, 32]),
                (2, 0x544b1700, vec![109, 0, 44])
            ]
        );
        let stages: Vec<_> = events
            .iter()
            .filter(|e| e.code == 0x544b1710)
            .map(|e| e.values.clone())
            .collect();
        assert_eq!(stages, vec![vec![0], vec![1], vec![2]]);
        let mut warnings: Vec<_> = events
            .iter()
            .filter(|e| e.code == 0x544b1700)
            .map(|e| e.values.clone())
            .collect();
        warnings.sort();
        assert_eq!(
            warnings,
            vec![
                vec![107, 0, 32],
                vec![107, 0, 32],
                vec![109, 0, 44],
                vec![109, 0, 44]
            ]
        );
        owner.stop().unwrap();
        println!("PASS {target:?}: 768 actual initializer calls match original CPU/RAM-hash/seen-state witnesses; exact once-per-root warnings, two concurrent callers, skew/TSC preservation, read-only/gap/truncated-stack/malformed-version/foreign/revoked-state controls; no IRQ or DPC execution claim");
    }
}
