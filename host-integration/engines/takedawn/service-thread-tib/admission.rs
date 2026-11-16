// Maintainer-only signed-fixture admission; never injected into game objects.
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
    let vectors = fs::read(Path::new(&args[3]).join("vectors")).unwrap();
    assert_eq!(vectors.len(), 364);
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
    for (name, relative) in [
        ("memory", "../service-native/memory.rs"),
        ("call_state", "../service-native/call_state.rs"),
        ("heap", "../service-heap/heap.rs"),
        ("contiguous", "../service-contiguous/contiguous.rs"),
        ("heap_service", "../service-heap/service.rs"),
        ("layout", "../service-memory/layout.rs"),
        ("sha256", "../service-image/sha256.rs"),
        ("thread_tib", "thread_tib.rs"),
        ("probe", "probe.rs"),
    ] {
        fs::copy(
            here.join(relative),
            source.join("source/src").join(format!("{name}.rs")),
        )
        .unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod call_state;mod heap;mod contiguous;mod heap_service;mod layout;mod sha256;mod thread_tib;mod probe;
pub fn heap_worker(arg:u64)->u64{heap_service::heap_serve(arg)}
pub fn tib_worker(arg:u64)->u64{probe::tib_worker(arg)}
fn run()->Result<(),i32>{let bytes=probe::tib_probe()?;gamepkg::diagnostic_bytes(1,0x544b2e00,&bytes)?;Ok(())}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(c)=>c as i64 as u64};let _=gamepkg::diagnostic(1,0x544b2eff,&[code]);}
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
    let package = root.path().join("dpc.gamepkg");
    gamepkg::pack::pack(&source, &package, &[188; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    fs::copy(
        here.join("reference.rs"),
        source.join("source/src/unused.rs"),
    )
    .unwrap();
    assert!(gamepkg::pack::pack(&source, &root.path().join("denied.gamepkg"), &[188; 32]).is_err());
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
            vec!["heap_worker".into(), "tib_worker".into()],
            LocalLayout::TakedawnMemory,
            true,
        )
        .unwrap()
        .with_codegen_units(8)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(artifact.receipt().raw_objects.len(), 8);
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
        println!("PASS {target:?}: complete dependency-free thread-TIB/heap/worker/helper source scanned before SDK injection; each optimized game object audited before trusted linking; unused C-oracle harness rejected");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            384 * 1024 * 1024,
            || {},
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(120);
        let mut events = Vec::new();
        loop {
            owner.dispatch(16);
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
        assert_eq!(events.len(), 2);
        assert_eq!(events.last().unwrap().code, 0x544b2eff);
        assert_eq!(
            events.last().unwrap().values,
            [0],
            "{}",
            owner.diagnostics()
        );
        let mut actual = Vec::<u8>::new();
        for event in events.iter().filter(|e| e.code == 0x544b2e00) {
            actual.extend(event.bytes.as_ref().unwrap());
        }
        assert_eq!(actual, vectors);
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined child, original per-thread TIB witnesses, actual allocation worker and separate heap actor; not title gameplay; no window, GPU, title handler or gameplay");
    }
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}
