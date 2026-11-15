// Maintainer host. Raw original oracle and direct filesystem are never game code.
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
mod reference_cases;
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
fn digest(bytes: &[u8]) -> [u8; 32] {
    let text = gamepkg::verify::digest(bytes);
    std::array::from_fn(|n| u8::from_str_radix(&text[n * 2..n * 2 + 2], 16).unwrap())
}
struct Expected {
    bytes: Vec<u8>,
    ram: Vec<[u8; 32]>,
    table: [u8; 32],
    stats: [u32; 6],
    operations: u32,
}
fn execute(
    artifact: &preparation::Artifact,
    bytes: &[u8],
) -> Vec<gamepkg::port_host::diagnostics::Event> {
    let assets = tempfile::tempdir().unwrap();
    fs::write(assets.path().join("vectors"), &bytes).unwrap();
    let mut owner = Owner::start(
        artifact,
        Silent,
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
    owner.stop().unwrap();
    events
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
        ("heap_service.rs", here.join("../service-heap/service.rs")),
        (
            "call_state.rs",
            here.join("../service-native/call_state.rs"),
        ),
        ("sha256.rs", here.join("../service-image/sha256.rs")),
        ("heap.rs", here.join("../service-heap/heap.rs")),
        ("contiguous.rs", here.join("contiguous.rs")),
        ("probe.rs", here.join("probe.rs")),
        ("denials.rs", here.join("denials.rs")),
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    fs::write(source.join("source/src/lib.rs"),"mod memory;mod call_state;mod heap_service;mod sha256;mod heap;mod contiguous;mod denials;mod probe;pub fn game_main(){probe::game_main();}\n").unwrap();
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
    let package = root.path().join("contiguous.gamepkg");
    gamepkg::pack::pack(&source, &package, &[179; 32]).unwrap();
    let checked =
        CheckedSources::verify(&mut gamepkg::archive::Package::open(&package).unwrap()).unwrap();
    fs::write(
        source.join("source/src/AAA_unreviewed.rs"),
        "pub fn bypass(){let _=std::fs::read(\"/etc/passwd\");}\n",
    )
    .unwrap();
    let bad = root.path().join("bad.gamepkg");
    assert!(gamepkg::pack::pack(&source, &bad, &[179; 32])
        .unwrap_err()
        .to_string()
        .contains("forbidden"));
    fs::remove_file(source.join("source/src/AAA_unreviewed.rs")).unwrap();
    let mut expected = Vec::new();
    for index in 0..reference_cases::CASES {
        let fixture = reference_cases::make_case(index);
        let mut table = Vec::new();
        for record in &fixture.records {
            for word in record {
                table.extend(word.to_le_bytes());
            }
        }
        expected.push(Expected {
            bytes: fixture.bytes,
            ram: fixture.ram.chunks_exact(1048576).map(digest).collect(),
            table: digest(&table),
            stats: fixture.stats,
            operations: fixture.operations,
        });
    }
    let mut malformed = Vec::new();
    malformed.push(("short header", expected[0].bytes[..7].to_vec(), -3i64));
    let mut bad = expected[0].bytes.clone();
    bad[0] = 0;
    malformed.push(("bad magic", bad, -1));
    let mut bad = expected[4].bytes.clone();
    bad[4..8].copy_from_slice(&200001u32.to_le_bytes());
    malformed.push(("excessive count", bad, -1));
    let mut bad = expected[4].bytes.clone();
    bad.push(0);
    malformed.push(("trailing data", bad, -1));
    let mut bad = expected[0].bytes.clone();
    bad.pop();
    malformed.push(("truncated record", bad, -1));
    let mut bad = expected[0].bytes.clone();
    bad[4..8].copy_from_slice(&0u32.to_le_bytes());
    malformed.push(("mismatched count", bad, -1));
    let mut one = expected[0].bytes[..56].to_vec();
    one[4..8].copy_from_slice(&1u32.to_le_bytes());
    let mut bad = one.clone();
    bad[8..12].copy_from_slice(&99u32.to_le_bytes());
    malformed.push(("unknown operation", bad, -1));
    let mut bad = one.clone();
    bad[28] ^= 1;
    malformed.push(("forged result witness", bad, -3));
    let mut bad = one.clone();
    bad[32] ^= 1;
    malformed.push(("forged state witness", bad, -3));
    one[8..12].copy_from_slice(&4u32.to_le_bytes());
    one[16..20].copy_from_slice(&65537u32.to_le_bytes());
    malformed.push(("oversized fixture write", one, -1));
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(
            target,
            "game_main",
            vec![],
            LocalLayout::TakedawnMemory,
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
        println!("PASS {target:?}: complete source checked before bindings, all separately optimized game objects audited before trusted linking");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        for (index, fixture) in expected.iter().enumerate() {
            let events = execute(&artifact, &fixture.bytes);
            let status: Vec<_> = events.iter().filter(|e| e.code == 0x544b08ff).collect();
            assert_eq!(status.len(), 1);
            assert_eq!(status[0].values, vec![0]);
            let stats: Vec<_> = events.iter().filter(|e| e.code == 0x544b0800).collect();
            assert_eq!(stats.len(), 1);
            assert_eq!(stats[0].values, fixture.stats.map(u64::from));
            let table: Vec<_> = events.iter().filter(|e| e.code == 0x544b0803).collect();
            assert_eq!(table.len(), 1);
            assert_eq!(table[0].bytes.as_deref(), Some(fixture.table.as_slice()));
            let mut blocks = BTreeMap::new();
            for event in events.iter().filter(|e| e.code == 0x544b0802) {
                let bytes = event.bytes.as_ref().unwrap();
                assert_eq!(bytes.len(), 36);
                let index = u32::from_le_bytes(bytes[..4].try_into().unwrap());
                assert!(blocks.insert(index, bytes[4..].to_vec()).is_none());
            }
            assert_eq!(blocks.len(), 64);
            for (number, hash) in fixture.ram.iter().enumerate() {
                assert_eq!(
                    blocks.get(&(number as u32)).unwrap(),
                    hash,
                    "RAM chunk {number}, case {index}"
                );
            }
            assert_eq!(events.len(), 67);
            println!("PASS {target:?} case {index}: {} original contiguous operations, all RAM and page-table hashes, per-operation results/state; no native entry",fixture.operations);
        }
        for (name, bytes, error) in &malformed {
            let events = execute(&artifact, bytes);
            assert_eq!(events.len(), 1, "{target:?}: {name}");
            assert_eq!(events[0].code, 0x544b08ff);
            assert_eq!(events[0].values, vec![*error as u64], "{target:?}: {name}");
        }
        println!(
            "PASS {target:?}: ten malformed/forged fixture cases reject without a success witness"
        );
    }
}
