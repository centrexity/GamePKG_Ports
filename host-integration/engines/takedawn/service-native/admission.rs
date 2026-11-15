// Maintainer harness: exact source admission, object audit, trusted linking,
// then actual confined native execution with generated reference vectors.
use gamepkg::port_host::{
    admission::CheckedSources,
    owner::{Grants, Owner},
    preparation::{self, LocalLayout, Plan, Target},
    Backend, Frame, Result, WindowCommand,
};
use std::{
    fs,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
struct Silent(Arc<Mutex<Vec<i16>>>);
impl Backend for Silent {
    fn window(&mut self, _: WindowCommand) -> Result<()> {
        Err(gamepkg::port_host::Error::Unsupported)
    }
    fn present(&mut self, _: Frame) -> Result<()> {
        Err(gamepkg::port_host::Error::Unsupported)
    }
    fn audio(&mut self, bytes: &[i16], _: u32) -> Result<()> {
        self.0.lock().unwrap().extend(bytes);
        Ok(())
    }
    fn rumble(&mut self, _: u64, _: u16, _: u16, _: u32) -> Result<()> {
        Err(gamepkg::port_host::Error::Unsupported)
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!([4, 5].contains(&args.len()));
    let app = Path::new(&args[1]);
    let here = Path::new(&args[2]);
    let vectors = Path::new(&args[3]);
    let project = tempfile::tempdir().unwrap();
    let source = project.path().join("package");
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
    for name in ["memory.rs", "registers.rs", "fixture.rs", "vector.rs"] {
        fs::copy(here.join(name), source.join("source/src").join(name)).unwrap();
    }
    if args.len() == 5 {
        let prepared = Path::new(&args[4]);
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(prepared.join("inventory.json")).unwrap()).unwrap();
        assert_eq!(report["production_ready"], false);
        for (name, hash) in report["emitted_files"].as_object().unwrap() {
            assert!(gamepkg::archive::valid_path(name) && name.ends_with(".rs"));
            let data = fs::read(prepared.join(name)).unwrap();
            assert_eq!(gamepkg::verify::digest(&data), hash.as_str().unwrap());
            let destination = source.join("source/src").join(name);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, data).unwrap();
        }
    } else {
        for name in ["game.rs", "dispatch.rs"] {
            fs::copy(
                here.join("generated").join(name),
                source.join("source/src").join(name),
            )
            .unwrap();
        }
    }
    fs::write(source.join("source/src/lib.rs"),"mod memory;mod vector;mod registers;mod game;mod dispatch;mod fixture;pub fn game_main(){fixture::game_main();}\n").unwrap();
    // Preserve the source's attribution in the temporary signed test package.
    let mut licenses = fs::read(source.join("LICENSE")).unwrap();
    licenses.extend(b"\n\nUpstream Takedawn / XboxRecomp notices:\n");
    licenses.extend(fs::read(here.join("UPSTREAM_LICENSES.txt")).unwrap());
    fs::write(source.join("LICENSE"), licenses).unwrap();
    let mut manifest =
        gamepkg::manifest::Manifest::parse(&fs::read(source.join("manifest.toml")).unwrap())
            .unwrap();
    manifest.permissions.audio = true;
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    fs::write(
        source.join("manifest.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let package = project.path().join("native-functions.gamepkg");
    gamepkg::pack::pack(&source, &package, &[173; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    // An accidentally packaged reference/oracle never receives a source exception.
    fs::write(
        source.join("source/src/AAA_unreviewed_reference.rs"),
        "pub unsafe extern \"C\" fn reference(){std::fs::read(\"secret\");}",
    )
    .unwrap();
    let denied = gamepkg::pack::pack(&source, &project.path().join("denied.gamepkg"), &[173; 32])
        .unwrap_err();
    assert!(denied.to_string().contains("forbidden"), "{denied:#}");
    fs::remove_file(source.join("source/src/AAA_unreviewed_reference.rs")).unwrap();
    if args[3] == "--source-only" {
        println!("PASS: complete converted component scanned before bindings; unused unsafe reference rejected");
        return;
    }
    let assets = tempfile::tempdir().unwrap();
    fs::copy(vectors, assets.path().join("vectors")).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let mut plan = Plan::new(target, "game_main", vec![], LocalLayout::TakedawnRam, true)
            .unwrap()
            .with_codegen_units(4)
            .unwrap();
        if args.len() == 5 {
            plan = plan
                .with_compiler_memory_bytes(8 * 1024 * 1024 * 1024)
                .unwrap();
        }
        let artifact = preparation::build(project.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert!((2..=4).contains(&artifact.receipt().game.objects.len()));
        assert_eq!(
            artifact.receipt().raw_objects.len(),
            artifact.receipt().game.objects.len()
        );
        assert!(artifact
            .receipt()
            .arguments
            .iter()
            .any(|arg| arg == "codegen-units=4"));
        for (raw, audited) in artifact
            .receipt()
            .raw_objects
            .iter()
            .zip(&artifact.receipt().game.objects)
        {
            assert_eq!(raw.name, audited.name);
            assert_eq!(raw.sha256.len(), 64);
            assert_eq!(audited.sha256.len(), 64);
        }

        assert_eq!(
            artifact.receipt().game.sdk,
            gamepkg::port_host::build_identity()
        );
        println!("PASS {target:?}: exact converted source checked before bindings, multiple independently optimized game objects, all audited before trusted runtime linking");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        let samples = Arc::new(Mutex::new(Vec::new()));
        let mut owner = Owner::start(
            &artifact,
            Silent(samples.clone()),
            Grants {
                assets: Some(
                    cap_std::fs::Dir::open_ambient_dir(assets.path(), cap_std::ambient_authority())
                        .unwrap(),
                ),
                ..Default::default()
            },
            256 * 1024 * 1024,
            || {},
        )
        .unwrap();
        let end = Instant::now() + Duration::from_secs(120);
        loop {
            owner.dispatch(32);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(
                Instant::now() < end,
                "native conversion fixture timed out: {}",
                owner.diagnostics()
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(*samples.lock().unwrap(), [1234], "{}", owner.diagnostics());
        owner.stop().unwrap();
        println!("PASS {target:?}: actual confined native execution of every generated reference vector through host-owned RAM services; in-memory output only");
    }
    println!("Unsafe/OS reference source remains rejected; no source, import or legacy-profile exemption");
}
