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
    assert_eq!(args.len(), 4);
    let app = Path::new(&args[1]);
    let here = Path::new(&args[2]);
    let vectors = fs::read(Path::new(&args[3]).join("vectors")).unwrap();
    assert_eq!(vectors.len(), 128 * 452);
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
        ("registers", "../service-native/registers.rs"),
        ("memory", "../service-native/memory.rs"),
        ("call_state", "../service-native/call_state.rs"),
        ("heap", "../service-heap/heap.rs"),
        ("contiguous", "../service-contiguous/contiguous.rs"),
        ("heap_service", "../service-heap/service.rs"),
        ("kernel_pool", "../service-kernel-pool/kernel_pool.rs"),
        ("gate", "../service-kernel-gate/gate.rs"),
        ("irql", "../service-kernel-irql/state.rs"),
        ("stacks", "../service-worker-stacks/stacks.rs"),
        ("wait_service", "../service-kernel-wait/wait.rs"),
        ("production_clock", "clock.rs"),
        ("irq_delivery", "irq.rs"),
        ("irq_hooks", "hooks.rs"),
        ("irq_clock", "fixture_clock.rs"),
        ("irq_routes", "fixture_routes.rs"),
        ("irq_observer", "fixture_observer.rs"),
        ("races", "races.rs"),
        ("controls", "controls.rs"),
        ("options", "options.rs"),
        ("callbacks", "callbacks.rs"),
        ("cases", "cases.rs"),
        ("probe", "probe.rs"),
    ] {
        fs::copy(
            here.join(relative),
            source.join("source/src").join(format!("{name}.rs")),
        )
        .unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod call_state;mod registers;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod gate;mod irql;mod stacks;mod wait_service;mod production_clock;mod irq_delivery;mod irq_hooks;mod irq_clock;mod irq_routes;mod irq_observer;mod races;mod controls;mod options;mod callbacks;mod cases;mod probe;

fn run()->Result<(),i32>{
 let mut windows=[0u8;16];if gamepkg::local_call(1,&[],&mut windows)?!=16{return Err(-3);}
 let window=u64::from_le_bytes([windows[0],windows[1],windows[2],windows[3],windows[4],windows[5],windows[6],windows[7]]);
 gamepkg::local_call(2,&[u64::to_le_bytes(window).as_slice(),&u64::to_le_bytes(0x740000),&u64::to_le_bytes(65536),&u32::to_le_bytes(2)].concat(),&mut [])?;
 gamepkg::local_call(2,&[u64::to_le_bytes(window).as_slice(),&u64::to_le_bytes(0x780000),&u64::to_le_bytes(1048576),&u32::to_le_bytes(2)].concat(),&mut [])?;
 for index in 0..128u32{
 let mut ram=memory::Memory::new()?;ram.attach(0,0x4000000,window,0,true)?;
 let services=kernel_pool::kernel_services_initialize(&ram,0x10000,2)?;
 gamepkg::diagnostic_bytes(1,0x544b2100,&probe::irq_probe_case(&mut ram,window,index)?)?;
 services.service_close(&ram)?;gamepkg::local_record_release(ram.call_handle())?;
 }Ok(())
}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(c)=>c as i64 as u64};let _=gamepkg::diagnostic(1,0x544b21ff,&[code]);}
"#).unwrap();
    let mut license = fs::read(source.join("LICENSE")).unwrap();
    license.extend(fs::read(here.join("../service-native/UPSTREAM_LICENSES.txt")).unwrap());
    fs::write(source.join("LICENSE"), license).unwrap();
    let mut manifest =
        gamepkg::manifest::Manifest::parse(&fs::read(source.join("manifest.toml")).unwrap())
            .unwrap();
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    fs::write(
        source.join("manifest.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let package = root.path().join("irq-state.gamepkg");
    gamepkg::pack::pack(&source, &package, &[189; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    fs::copy(here.join("runtime.rs"), source.join("source/src/unused.rs")).unwrap();
    assert!(gamepkg::pack::pack(&source, &root.path().join("denied.gamepkg"), &[189; 32]).is_err());
    fs::remove_file(source.join("source/src/unused.rs")).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::TakedawnRam, true)
            .unwrap()
            .with_codegen_units(8)
            .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(artifact.receipt().game.objects.len(), 8);
        artifact
            .publish(&Path::new(&args[3]).join(format!("receipt-{target:?}")))
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
        println!("PASS {target:?}: unmodified IRQ composition source/8-unit object admission; no allocation import exemption");
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
        assert_eq!(events.len(), 129);
        assert_eq!(events.last().unwrap().code, 0x544b21ff);
        assert_eq!(events.last().unwrap().values, [0]);
        let mut actual = Vec::new();
        for event in events.iter().filter(|e| e.code == 0x544b2100) {
            actual.extend_from_slice(event.bytes.as_ref().unwrap());
        }
        assert_eq!(actual, vectors);
        owner.stop().unwrap();
        println!(
            "PASS {target:?}: actual confined 128 original-C CPU/ordered-RAM/stats/IRQL/stack witnesses"
        );
    }
    for profile in ["takedawn", "opence-ce95f7de"] {
        assert!(gamepkg::build_policy::require(Some(profile)).is_err());
    }
}
