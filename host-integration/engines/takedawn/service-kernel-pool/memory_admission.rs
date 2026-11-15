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
    assert_eq!(args.len(), 4);
    let app = Path::new(&args[1]);
    let here = Path::new(&args[2]);
    let targets = Path::new(&args[3]);
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
        (
            "contiguous.rs",
            here.join("../service-contiguous/contiguous.rs"),
        ),
        ("heap_service.rs", here.join("../service-heap/service.rs")),
        ("kernel_pool.rs", here.join("kernel_pool.rs")),
        ("memory_checks.rs", here.join("memory_checks.rs")),
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
mod memory;mod registers;mod call_state;mod calls_runtime;mod vector;mod scalar;mod debug;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod memory_checks;mod game;mod dispatch;mod targets;
pub fn heap_worker(arg:u64)->u64{heap_service::heap_serve(arg)}
pub fn memory_worker(arg:u64)->u64{memory_checks::kernel_memory_worker(arg)}
fn run()->Result<(),i32>{
 let mut banks=[0u8;48];if gamepkg::local_call(1,&[],&mut banks)?!=48{return Err(-3);}
 let main=u64::from_le_bytes([banks[0],banks[1],banks[2],banks[3],banks[4],banks[5],banks[6],banks[7]]);
 let physical=u64::from_le_bytes([banks[16],banks[17],banks[18],banks[19],banks[20],banks[21],banks[22],banks[23]]);
 for window in [main,physical]{let mut req=std::vec::Vec::new();req.extend(u64::to_le_bytes(window));req.extend(u64::to_le_bytes(0));req.extend(u64::to_le_bytes(67108864));req.extend(u32::to_le_bytes(2));gamepkg::local_call(2,&req,&mut [])?;}
 let mut ram=memory::Memory::new()?;ram.attach(0,67108864,main,0,true)?;memory_checks::memory_kernel_verify(&mut ram,main,physical)
}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b0bff,&[code]);}
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
    let package = root.path().join("pool.gamepkg");
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
            vec!["heap_worker".into(), "memory_worker".into()],
            LocalLayout::TakedawnMemory,
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        println!("PASS {target:?}: focused allocator kernel/helper source admission and all-object audit before trusted linking; not full generated kit");
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
        let end = Instant::now() + Duration::from_secs(30);
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
        assert_eq!(events.last().unwrap().code, 0x544b0bff);
        assert_eq!(events.last().unwrap().values, vec![0], "{events:?}");
        let stages: Vec<_> = events.iter().filter(|e| e.code == 0x544b0b00).collect();
        assert_eq!(stages.len(), 5);
        for (index, event) in stages.iter().enumerate() {
            assert_eq!(event.values, vec![index as u64 + 1]);
        }
        let headers: Vec<_> = events.iter().filter(|e| e.code == 0x544b0a00).collect();
        assert_eq!(headers.len(), 7);
        for event in headers {
            assert_eq!(
                event.values,
                vec![
                    0x23456789,
                    u32::MAX as u64,
                    0,
                    u32::MAX as u64,
                    4096,
                    0xaa55
                ]
            );
        }
        let payloads: Vec<_> = events.iter().filter(|e| e.code == 0x544b0a01).collect();
        assert_eq!(payloads.len(), 7);
        let expected: Vec<_> = (6..=40u32)
            .flat_map(|n| (0x11000000 + n).to_le_bytes())
            .collect();
        for event in payloads {
            assert_eq!(event.bytes.as_deref(), Some(expected.as_slice()));
        }
        assert_eq!(events.len(), 20);
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined allocator kernel routing, four callers, exact bounded refusal dumps and failure controls");
    }
}
