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
    let vectors = Path::new(&args[3]);
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
        ("cases.rs", here.join("cases.rs")),
        ("probe.rs", here.join("probe.rs")),
        ("controls.rs", here.join("controls.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),r#"
mod memory;mod registers;mod call_state;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod cases;mod probe;mod controls;
fn run()->Result<(),i32>{for mode in 0..96u32{let bytes=probe::crypto_case(mode)?;gamepkg::diagnostic_bytes(1,0x544b1100+mode,&bytes)?;}controls::crypto_denials()}
pub fn game_main(){let code=match run(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b11ff,&[code]);}
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
    let package = root.path().join("crypto.gamepkg");
    gamepkg::pack::pack(&source, &package, &[183; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
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
        println!("PASS {target:?}: complete RC4/HMAC/kernel/helper source admission and all-object audit before trusted linking; not complete title");
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
        assert_eq!(events.last().unwrap().code, 0x544b11ff);
        assert_eq!(events.last().unwrap().values, vec![0], "{events:?}");
        assert_eq!(events.len(), 97);
        let expected = fs::read(vectors).unwrap();
        let mut offset = 0;
        for (mode, event) in events[..96].iter().enumerate() {
            assert_eq!(event.code, 0x544b1100 + mode as u32);
            let length =
                u32::from_le_bytes(expected[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;
            assert_eq!(
                event.bytes.as_deref().unwrap(),
                &expected[offset..offset + length],
                "mode {mode}"
            );
            offset += length;
        }
        assert_eq!(offset, expected.len());
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined original RC4/HMAC witnesses, full CPU and null/extent/protection/revocation controls");
    }
}
