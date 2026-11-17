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
    assert!(args.len() >= 6);
    let mut cases_start = 4;
    let mut evidence = None;
    let mut startup_copy = None;
    let mut tick_copy = None;
    let mut tick_startup_copy = None;
    let mut timer_launch_copy = None;
    let mut linux64_only = false;
    let mut objects_layout = false;
    while args[cases_start].to_str().unwrap().starts_with("--") {
        match args[cases_start].to_str().unwrap() {
            "--evidence" => {
                evidence = Some(Path::new(&args[cases_start + 1]));
                cases_start += 2;
            }
            "--startup-copy" => {
                startup_copy = Some(Path::new(&args[cases_start + 1]));
                cases_start += 2;
            }
            "--tick-copy" => {
                tick_copy = Some(Path::new(&args[cases_start + 1]));
                cases_start += 2;
            }
            "--tick-startup-copy" => {
                tick_startup_copy = Some(Path::new(&args[cases_start + 1]));
                cases_start += 2;
            }
            "--timer-launch-copy" => {
                timer_launch_copy = Some(Path::new(&args[cases_start + 1]));
                cases_start += 2;
            }
            "--objects-layout" => {
                objects_layout = true;
                cases_start += 1;
            }
            "--linux64-only" => {
                linux64_only = true;
                cases_start += 1;
            }
            _ => panic!("unknown maintainer option"),
        }
    }
    assert!((args.len() - cases_start) % 2 == 0);
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
        (
            "semaphore_import_probe.rs",
            here.join("../service-kernel-semaphore-imports/probe.rs"),
        ),
        (
            "mixed_image_probe.rs",
            here.join("../service-kernel-mixed-waits/image_probe.rs"),
        ),
        (
            "critical_image_probe.rs",
            here.join("../service-critical/image_probe.rs"),
        ),
        (
            "rtl_image_probe.rs",
            here.join("../service-kernel-rtl/image_probe.rs"),
        ),
        (
            "sync_startup_probe.rs",
            here.join("../service-kernel-context/sync_startup_probe.rs"),
        ),
        (
            "event_import_probe.rs",
            here.join("../service-kernel-event-imports/probe.rs"),
        ),
        (
            "handles.rs",
            here.join("../service-kernel-handles/handles.rs"),
        ),
        (
            "typed_events.rs",
            here.join("../service-kernel-events/events.rs"),
        ),
        (
            "object_startup_probe.rs",
            here.join("../service-kernel-context/object_startup_probe.rs"),
        ),
        ("imports.rs", here.join("imports.rs")),
        ("image_imports.rs", here.join("image_imports.rs")),
        ("image_probe.rs", here.join("image_probe.rs")),
        (
            "kernel_startup.rs",
            startup_copy
                .map(Path::to_path_buf)
                .unwrap_or_else(|| here.join("../service-kernel-context/startup.rs")),
        ),
        (
            "startup_probe.rs",
            here.join("../service-kernel-context/startup_probe.rs"),
        ),
        (
            "tick_service.rs",
            tick_copy
                .map(Path::to_path_buf)
                .unwrap_or_else(|| here.join("../service-kernel-tick/tick.rs")),
        ),
        (
            "tick_startup.rs",
            tick_startup_copy
                .map(Path::to_path_buf)
                .unwrap_or_else(|| here.join("../service-kernel-context/tick_startup.rs")),
        ),
        (
            "tick_probe.rs",
            here.join("../service-kernel-context/tick_probe.rs"),
        ),
        (
            "wait_service.rs",
            here.join("../service-kernel-wait/wait.rs"),
        ),
        ("gate.rs", here.join("../service-kernel-gate/gate.rs")),
        ("irql.rs", here.join("../service-kernel-irql/state.rs")),
        ("queue.rs", here.join("../service-kernel-dpc/queue.rs")),
        (
            "callback.rs",
            here.join("../service-kernel-dpc/callback.rs"),
        ),
        ("drain.rs", here.join("../service-kernel-dpc/drain.rs")),
        ("dpc_clock.rs", here.join("../service-kernel-dpc/clock.rs")),
        ("stacks.rs", here.join("../service-worker-stacks/stacks.rs")),
        ("shadow.rs", here.join("../service-kernel-shadow/shadow.rs")),
        ("events.rs", here.join("../service-kernel-shadow/events.rs")),
        ("timers.rs", here.join("../service-kernel-timers/timers.rs")),
        ("firing.rs", here.join("../service-kernel-timers/firing.rs")),
        (
            "timer_clock.rs",
            here.join("../service-kernel-timers/clock.rs"),
        ),
        (
            "production_clock.rs",
            here.join("../service-kernel-timers/clock.rs"),
        ),
        (
            "thread_tib.rs",
            here.join("../service-thread-tib/thread_tib.rs"),
        ),
        (
            "worker_setup.rs",
            here.join("../service-timer-worker/worker.rs"),
        ),
        (
            "dispatching.rs",
            here.join("../service-timer-worker/dispatch.rs"),
        ),
        (
            "dpc_observer.rs",
            here.join("../service-timer-worker/dispatch-tests/observer.rs"),
        ),
        (
            "irq_hooks.rs",
            here.join("../service-timer-worker/dispatch-tests/hooks.rs"),
        ),
        (
            "dpc_routes.rs",
            here.join("../service-timer-worker/dispatch-tests/routes.rs"),
        ),
        ("pacing.rs", here.join("../service-kernel-pacing/pacing.rs")),
        (
            "schedule.rs",
            here.join("../service-kernel-pacing/shared.rs"),
        ),
        (
            "timer_startup.rs",
            timer_launch_copy
                .map(Path::to_path_buf)
                .unwrap_or_else(|| here.join("../service-kernel-context/timer_startup.rs")),
        ),
        (
            "timer_probe.rs",
            here.join("../service-kernel-context/timer_probe.rs"),
        ),
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
pub mod handles;pub mod typed_events;mod event_import_probe;mod semaphore_import_probe;mod rtl_image_probe;mod mixed_image_probe;mod critical_image_probe;mod sync_startup_probe;mod object_startup_probe;mod layout;mod memory;mod registers;mod call_state;mod calls_runtime;mod vector;mod scalar;mod debug;mod heap;mod contiguous;mod heap_service;mod kernel_pool;mod kernel_data;mod signature;mod imports;mod cases;mod probe;mod controls;mod game;mod dispatch;mod targets;mod disc;mod sha256;mod xbe;mod bootstrap;mod image_imports;mod image_probe;mod kernel_startup;mod startup_probe;mod tick_service;mod tick_startup;mod tick_probe;mod wait_service;mod gate;mod irql;mod queue;mod callback;mod drain;mod dpc_clock;mod stacks;mod shadow;mod events;mod timers;mod firing;mod timer_clock;mod production_clock;mod thread_tib;mod worker_setup;mod dispatching;mod dpc_observer;mod irq_hooks;mod dpc_routes;mod pacing;mod schedule;mod timer_startup;mod timer_probe;
pub fn object_worker(arg:u64)->u64{object_startup_probe::object_worker(arg)}
pub fn sync_worker(arg:u64)->u64{sync_startup_probe::sync_startup_worker(arg)}
pub fn heap_worker(arg:u64)->u64{heap_service::heap_serve(arg)}
pub fn startup_worker(arg:u64)->u64{startup_probe::startup_worker(arg)}
pub fn tick_worker(arg:u64)->u64{tick_service::tick_worker(arg)}
pub fn timer_worker(arg:u64)->u64{timer_probe::timer_worker(arg)}
pub fn timer_holder(arg:u64)->u64{timer_probe::timer_holder(arg)}
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
    if let Some(out) = evidence {
        let mut hashes = std::collections::BTreeMap::new();
        fn scan(root: &Path, dir: &Path, hashes: &mut std::collections::BTreeMap<String, String>) {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if entry.file_type().unwrap().is_dir() {
                    scan(root, &path, hashes);
                } else {
                    hashes.insert(
                        path.strip_prefix(root)
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_owned(),
                        gamepkg::verify::digest(&fs::read(&path).unwrap()),
                    );
                }
            }
        }
        scan(&source, &source, &mut hashes);
        fs::write(
            out.join("checked-sources.json"),
            serde_json::to_vec_pretty(&hashes).unwrap(),
        )
        .unwrap();
    }
    fs::write(
        source.join("source/src/unused_shell.rs"),
        r#"pub fn unused(){let _=std::fs::read("/etc/passwd");}"#,
    )
    .unwrap();
    assert!(gamepkg::pack::pack(&source, &root.path().join("denied.gamepkg"), &[183; 32]).is_err());
    fs::remove_file(source.join("source/src/unused_shell.rs")).unwrap();
    fs::copy(
        here.join("../service-kernel-tick/revocation.rs"),
        source.join("source/src/unused_revocation.rs"),
    )
    .unwrap();
    assert!(gamepkg::pack::pack(
        &source,
        &root.path().join("denied-revocation.gamepkg"),
        &[183; 32]
    )
    .is_err());
    fs::remove_file(source.join("source/src/unused_revocation.rs")).unwrap();
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        if linux64_only && !matches!(target, Target::Linux64) {
            continue;
        }
        let plan = Plan::new(
            target,
            "game_main",
            vec![
                "heap_worker".into(),
                "startup_worker".into(),
                "tick_worker".into(),
                "timer_worker".into(),
                "timer_holder".into(),
                "object_worker".into(),
                "sync_worker".into(),
            ],
            if objects_layout {
                LocalLayout::TakedawnServices
            } else {
                LocalLayout::TakedawnMemory
            },
            true,
        )
        .unwrap()
        .with_codegen_units(2)
        .unwrap();
        let artifact = preparation::build(root.path(), &checked, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        assert_eq!(
            artifact.receipt().game.sdk,
            gamepkg::port_host::build_identity()
        );
        assert_eq!(artifact.receipt().game.source_kit, checked.identity());
        assert_eq!(
            artifact.receipt().raw_objects.len(),
            artifact.receipt().game.objects.len()
        );
        if let Some(out) = evidence {
            fs::write(
                out.join(format!("receipt-{target:?}.json")),
                serde_json::to_vec_pretty(artifact.receipt()).unwrap(),
            )
            .unwrap();
        }
        println!("PASS {target:?}: complete image/import/helper source admission and all-object audit before trusted linking; not full generated kit");
        if matches!(target, Target::Windows64 | Target::Windows32) {
            continue;
        }
        for pair in args[cases_start..].chunks_exact(2) {
            let mode = pair[0].to_str().unwrap().parse::<u8>().unwrap();
            assert!(mode <= 85);
            assert_eq!(objects_layout, mode >= 51);
            let selected = fs::File::open(&pair[1]).unwrap();
            let mut indexed =
                gamepkg::xbox_disc::Disc::from_file(selected.try_clone().unwrap()).unwrap();
            let expected: [u8; 32] = if mode == 4
                || mode == 23
                || mode == 49
                || mode == 67
                || mode == 79
                || mode == 84
            {
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
                "{} {events:?}",
                owner.diagnostics()
            );
            let summaries: Vec<_> = events
                .iter()
                .filter(|event| event.code == 0x544b0f00)
                .collect();
            assert_eq!(summaries.len(), 1);
            let values = &summaries[0].values;
            assert_eq!(values[0], u64::from(mode));
            if mode >= 51 {
                let code = if mode == 52 || mode == 53 {
                    (-4i64) as u64
                } else if mode == 54 || (mode >= 55 && mode <= 61) || (mode >= 76 && mode <= 78) {
                    (-2i64) as u64
                } else if mode == 67 || mode == 79 || mode == 84 || mode == 85 {
                    (-5i64) as u64
                } else {
                    0
                };
                assert_eq!(
                    &values[1..],
                    &[
                        code,
                        if mode == 83 || mode == 85 {
                            13
                        } else if mode == 81 {
                            15
                        } else if mode >= 69 {
                            10
                        } else if mode == 68 {
                            8
                        } else {
                            6
                        },
                        0
                    ]
                );
            } else if mode >= 12 {
                let expected = if mode >= 19 && mode <= 21 || mode >= 44 && mode <= 48 {
                    -2i64 as u64
                } else if mode == 13 || mode == 23 || mode == 49 {
                    -5i64 as u64
                } else if mode == 14 || mode == 15 || mode == 17 {
                    -1i64 as u64
                } else {
                    0
                };
                assert_eq!(&values[1..], &[expected, 5, 0]);
            } else if mode < 2 || mode == 6 || mode == 7 || mode == 8 || mode == 10 || mode == 11 {
                let count = if mode == 11 {
                    5
                } else if mode == 10 {
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
                assert!([
                    0x544b0f00, 0x544b0f10, 0x544b0fff, 0x544b1300, 0x544b3200, 0x544b1710,
                    0x544b1800, 0x544b1801, 0x544b3b00, 0x544b3900, 0x544b3c00, 0x544b3d00,
                    0x544b3f00, 0x544b4100, 0x544b4300
                ]
                .contains(&event.code));
                if event.code == 0x544b1300 {
                    assert!(mode == 8 || mode == 10);
                    assert!(event.values == vec![8, 4] || event.values == vec![9, 4]);
                }
                if event.code == 0x544b3200 {
                    assert!(mode == 11 || mode == 16 || mode == 18);
                    assert_eq!(event.values, [3, 0x1d000, 0x1100, 0x89abcdef]);
                }
                if event.code == 0x544b0f10 {
                    assert!(
                        mode == 4
                            || mode == 23
                            || mode == 49
                            || mode == 67
                            || mode == 79
                            || mode == 84
                    );
                    println!(
                        "PRIVATE mode {mode} pending canonical kernel ordinals: {:?}",
                        event.values
                    );
                }
            }
            if mode == 67 {
                assert_eq!(
                    events
                        .iter()
                        .filter(|e| e.code == 0x544b0f10)
                        .map(|e| e.values.len())
                        .sum::<usize>(),
                    100
                );
            }
            if mode == 79 {
                assert_eq!(
                    events
                        .iter()
                        .filter(|e| e.code == 0x544b0f10)
                        .map(|e| e.values.len())
                        .sum::<usize>(),
                    98
                );
            }
            if mode == 84 {
                assert_eq!(
                    events
                        .iter()
                        .filter(|e| e.code == 0x544b0f10)
                        .map(|e| e.values.len())
                        .sum::<usize>(),
                    95
                );
            }
            if mode == 83 {
                let calls: Vec<_> = events.iter().filter(|e| e.code == 0x544b4300).collect();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].values, [2, 2, 1, 1, 1]);
            }
            if mode == 82 {
                let calls: Vec<_> = events
                    .iter()
                    .filter(|event| event.code == 0x544b4100)
                    .collect();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].values, [(-2i64) as u64, 0, 0, 0, 1, 1, 1, 1]);
            }
            if mode == 81 {
                let calls: Vec<_> = events
                    .iter()
                    .filter(|event| event.code == 0x544b3f00)
                    .collect();
                assert_eq!(calls.len(), 1);
                assert_eq!(calls[0].values, [8, 0, 0, 1, 3, 4, 4, 6, 997, 317]);
            }
            if mode == 70 {
                let vectors: Vec<_> = events.iter().filter(|e| e.code == 0x544b3c00).collect();
                assert_eq!(vectors.len(), 1);
                let bytes = vectors[0].bytes.as_ref().unwrap();
                assert_eq!(bytes.len(), 3100);
                assert_eq!(
                    gamepkg::verify::digest(bytes),
                    "90ac7f1f727225e3f9cc67bbc49fbc9680fdddd8f8cdd517dda759cde54dfdc6"
                );
            }
            if mode == 68 {
                let vectors: Vec<_> = events.iter().filter(|e| e.code == 0x544b3b00).collect();
                assert_eq!(vectors.len(), 1);
                let bytes = vectors[0].bytes.as_ref().unwrap();
                assert_eq!(bytes.len(), 1764);
                // Pinned original-C fixture result; no game asset or native pointer.
                assert_eq!(
                    gamepkg::verify::digest(bytes),
                    "980b967181bb3ce093bdc54e5a8e62a9abde5ac5a413284f56159351d3255c8c"
                );
            }
            let connections = events.iter().filter(|e| e.code == 0x544b3200).count();
            assert_eq!(
                connections,
                if mode == 16 || mode == 18 {
                    4
                } else if mode == 11 {
                    1
                } else {
                    0
                }
            );
            if let Some(out) = evidence {
                let reports: Vec<_> = events
                    .iter()
                    .map(|e| serde_json::json!({"code":e.code,"values":e.values,"bytes":e.bytes}))
                    .collect();
                fs::write(
                    out.join(format!("{target:?}-mode-{mode}.json")),
                    serde_json::to_vec_pretty(&reports).unwrap(),
                )
                .unwrap();
            }
            owner.stop().unwrap();
            println!("PASS {target:?} image mode {mode}: authenticated in-place image, CPU/allocator/data/import setup and explicit unmigrated/wrong-context rejection; no native entry");
        }
    }
}
