// Maintainer-only: terminate actual audited/confined native children at save
// checkpoints, then run their actual startup scanner on the retained files.
use gamepkg::port_host::{
    admission::CheckedSources,
    owner::{Grants, Owner, SaveGrant},
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
fn save_directory(root: &Path, manifest: &gamepkg::manifest::Manifest) -> std::path::PathBuf {
    let id = gamepkg::verify::digest(
        format!(
            "{}\0{}\0{}",
            manifest.id, manifest.creator_key, manifest.save_schema
        )
        .as_bytes(),
    );
    root.join("port-state").join(id).join("files")
}
fn save_grant(root: &Path, manifest: &gamepkg::manifest::Manifest) -> SaveGrant {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match SaveGrant::open(root, manifest, 1024 * 1024) {
            Ok(grant) => return grant,
            Err(error) => {
                assert!(
                    Instant::now() < deadline,
                    "save lease was not released: {error:#}"
                );
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
}
fn execute(
    artifact: &preparation::Artifact,
    root: &Path,
    assets: &Path,
    expected: i16,
    kill: bool,
) {
    let samples = Arc::new(Mutex::new(Vec::new()));
    let save = save_grant(root, artifact.manifest());
    let mut owner = Owner::start(
        artifact,
        Silent(samples.clone()),
        Grants {
            assets: Some(
                cap_std::fs::Dir::open_ambient_dir(assets, cap_std::ambient_authority()).unwrap(),
            ),
            saves: Some(save),
            ..Default::default()
        },
        256 * 1024 * 1024,
        || {},
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        owner.dispatch(32);
        if kill && samples.lock().unwrap().contains(&expected) {
            owner.stop().unwrap();
            break;
        }
        if let Some(code) = owner.poll().unwrap() {
            assert!(
                !kill && code == 0,
                "unexpected native exit {code}: {}",
                owner.diagnostics()
            );
            assert!(
                samples.lock().unwrap().contains(&expected),
                "native scanner did not finish: {}",
                owner.diagnostics()
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "native save checkpoint timeout: {}",
            owner.diagnostics()
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    drop(owner);
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3);
    let app = Path::new(&args[1]);
    let here = Path::new(&args[2]);
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
    for module in [
        "paths.rs",
        "files.rs",
        "state.rs",
        "adapter.rs",
        "history.rs",
        "recovery.rs",
    ] {
        fs::copy(here.join(module), source.join("source/src").join(module)).unwrap();
    }
    fs::copy(
        here.join("native_game.rs"),
        source.join("source/src/lib.rs"),
    )
    .unwrap();
    let mut manifest =
        gamepkg::manifest::Manifest::parse(&fs::read(source.join("manifest.toml")).unwrap())
            .unwrap();
    manifest.permissions.save = true;
    manifest.permissions.audio = true;
    manifest.limits.hard_memory_bytes = 384 * 1024 * 1024;
    fs::write(
        source.join("manifest.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let archive = project.path().join("synthetic-save.gamepkg");
    gamepkg::pack::pack(&source, &archive, &[173; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&archive).unwrap()).unwrap();
    for target in [Target::Linux64, Target::Linux32] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true).unwrap();
        let artifact = preparation::build(project.path(), &checked, &plan, vec![], vec![]).unwrap();
        for phase in 1..=5u8 {
            let data = tempfile::tempdir().unwrap();
            let controls = tempfile::tempdir().unwrap();
            fs::write(controls.path().join("phase"), [phase]).unwrap();
            let grant = save_grant(data.path(), checked.manifest());
            drop(grant);
            let base = save_directory(data.path(), checked.manifest());
            fs::create_dir_all(base.join("UDATA/12345678")).unwrap();
            let real = base.join("UDATA/12345678/profile");
            fs::write(&real, b"old-save").unwrap();
            execute(
                &artifact,
                data.path(),
                controls.path(),
                i16::from(phase),
                true,
            );
            // Opening this lease confirms the previous session's file worker
            // and retained handles have actually released the save scope.
            drop(save_grant(data.path(), checked.manifest()));
            assert_eq!(
                fs::read(&real).unwrap(),
                if phase == 5 { b"new-save" } else { b"old-save" }
            );
            fs::write(controls.path().join("phase"), [0]).unwrap();
            execute(&artifact, data.path(), controls.path(), 100, false);
            drop(save_grant(data.path(), checked.manifest()));
            assert_eq!(
                fs::read(&real).unwrap(),
                if phase == 5 { b"new-save" } else { b"old-save" }
            );
            assert!(!base.join("UDATA/12345678/profile.~new").exists());
        }
        println!("PASS {target:?}: five real process terminations and five confined startup recoveries; dummy output only");
    }
    println!("Actual native processes/temporary saves, not power-loss, real game saves, Windows runtime or gameplay qualification");
}
