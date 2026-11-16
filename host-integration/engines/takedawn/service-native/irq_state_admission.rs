// Maintainer-only isolated state component admission; no engine/profile exception.
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
    for name in ["registers.rs", "irq_state_checks.rs"] {
        fs::copy(here.join(name), source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),"mod registers;mod irq_state_checks;pub fn game_main(){let code=match irq_state_checks::verify(){Ok(())=>0u64,Err(c)=>c as i64 as u64};let _=gamepkg::diagnostic(1,0x544b1cff,&[code]);}").unwrap();
    let mut manifest =
        gamepkg::manifest::Manifest::parse(&fs::read(source.join("manifest.toml")).unwrap())
            .unwrap();
    manifest.limits.hard_memory_bytes = 128 * 1024 * 1024;
    fs::write(
        source.join("manifest.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let package = root.path().join("irq-state.gamepkg");
    gamepkg::pack::pack(&source, &package, &[189; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    fs::copy(
        here.join("registers_tests.rs"),
        source.join("source/src/unused.rs"),
    )
    .unwrap();
    assert!(gamepkg::pack::pack(&source, &root.path().join("denied.gamepkg"), &[189; 32]).is_err());
    fs::remove_file(source.join("source/src/unused.rs")).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true)
            .unwrap()
            .with_codegen_units(8)
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
        println!("PASS {target:?}: isolated IRQ-state source/8-unit object admission; no allocation import exemption");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let mut owner = Owner::start(
            &artifact,
            Silent,
            Grants::default(),
            64 * 1024 * 1024,
            || {},
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(30);
        let mut events = Vec::new();
        loop {
            owner.dispatch(8);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            events.extend(batch);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(1));
        }
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        events.extend(batch);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, 0x544b1cff);
        assert_eq!(events[0].values, [0]);
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined 4096 IRQ state preservation/rejection controls");
    }
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}
