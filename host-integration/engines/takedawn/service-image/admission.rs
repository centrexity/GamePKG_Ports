// Maintainer host: exact original C section loop is an inert oracle only.
use gamepkg::port_host::{
    admission::CheckedSources,
    owner::{Grants, Owner},
    preparation::{self, LocalLayout, Plan, Target},
    Backend, Error, Frame, Result, WindowCommand,
};
use std::{
    collections::BTreeMap,
    fs,
    path::Path,
    time::{Duration, Instant},
};
extern "C" {
    fn gamepkg_xbe_reference(xbe: *const u8, size: usize, ram: *mut u8, bounds: *mut u32);
}
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
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn digest(b: &[u8]) -> [u8; 32] {
    let text = gamepkg::verify::digest(b);
    std::array::from_fn(|n| u8::from_str_radix(&text[n * 2..n * 2 + 2], 16).unwrap())
}
fn reference(bytes: &[u8], ram: &mut [u8]) -> [u32; 4] {
    // Guard every access of the unsafe original oracle. Invalid inputs never
    // enter C; the production Rust plan must reject them before touching RAM.
    assert!(bytes.len() >= 0x178);
    let base = word(bytes, 0x104);
    assert_eq!(base, 0x10000);
    let count = word(bytes, 0x11c);
    assert!((1..=64).contains(&count));
    let table = word(bytes, 0x120).checked_sub(base).unwrap() as usize;
    assert!(table + count as usize * 56 <= bytes.len());
    for n in 0..count as usize {
        let at = table + n * 56;
        let va = word(bytes, at + 4) as u64;
        let vsize = word(bytes, at + 8) as u64;
        let raw = word(bytes, at + 12) as u64;
        let size = word(bytes, at + 16) as u64;
        assert!(va >= 0x10000 && va + vsize <= ram.len() as u64);
        assert!(size.min(vsize) == 0 || raw + size.min(vsize) <= bytes.len() as u64);
    }
    let mut bounds = [0u32; 4];
    unsafe {
        gamepkg_xbe_reference(
            bytes.as_ptr(),
            bytes.len(),
            ram.as_mut_ptr(),
            bounds.as_mut_ptr(),
        );
    }
    bounds
}
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    assert!(args.len() >= 3 && (args.len() - 3) % 2 == 0);
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
        ("disc.rs", here.join("../service-disc/disc.rs")),
        ("memory.rs", here.join("../service-native/memory.rs")),
        (
            "call_state.rs",
            here.join("../service-native/call_state.rs"),
        ),
        ("sha256.rs", here.join("sha256.rs")),
        ("xbe.rs", here.join("image.rs")),
        ("probe.rs", here.join("probe.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),"mod disc;mod memory;mod call_state;mod sha256;mod xbe;mod probe;pub fn game_main(){probe::game_main();}\n").unwrap();
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
    let package = root.path().join("image.gamepkg");
    gamepkg::pack::pack(&source, &package, &[173; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    fs::write(
        source.join("source/src/AAA_unreviewed.rs"),
        "pub unsafe extern \"C\" fn original(){std::fs::read(\"outside\");}",
    )
    .unwrap();
    assert!(
        gamepkg::pack::pack(&source, &root.path().join("denied.gamepkg"), &[173; 32])
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
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::TakedawnRam, true)
            .unwrap()
            .with_codegen_units(2)
            .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(artifact.receipt().game.objects.len(), 2);
        assert_eq!(artifact.receipt().raw_objects.len(), 2);
        println!("PASS {target:?}: complete source checked before bindings; two optimized game objects individually audited before trusted linking; no image/kernel exemption");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        for pair in args[3..].chunks_exact(2) {
            let mode = pair[0].to_str().unwrap();
            assert!(matches!(
                mode,
                "valid" | "invalid" | "revoke" | "hash-mismatch" | "private"
            ));
            let selected = fs::File::open(&pair[1]).unwrap();
            let mut disc =
                gamepkg::xbox_disc::Disc::from_file(selected.try_clone().unwrap()).unwrap();
            let authenticated = if mode == "private" {
                let report = disc.verify_takedawn().unwrap();
                assert_eq!(report.verified_files, 716);
                let text = report.executable_sha256;
                let expected: [u8; 32] = std::array::from_fn(|n| {
                    u8::from_str_radix(&text[n * 2..n * 2 + 2], 16).unwrap()
                });
                println!(
                    "PASS: private ISO's pinned manifest authenticated without extracting any file"
                );
                Some(expected)
            } else {
                None
            };
            let entry = disc.file("default.xbe").unwrap();
            assert!(entry.size <= 16777216);
            let mut bytes = vec![0u8; entry.size as usize];
            disc.read_file("default.xbe", 0, &mut bytes).unwrap();
            let hash = digest(&bytes);
            if let Some(authenticated) = authenticated {
                assert_eq!(
                    hash, authenticated,
                    "XBE changed after manifest authentication"
                );
            }
            let mut expected = authenticated.unwrap_or(hash);
            if mode == "hash-mismatch" {
                expected[0] ^= 1;
            }
            let mut ram = vec![0xa5u8; 67108864];
            let bounds = if matches!(mode, "valid" | "private") {
                Some(reference(&bytes, &mut ram))
            } else {
                None
            };
            let assets = tempfile::tempdir().unwrap();
            fs::write(assets.path().join("expected-sha256"), expected).unwrap();
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
            let end = Instant::now() + Duration::from_secs(90);
            let mut observed = Vec::new();
            loop {
                owner.dispatch(32);
                let (events, lost) = owner.host.take_diagnostics();
                assert_eq!(lost, 0, "{}", owner.diagnostics());
                for event in &events {
                    if event.code == 0x544b0203 {
                        assert_eq!(event.values.len(), 2);
                        if mode == "revoke" {
                            owner.host.revoke(event.values[0]).unwrap();
                        }
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
            assert!(
                !observed.iter().any(|e| e.code == 0x544b02fe),
                "{}",
                owner.diagnostics()
            );
            let status = observed
                .iter()
                .filter(|e| e.code == 0x544b02ff)
                .collect::<Vec<_>>();
            assert_eq!(status.len(), 1);
            let code = match mode {
                "invalid" | "hash-mismatch" => -1i64,
                "revoke" => -4,
                _ => 0,
            };
            assert_eq!(
                status[0].values,
                vec![code as u64],
                "{}",
                owner.diagnostics()
            );
            let mut blocks = BTreeMap::new();
            for event in observed.iter().filter(|e| e.code == 0x544b0202) {
                let payload = event.bytes.as_ref().unwrap();
                assert_eq!(payload.len(), 36);
                assert!(blocks
                    .insert(word(payload, 0), payload[4..].to_vec())
                    .is_none());
            }
            assert_eq!(blocks.len(), 64);
            for (index, part) in ram.chunks_exact(1048576).enumerate() {
                assert_eq!(
                    blocks.get(&(index as u32)).unwrap(),
                    &digest(part),
                    "RAM chunk {index}, mode {mode}"
                );
            }
            if let Some(bounds) = bounds {
                let report = observed.iter().find(|e| e.code == 0x544b0200).unwrap();
                let name = Path::new(&pair[1]).file_stem().unwrap().to_str().unwrap();
                let entry = if mode == "private" {
                    0x001d2807u32
                } else if name == "header-overlap" {
                    0x10020
                } else {
                    0x11020
                };
                let thunk = if mode == "private" {
                    0x0036b7c0u32
                } else if name == "virtual-overlap" {
                    0x11700
                } else {
                    0x12000
                };
                let tls = if mode == "private" { 3587964u32 } else { 0 };
                let sections = if mode == "private" { 17u32 } else { 2 };
                assert_eq!(
                    report.values[..4],
                    [entry, thunk, tls, sections].map(u64::from)
                );
                assert_eq!(report.values[4..], bounds.map(u64::from));
                let actual = observed.iter().find(|e| e.code == 0x544b0201).unwrap();
                assert_eq!(actual.bytes.as_deref(), Some(hash.as_slice()));
            }
            owner.stop().unwrap();
            println!("PASS {target:?} {mode}: all 64 MiB RAM chunk hashes match original section-copy/BSS/order behavior, or unchanged sentinel on rejected input; no code execution");
        }
    }
    println!("Image-load component only: no bootstrap/kernel initialization, native entrypoint, rendering, physical controls, audio or pacing qualification");
}
