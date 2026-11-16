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
    assert!(args.len() >= 6 && (args.len() - 4) % 2 == 0);
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
        ("image_imports.rs", here.join("image_imports.rs")),
        ("image_probe.rs", here.join("image_probe.rs")),
        ("disc.rs", here.join("../service-disc/disc.rs")),
        ("sha256.rs", here.join("../service-image/sha256.rs")),
        ("xbe.rs", here.join("../service-image/image.rs")),
        (
            "bootstrap.rs",
            here.join("../service-bootstrap/bootstrap.rs"),
        ),
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
mod layout;mod memory;mod registers;mod call_state;mod calls_runtime;mod vector;mod scalar;mod debug;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod kernel_data;mod signature;mod imports;mod cases;mod probe;mod controls;mod game;mod dispatch;mod targets;mod disc;mod sha256;mod xbe;mod bootstrap;mod image_imports;mod image_probe;
pub fn heap_worker(arg:u64)->u64{heap_service::heap_serve(arg)}
pub fn game_main(){let code=match image_probe::image_import_run(){Ok(())=>0u64,Err(code)=>code as i64 as u64};let _=gamepkg::diagnostic(1,0x544b0fff,&[code]);}
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
    let package = root.path().join("image-imports.gamepkg");
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
        println!("PASS {target:?}: complete image/import/helper source admission and all-object audit before trusted linking; not full generated kit");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        for pair in args[4..].chunks_exact(2) {
            let mode = pair[0].to_str().unwrap().parse::<u8>().unwrap();
            assert!(mode <= 10);
            let selected = fs::File::open(&pair[1]).unwrap();
            let mut indexed =
                gamepkg::xbox_disc::Disc::from_file(selected.try_clone().unwrap()).unwrap();
            let expected: [u8; 32] = if mode == 4 {
                let checked = indexed.verify_takedawn().unwrap();
                assert_eq!(checked.verified_files, 716);
                let text = checked.executable_sha256;
                std::array::from_fn(|index| {
                    u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap()
                })
            } else {
                let entry = indexed.file("default.xbe").unwrap();
                let mut bytes = vec![0u8; entry.size as usize];
                indexed.read_file("default.xbe", 0, &mut bytes).unwrap();
                let text = gamepkg::verify::digest(&bytes);
                std::array::from_fn(|index| {
                    u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap()
                })
            };
            let assets = tempfile::tempdir().unwrap();
            fs::write(assets.path().join("expected-sha256"), expected).unwrap();
            fs::write(assets.path().join("mode"), [mode]).unwrap();
            let mut owner = Owner::start(
                &artifact,
                Silent,
                Grants {
                    roms: vec![selected],
                    assets: Some(
                        cap_std::fs::Dir::open_ambient_dir(
                            assets.path(),
                            cap_std::ambient_authority(),
                        )
                        .unwrap(),
                    ),
                    ..Default::default()
                },
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
            assert_eq!(events.last().unwrap().code, 0x544b0fff);
            assert_eq!(
                events.last().unwrap().values,
                vec![0],
                "{}",
                owner.diagnostics()
            );
            let summaries: Vec<_> = events
                .iter()
                .filter(|event| event.code == 0x544b0f00)
                .collect();
            assert_eq!(summaries.len(), 1);
            let values = &summaries[0].values;
            assert_eq!(values[0], u64::from(mode));
            if mode < 2 || mode == 6 || mode == 7 || mode == 8 || mode == 10 {
                let count = if mode == 10 {
                    14
                } else if mode == 8 {
                    10
                } else if mode == 7 {
                    9
                } else if mode == 6 {
                    6
                } else {
                    3
                };
                assert_eq!(&values[1..], &[0x12400, count, count + 2, 0x1000000]);
            } else {
                assert_eq!(
                    values[1],
                    if mode == 3 || mode == 5 {
                        -1i64 as u64
                    } else {
                        -5i64 as u64
                    }
                );
            }
            for event in &events {
                assert!([0x544b0f00, 0x544b0f10, 0x544b0fff, 0x544b1300].contains(&event.code));
                if event.code == 0x544b1300 {
                    assert!(mode == 8 || mode == 10);
                    assert!(event.values == vec![8, 4] || event.values == vec![9, 4]);
                }
                if event.code == 0x544b0f10 {
                    assert_eq!(mode, 4);
                    println!(
                        "PRIVATE pending canonical kernel ordinals: {:?}",
                        event.values
                    );
                }
            }
            owner.stop().unwrap();
            println!("PASS {target:?} image mode {mode}: authenticated in-place image, CPU/allocator/data/import setup and explicit unmigrated/wrong-context rejection; no native entry");
        }
    }
}
