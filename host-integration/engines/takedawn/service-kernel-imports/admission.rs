// Focused maintainer admission; complete generated-kit qualification is separate.
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
    assert_eq!(args.len(), 5);
    let app = Path::new(&args[1]);
    let here = Path::new(&args[2]);
    let vectors = Path::new(&args[3]);
    let targets = Path::new(&args[4]);
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
    for name in [
        "memory",
        "registers",
        "call_state",
        "calls_runtime",
        "vector",
        "scalar",
        "debug",
    ] {
        fs::copy(
            here.join("../service-native").join(format!("{name}.rs")),
            source.join("source/src").join(format!("{name}.rs")),
        )
        .unwrap();
    }
    for (name, path) in [
        ("heap.rs", here.join("../service-heap/heap.rs")),
        ("layout.rs", here.join("../service-memory/layout.rs")),
        (
            "contiguous.rs",
            here.join("../service-contiguous/contiguous.rs"),
        ),
        ("heap_service.rs", here.join("../service-heap/service.rs")),
        (
            "kernel_pool.rs",
            here.join("../service-kernel-pool/kernel_pool.rs"),
        ),
        ("imports.rs", here.join("imports.rs")),
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
        ("game.rs", here.join("../service-native/generated/game.rs")),
        (
            "dispatch.rs",
            here.join("../service-native/generated/dispatch.rs"),
        ),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    let report: serde_json::Value =
        serde_json::from_slice(&fs::read(targets.join("call_targets.json")).unwrap()).unwrap();
    assert_eq!(report["production_ready"], false);
    for (name, hash) in report["emitted_files"].as_object().unwrap() {
        assert!(gamepkg::archive::valid_path(name) && name.ends_with(".rs"));
        let data = fs::read(targets.join(name)).unwrap();
        assert_eq!(gamepkg::verify::digest(&data), hash.as_str().unwrap());
        let path = source.join("source/src").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod layout;mod memory;mod registers;mod call_state;mod calls_runtime;mod vector;mod scalar;mod debug;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod kernel_data;mod signature;mod imports;mod cases;mod probe;mod controls;mod game;mod dispatch;mod targets;
pub fn heap_worker(arg:u64)->u64{heap_service::heap_serve(arg)}
fn run()->Result<(),i32>{for mode in 0..16u32{let bytes=probe::imports_case(mode)?;gamepkg::diagnostic_bytes(1,0x544b0e00+mode,&bytes)?;}controls::imports_controls()}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b0eff,&[code]);}
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
    let package = root.path().join("imports.gamepkg");
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
            vec!["heap_worker".into()],
            LocalLayout::TakedawnMemory,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        println!("PASS {target:?}: complete import/helper source admission and all-object audit before trusted linking; not full generated kit");
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
            owner.dispatch(32);
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
        assert_eq!(events.last().unwrap().code, 0x544b0eff);
        assert_eq!(events.last().unwrap().values, vec![0], "{events:?}");
        assert_eq!(events.len(), 17);
        for mode in 0..16usize {
            assert_eq!(events[mode].code, 0x544b0e00 + mode as u32);
            assert_eq!(
                events[mode].bytes.as_deref(),
                Some(
                    fs::read(vectors.join(format!("{mode}.bin")))
                        .unwrap()
                        .as_slice()
                ),
                "mode {mode}"
            );
        }
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined complete import witnesses, traced/untraced routing, per-page protection restoration and adversarial controls");
    }
}
