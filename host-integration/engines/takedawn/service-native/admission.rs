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
fn read_events(path: &Path) -> Vec<(u32, u32, Vec<u64>)> {
    let bytes = fs::read(path).unwrap();
    let mut offset = 0usize;
    let word = |offset: &mut usize| {
        let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
        *offset += 4;
        value
    };
    let count = word(&mut offset);
    assert!(count <= 16384);
    let mut events = Vec::new();
    for _ in 0..count {
        let level = word(&mut offset);
        let code = word(&mut offset);
        let length = word(&mut offset);
        assert!(length <= 32);
        let mut values = Vec::new();
        for _ in 0..length {
            values.push(u64::from_le_bytes(
                bytes[offset..offset + 8].try_into().unwrap(),
            ));
            offset += 8;
        }
        events.push((level, code, values));
    }
    assert_eq!(offset, bytes.len());
    events
}
fn read_byte_events(path: &Path) -> Vec<(u32, u32, Vec<u8>)> {
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[..4], b"GPB1");
    let mut offset = 4usize;
    let word = |offset: &mut usize| {
        let value = u32::from_le_bytes(bytes[*offset..*offset + 4].try_into().unwrap());
        *offset += 4;
        value
    };
    let count = word(&mut offset);
    assert!(count <= 128);
    let mut events = Vec::new();
    for _ in 0..count {
        let level = word(&mut offset);
        let code = word(&mut offset);
        let length = word(&mut offset) as usize;
        assert!(level <= 3 && length <= 128 * 1024);
        let end = offset.checked_add(length).unwrap();
        events.push((level, code, bytes[offset..end].to_vec()));
        offset = end;
    }
    assert_eq!(offset, bytes.len());
    events
}
fn reject_bad_vectors(artifact: &preparation::Artifact) {
    let mut cases = Vec::new();
    for (magic, batches, count, error) in [
        (b"TKN7", 1u32, None, -1i16),
        (b"TKN8", 1, None, -1),
        (b"TKN9", 1, None, -1),
        (b"TKNA", 0, None, -1),
        (b"TKNA", 17, None, -1),
        (b"TKNA", 1, Some(4097), -1),
        (b"TKNA", 1, Some(1), -3),
    ] {
        let mut bytes = magic.to_vec();
        bytes.extend(batches.to_le_bytes());
        if let Some(count) = count {
            bytes.extend(u32::to_le_bytes(count));
        }
        cases.push((bytes, error));
    }
    let mut later = b"TKNA".to_vec();
    for value in [2u32, 0, 4097] {
        later.extend(value.to_le_bytes());
    }
    cases.push((later, -1));
    for (index, (bytes, error)) in cases.into_iter().enumerate() {
        let assets = tempfile::tempdir().unwrap();
        fs::write(assets.path().join("vectors"), bytes).unwrap();
        let samples = Arc::new(Mutex::new(Vec::new()));
        let mut owner = Owner::start(
            artifact,
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
        let end = Instant::now() + Duration::from_secs(15);
        loop {
            owner.dispatch(32);
            if let Some(code) = owner.poll().unwrap() {
                assert_eq!(code, 0, "{}", owner.diagnostics());
                break;
            }
            assert!(
                Instant::now() < end,
                "bad vector case {index} timed out: {}",
                owner.diagnostics()
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(
            *samples.lock().unwrap(),
            [error],
            "bad vector case {index}: {}",
            owner.diagnostics()
        );
        let (events, lost) = owner.host.take_diagnostics();
        assert!(events.is_empty() && lost == 0);
        owner.stop().unwrap();
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!([4, 5, 6].contains(&args.len()));
    let prepared = args.get(4).filter(|p| p.as_str() != "-");
    let call_targets = args.get(5);
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
    for name in [
        "memory.rs",
        "registers.rs",
        "irq_state_checks.rs",
        "fixture.rs",
        "vector.rs",
        "vector_spans.rs",
        "scalar.rs",
        "copy_checks.rs",
        "fill_checks.rs",
        "debug.rs",
        "debug_checks.rs",
        "call_state.rs",
        "call_checks.rs",
    ] {
        fs::copy(here.join(name), source.join("source/src").join(name)).unwrap();
    }
    for (name, path) in [
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
    ] {
        fs::copy(path, source.join("source/src").join(name)).unwrap();
    }
    if let Some(prepared) = prepared {
        let prepared = Path::new(prepared);
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(prepared.join("inventory.json")).unwrap()).unwrap();
        assert_eq!(report["production_ready"], false);
        assert_eq!(report["register_record_version"], 5);
        assert_eq!(report["register_record_bytes"], 316);
        assert_eq!(
            report["registers_source_sha256"],
            gamepkg::verify::digest(&fs::read(here.join("registers.rs")).unwrap())
        );
        assert_eq!(
            report["kernel_services_source_sha256"],
            gamepkg::verify::digest(&fs::read(source.join("source/src/kernel_pool.rs")).unwrap())
        );
        assert_eq!(
            report["debug_adapter_sha256"],
            gamepkg::verify::digest(&fs::read(here.join("debug.rs")).unwrap())
        );
        assert_eq!(
            report["debug_review_sha256"],
            gamepkg::verify::digest(
                &fs::read(here.join("maintainer/reviewed_debug_service.json")).unwrap()
            )
        );
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
    let mut library="mod memory;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod call_state;mod call_checks;mod copy_checks;mod fill_checks;mod debug;mod debug_checks;mod vector;mod vector_spans;mod scalar;mod registers;mod irq_state_checks;mod game;mod dispatch;mod fixture;pub fn game_main(){fixture::game_main();}\n".to_owned();
    if let Some(directory) = call_targets {
        let directory = Path::new(directory);
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("call_targets.json")).unwrap())
                .unwrap();
        assert_eq!(report["production_ready"], false);
        for (name, hash) in report["emitted_files"].as_object().unwrap() {
            assert!(gamepkg::archive::valid_path(name) && name.ends_with(".rs"));
            let data = fs::read(directory.join(name)).unwrap();
            assert_eq!(gamepkg::verify::digest(&data), hash.as_str().unwrap());
            let destination = source.join("source/src").join(name);
            fs::create_dir_all(destination.parent().unwrap()).unwrap();
            fs::write(destination, data).unwrap();
        }
        fs::copy(
            here.join("target_checks.rs"),
            source.join("source/src/target_checks.rs"),
        )
        .unwrap();
        fs::copy(
            here.join("calls_runtime.rs"),
            source.join("source/src/calls_runtime.rs"),
        )
        .unwrap();
        fs::copy(
            here.join("jump_checks.rs"),
            source.join("source/src/jump_checks.rs"),
        )
        .unwrap();
        library.push_str("mod targets;mod target_checks;mod calls_runtime;mod jump_checks;\n");
        fs::copy(
            here.join("../service-kernel-pool/pool_checks.rs"),
            source.join("source/src/pool_checks.rs"),
        )
        .unwrap();
        library.push_str("mod pool_checks;pub fn heap_worker(arg:u64)->u64{heap_service::heap_serve(arg)}pub fn pool_worker(arg:u64)->u64{pool_checks::pool_worker(arg)}\n");

        let fixture = source.join("source/src/fixture.rs");
        let original = fs::read_to_string(&fixture).unwrap();
        let marker = "crate::call_checks::call_verify()?;";
        assert_eq!(original.matches(marker).count(), 1);
        fs::write(
            fixture,
            original.replace(
                marker,
                &format!("{marker}crate::target_checks::target_verify()?;"),
            ),
        )
        .unwrap();
        let fixture = source.join("source/src/fixture.rs");
        let original = fs::read_to_string(&fixture).unwrap();
        let marker = "crate::call_checks::call_memory_verify()?;";
        assert_eq!(original.matches(marker).count(), 1);
        fs::write(
            fixture,
            original.replace(
                marker,
                &format!("{marker}crate::jump_checks::jump_verify()?;crate::pool_checks::pool_verify(&mut ram,window)?;"),
            ),
        )
        .unwrap();
    }
    if prepared.is_some() {
        fs::copy(
            here.join("fs_checks.rs"),
            source.join("source/src/fs_checks.rs"),
        )
        .unwrap();
        library.push_str("mod fs_checks;\n");
        let fixture = source.join("source/src/fixture.rs");
        let text = fs::read_to_string(&fixture).unwrap();
        let marker = "crate::call_checks::call_memory_verify()?;";
        assert_eq!(text.matches(marker).count(), 1);
        fs::write(
            fixture,
            text.replace(
                marker,
                &format!("{marker}crate::fs_checks::thread_memory_verify(&mut ram,window)?;"),
            ),
        )
        .unwrap();
    }
    fs::write(source.join("source/src/lib.rs"), library).unwrap();
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
    if let Some(directory) = call_targets {
        fs::copy(
            Path::new(directory).join("lookup-vectors"),
            assets.path().join("target-vectors"),
        )
        .unwrap();
    }
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let codegen_units = 8;
        let tasks = if call_targets.is_some() {
            vec!["heap_worker".into(), "pool_worker".into()]
        } else {
            vec![]
        };
        let mut plan = Plan::new(target, "game_main", tasks, LocalLayout::TakedawnRam, true)
            .unwrap()
            .with_codegen_units(codegen_units)
            .unwrap();
        if prepared.is_some() {
            plan = plan
                .with_compiler_memory_bytes(12 * 1024 * 1024 * 1024)
                .unwrap();
        }
        let artifact = preparation::build(project.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(artifact.receipt().plan.identity(), plan.identity());
        if prepared.is_some() {
            assert_eq!(
                serde_json::to_value(&artifact.receipt().plan).unwrap()["compiler_memory_bytes"],
                12u64 * 1024 * 1024 * 1024
            );
        }
        assert!((2..=codegen_units as usize).contains(&artifact.receipt().game.objects.len()));
        assert_eq!(
            artifact.receipt().raw_objects.len(),
            artifact.receipt().game.objects.len()
        );
        assert!(artifact
            .receipt()
            .arguments
            .iter()
            .any(|arg| arg == &format!("codegen-units={codegen_units}")));
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
        let mut observed = Vec::new();
        let end = Instant::now() + Duration::from_secs(120);
        loop {
            owner.dispatch(32);
            let (batch, lost) = owner.host.take_diagnostics();
            assert_eq!(lost, 0);
            observed.extend(batch);
            assert!(observed.len() <= 16384);
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
        assert_eq!(
            *samples.lock().unwrap(),
            [1234],
            "{}; numeric events: {:?}",
            owner.diagnostics(),
            observed
                .iter()
                .filter(|e| e.bytes.is_none())
                .collect::<Vec<_>>()
        );
        let (batch, lost) = owner.host.take_diagnostics();
        assert_eq!(lost, 0);
        observed.extend(batch);
        let mut expected_events = read_events(&vectors.with_file_name("diagnostics"));
        expected_events.push((
            2,
            0x544b0002,
            vec![
                0x12345678, 10023, 10023, 9, 10, 11, 12, 13, 14, 15, 0, 1, 2, 3, 4, 5, 6, 7, 8,
            ],
        ));
        expected_events.push((2, 0x544b0001, vec![0, 1, 1]));
        if call_targets.is_some() {
            let mut values = vec![0x8000, 0, 0];
            values.resize(19, 0);
            expected_events.push((2, 0x544b0002, values));
        }
        if call_targets.is_some() {
            for stage in 1..=7u64 {
                expected_events.push((0, 0x544b0600, vec![stage]));
            }
        }
        let byte_events: Vec<_> = observed
            .iter()
            .filter(|event| event.bytes.is_some())
            .collect();
        let expected_bytes = read_byte_events(&vectors.with_file_name("debug-diagnostics"));
        assert_eq!(byte_events.len(), expected_bytes.len());
        for (event, expected) in byte_events.iter().zip(&expected_bytes) {
            assert_eq!((event.level, event.code), (expected.0, expected.1));
            assert!(event.values.is_empty());
            assert_eq!(event.bytes.as_ref().unwrap(), &expected.2);
        }
        for (index, event) in observed.iter().enumerate() {
            assert_eq!(event.sequence, index as u64 + 1);
        }
        let observed: Vec<_> = observed
            .iter()
            .filter(|event| event.bytes.is_none())
            .collect();
        assert_eq!(observed.len(), expected_events.len());
        for (index, (event, expected)) in observed.iter().zip(&expected_events).enumerate() {
            let _ = index;
            assert_eq!(
                (event.level, event.code, &event.values),
                (expected.0, expected.1, &expected.2)
            );
        }
        owner.stop().unwrap();
        reject_bad_vectors(&artifact);
        println!("PASS {target:?}: malformed/old vector formats, batch counts and truncated records rejected by the same audited confined child");
        println!("PASS {target:?}: actual confined native execution of every generated reference vector through host-owned RAM services; in-memory output only");
        if call_targets.is_some() {
            println!("PASS {target:?}: original-target lookup vectors, exact diagnostics, untraced jumps and four configured pool ordinals with concurrent shared-heap callers; other kernel/manual routes remain unsupported");
        }
    }
    println!("Unsafe/OS reference source remains rejected; no source, import or legacy-profile exemption");
}
