// Maintainer-only complete source admission, object audits and real SDK witness.
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
    let oracle = Path::new(&args[3]);
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
    for (name, path) in [
        ("memory.rs", here.join("../service-native/memory.rs")),
        ("registers.rs", here.join("../service-native/registers.rs")),
        (
            "call_state.rs",
            here.join("../service-native/call_state.rs"),
        ),
        ("heap_service.rs", here.join("../service-heap/service.rs")),
        ("heap.rs", here.join("../service-heap/heap.rs")),
        (
            "contiguous.rs",
            here.join("../service-contiguous/contiguous.rs"),
        ),
        (
            "kernel_pool.rs",
            here.join("../service-kernel-pool/kernel_pool.rs"),
        ),
        ("leaf_probe.rs", here.join("leaf_probe.rs")),
        ("generated.rs", here.join("generated.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::copy(
        here.join("leaf_dispatch.rs"),
        source.join("source/src/dispatch.rs"),
    )
    .unwrap();
    fs::copy(
        here.join("../service-native/calls_runtime.rs"),
        source.join("source/src/calls_runtime.rs"),
    )
    .unwrap();
    let target_proof: serde_json::Value =
        serde_json::from_slice(&fs::read(targets.join("call_targets.json")).unwrap()).unwrap();
    assert_eq!(target_proof["production_ready"], false);
    for (name, hash) in target_proof["emitted_files"].as_object().unwrap() {
        assert!(gamepkg::archive::valid_path(name) && name.ends_with(".rs"));
        let data = fs::read(targets.join(name)).unwrap();
        assert_eq!(gamepkg::verify::digest(&data), hash.as_str().unwrap());
        let path = source.join("source/src").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, data).unwrap();
    }
    let proof: serde_json::Value =
        serde_json::from_slice(&fs::read(here.join("generated.json")).unwrap()).unwrap();
    assert_eq!(proof["production_ready"], false);
    assert_eq!(
        proof["kernel_services_source_sha256"],
        gamepkg::verify::digest(&fs::read(source.join("source/src/kernel_pool.rs")).unwrap())
    );
    assert_eq!(
        proof["game_sha256"],
        gamepkg::verify::digest(&fs::read(source.join("source/src/generated.rs")).unwrap())
    );
    fs::create_dir(source.join("assets")).unwrap();
    fs::copy(
        here.join("generated.json"),
        source.join("assets/tsc-generated.json"),
    )
    .unwrap();
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod registers;mod call_state;mod heap_service;mod heap;mod contiguous;mod kernel_pool;mod leaf_probe;mod generated;mod calls_runtime;mod targets;mod dispatch;
fn run()->Result<(),i32>{gamepkg::diagnostic(1,0x544b15fe,&[])?;leaf_probe::leaf_live()}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b15ff,&[code]);}
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
    let package = root.path().join("tsc-leaves.gamepkg");
    gamepkg::pack::pack(&source, &package, &[183; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    fs::copy(oracle, source.join("source/src/unused_reference.rs")).unwrap();
    let poisoned = root.path().join("unsafe-oracle.gamepkg");
    let denied = gamepkg::pack::pack(&source, &poisoned, &[183; 32]).unwrap_err();
    assert!(denied.to_string().contains("forbidden"), "{denied:#}");
    fs::remove_file(source.join("source/src/unused_reference.rs")).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::TakedawnRam, true)
            .unwrap()
            .with_codegen_units(2)
            .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        println!("PASS {target:?}: all five exact TSC leaf bodies and production helpers scanned before bindings, optimized separately and every game object audited before trusted linking; unsafe original remains rejected");
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
        let before = owner.host.timing().performance().unwrap();
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
        let after = owner.host.timing().performance().unwrap();
        assert_eq!(before.frequency, after.frequency);
        assert_eq!(events.len(), 82, "{events:?}");
        assert_eq!(events[0].code, 0x544b15fe);
        assert!(events[0].values.is_empty());
        assert_eq!(events[81].code, 0x544b15ff);
        assert_eq!(events[81].values, vec![0]);
        let mut counts = [0u32; 5];
        let mut origin = None;
        for event in &events[1..81] {
            assert_eq!(event.code, 0x544b1500);
            assert_eq!(event.values.len(), 4);
            let v = &event.values;
            assert!(v[0] < 5);
            counts[v[0] as usize] += 1;
            assert!((before.counter..=after.counter).contains(&v[2]));
            assert_eq!(v[3], before.frequency);
            if let Some(value) = origin {
                assert_eq!(v[2], value);
            } else {
                origin = Some(v[2]);
            }
            let low = 12345678901u128 * 733333333 / 1000000000;
            let high = u128::from(after.counter - v[2]) * 733333333 / u128::from(v[3]) + low;
            assert!((low..=high).contains(&u128::from(v[1])));
        }
        assert_eq!(counts, [16; 5]);
        owner.stop().unwrap();
        println!("PASS {target:?}: 80 actual finite-router generated TSC leaf calls, full CPU/all mapped-RAM witnesses, independent parent timestamp bounds, partial-store fidelity and missing-state denial");
    }
}
