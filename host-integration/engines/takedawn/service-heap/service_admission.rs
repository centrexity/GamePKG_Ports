// Maintainer-only admission and actual confined execution of checked game tasks.
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
        ("heap.rs", here.join("heap.rs")),
        ("service.rs", here.join("service.rs")),
        ("service_checks.rs", here.join("service_checks.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod call_state;mod heap;mod service;mod service_checks;
pub fn heap_worker(arg:u64)->u64{service::heap_serve(arg)}
pub fn client_worker(arg:u64)->u64{service_checks::heap_test_client(arg)}
pub fn fake_worker(arg:u64)->u64{service_checks::heap_test_fake(arg)}
pub fn waiting_peer(arg:u64)->u64{service_checks::heap_test_waiting_peer(arg)}
pub fn cancelled_caller(arg:u64)->u64{service_checks::heap_test_cancelled_caller(arg)}
pub fn abandoned_client(arg:u64)->u64{service_checks::heap_test_abandoned_client(arg)}
pub fn game_main(){let status=match service_checks::heap_service_verify(){Ok(2000)=>0u64,Ok(_)=>(-3i64) as u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b0500,&[status]);}
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
    license.extend(b"\n\nTakedawn / XboxRecomp upstream notices:\n");
    license.extend(fs::read(here.join("../service-native/UPSTREAM_LICENSES.txt")).unwrap());
    fs::write(source.join("LICENSE"), license).unwrap();
    let package = root.path().join("heap-service.gamepkg");
    gamepkg::pack::pack(&source, &package, &[181; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    fs::write(
        source.join("source/src/AAA_unreviewed.rs"),
        "pub unsafe extern \"C\" fn escape(){std::fs::read(\"outside\");}",
    )
    .unwrap();
    assert!(
        gamepkg::pack::pack(&source, &root.path().join("denied.gamepkg"), &[181; 32])
            .unwrap_err()
            .to_string()
            .contains("forbidden")
    );
    fs::remove_file(source.join("source/src/AAA_unreviewed.rs")).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec![
                "heap_worker".into(),
                "client_worker".into(),
                "fake_worker".into(),
                "waiting_peer".into(),
                "cancelled_caller".into(),
                "abandoned_client".into(),
            ],
            LocalLayout::TakedawnRam,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert!(!artifact.receipt().game.objects.is_empty());
        assert_eq!(
            artifact.receipt().game.objects.len(),
            artifact.receipt().raw_objects.len()
        );
        println!("PASS {target:?}: complete dependency-free heap task/client source checked before bindings; every optimized game object audited before trusted linking");
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
        assert_eq!(events.len(), 1, "{}", owner.diagnostics());
        assert_eq!(events[0].code, 0x544b0500);
        assert_eq!(events[0].values, vec![0]);
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined shared heap task, 2000 concurrent allocation lifetimes and protocol/lifecycle checks; silent/no game entry");
    }
}
