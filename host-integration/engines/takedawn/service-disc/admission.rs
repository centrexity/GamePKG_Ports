// Maintainer host: no game source, object or confinement exemption.
use gamepkg::port_host::{
    admission::CheckedSources,
    owner::{Grants, Owner},
    preparation::{self, LocalLayout, Plan, Target},
    Backend, Error, Frame, Result, WindowCommand,
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
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
    assert!(args.len() >= 3);
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
    for name in ["disc.rs", "probe.rs"] {
        fs::copy(here.join(name), source.join("source/src").join(name)).unwrap();
    }
    fs::write(
        source.join("source/src/lib.rs"),
        "mod disc;mod probe;pub fn game_main(){probe::game_main();}\n",
    )
    .unwrap();
    let mut manifest =
        gamepkg::manifest::Manifest::parse(&fs::read(source.join("manifest.toml")).unwrap())
            .unwrap();
    manifest.limits.hard_memory_bytes = 256 * 1024 * 1024;
    fs::write(
        source.join("manifest.toml"),
        toml::to_string(&manifest).unwrap(),
    )
    .unwrap();
    let mut license = fs::read(source.join("LICENSE")).unwrap();
    license.extend(b"\n\nTakedawn / XboxRecomp reference notices:\n");
    license.extend(fs::read(here.join("../service-native/UPSTREAM_LICENSES.txt")).unwrap());
    fs::write(source.join("LICENSE"), license).unwrap();
    let archive = project.path().join("disc.gamepkg");
    gamepkg::pack::pack(&source, &archive, &[173; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&archive).unwrap()).unwrap();
    fs::write(
        source.join("source/src/AAA_reference.rs"),
        "pub unsafe extern \"C\" fn raw(){std::fs::read(\"outside\");}",
    )
    .unwrap();
    assert!(
        gamepkg::pack::pack(&source, &project.path().join("denied.gamepkg"), &[173; 32])
            .unwrap_err()
            .to_string()
            .contains("forbidden")
    );
    fs::remove_file(source.join("source/src/AAA_reference.rs")).unwrap();
    let mut images = Vec::<(PathBuf, bool)>::new();
    let mut private = false;
    for name in &args[3..] {
        if name == "--private-iso" {
            private = true;
            continue;
        }
        images.push((PathBuf::from(name), private));
        private = false;
    }
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true)
            .unwrap()
            .with_codegen_units(2)
            .unwrap();
        let artifact = preparation::build(project.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(artifact.receipt().game.objects.len(), 2);
        assert_eq!(artifact.receipt().raw_objects.len(), 2);
        println!("PASS {target:?}: full source scan before bindings; separately optimized/audited objects before trusted linking; no media bridge exemption");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        for (path, authenticate) in &images {
            let selected = fs::File::open(path).unwrap();
            let mut original = gamepkg::xbox_disc::Disc::from_file(selected.try_clone().unwrap());
            if *authenticate {
                let report = original.as_mut().unwrap().verify_takedawn().unwrap();
                assert_eq!(report.verified_files, 716);
                println!("PASS: selected private ISO authenticated against existing pinned 716-file manifest, without extraction");
            }
            let mut owner = Owner::start(
                &artifact,
                Silent,
                Grants {
                    roms: vec![selected],
                    ..Default::default()
                },
                128 * 1024 * 1024,
                || {},
            )
            .unwrap();
            let end = Instant::now() + Duration::from_secs(90);
            let mut observed = Vec::new();
            loop {
                owner.dispatch(32);
                let (events, lost) = owner.host.take_diagnostics();
                assert_eq!(lost, 0, "{}", owner.diagnostics());
                for event in &events {
                    if event.code == 0x544b0103 {
                        assert_eq!(event.values.len(), 2);
                        owner.host.revoke(event.values[0]).unwrap();
                        owner.host.timing().wake().unwrap();
                    }
                }
                observed.extend(events);
                if let Some(code) = owner.poll().unwrap() {
                    assert_eq!(code, 0, "{}", owner.diagnostics());
                    break;
                }
                assert!(Instant::now() < end, "{}", owner.diagnostics());
                std::thread::sleep(Duration::from_millis(1));
            }
            let (events, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            observed.extend(events);
            owner.stop().unwrap();
            let status = observed
                .iter()
                .filter(|e| e.code == 0x544b01ff)
                .collect::<Vec<_>>();
            assert_eq!(status.len(), 1);
            if let Ok(original) = original {
                assert_eq!(status[0].values, vec![0], "{}", owner.diagnostics());
                let header = observed
                    .iter()
                    .filter(|e| e.code == 0x544b0100)
                    .collect::<Vec<_>>();
                assert_eq!(header.len(), 1);
                assert_eq!(header[0].values[0], original.partition_offset());
                let mut entries = BTreeMap::new();
                let mut checks = BTreeMap::new();
                let mut directories = 0;
                for event in &observed {
                    if event.code == 0x544b0101 {
                        let b = event.bytes.as_ref().unwrap();
                        assert!(b.len() >= 20);
                        let length = u32::from_le_bytes(b[..4].try_into().unwrap()) as usize;
                        assert_eq!(b.len(), 20 + length);
                        let name = std::str::from_utf8(&b[20..]).unwrap().to_owned();
                        let offset = u64::from_le_bytes(b[4..12].try_into().unwrap());
                        let size = u32::from_le_bytes(b[12..16].try_into().unwrap());
                        let dir = u32::from_le_bytes(b[16..20].try_into().unwrap());
                        assert!(dir <= 1);
                        let key = name.to_ascii_lowercase();
                        assert!(entries
                            .insert(key, (name, offset, size, dir == 1, entries.len() as u64))
                            .is_none());
                        if dir == 1 {
                            directories += 1;
                        }
                    } else if event.code == 0x544b0102 {
                        assert_eq!(event.values.len(), 3);
                        assert!(checks
                            .insert(event.values[0], (event.values[1], event.values[2]))
                            .is_none());
                    }
                }
                assert_eq!(header[0].values[1], entries.len() as u64);
                assert_eq!(original.files().count() + directories, entries.len());
                for entry in original.files() {
                    let found = entries.get(&entry.path.to_ascii_lowercase()).unwrap();
                    assert_eq!(
                        (found.1, found.2, found.3),
                        (entry.offset, entry.size, false)
                    );
                    let mut bytes = [0; 64];
                    let length = entry.size.min(64) as usize;
                    original
                        .read_file(&entry.path, 0, &mut bytes[..length])
                        .unwrap();
                    let mut checksum = 0u64;
                    for b in &bytes[..length] {
                        checksum = checksum.wrapping_mul(257).wrapping_add(u64::from(*b));
                    }
                    assert_eq!(checks.get(&found.4), Some(&(length as u64, checksum)));
                }
                assert_eq!(checks.len(), original.files().count());
                println!("PASS {target:?}: in-place capability reader matches existing reader for {} files and bounded data witnesses; {directories} directory records observed; no extracted bytes",checks.len());
            } else {
                assert_eq!(
                    status[0].values,
                    vec![(-1i64) as u64],
                    "invalid image must fail closed"
                );
                println!("PASS {target:?}: malformed image rejected by confined reader");
            }
        }
    }
    println!("Component evidence only: no extracted files, image execution, graphics, physical input/audio or frame pacing; content authentication still requires pinned cryptographic hashes");
}
