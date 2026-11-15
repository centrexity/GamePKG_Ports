//! Maintainer executable using the built app's actual policy/preparation APIs.
//! Temporary test signatures are not production creator/admission credentials.
use std::{fs, path::Path};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(args.len() == 4 || args.len() == 5);
    let app = Path::new(&args[1]);
    let renderer = Path::new(&args[2]);
    let scratch = Path::new(&args[3]);
    let source = scratch.join("source");
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
    fs::copy(renderer.join("nvi1.rs"), source.join("source/src/lib.rs")).unwrap();
    if args.len() == 5 {
        fs::create_dir(source.join("shaders")).unwrap();
        for entry in fs::read_dir(renderer.join("shaders")).unwrap() {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            fs::copy(entry.path(), source.join("shaders").join(entry.file_name())).unwrap();
        }
    }
    let archive = scratch.join("component.gamepkg");
    // Public synthetic fixture seed; never used to sign a released port.
    gamepkg::pack::pack(&source, &archive, &[173; 32]).unwrap();
    let mut package = gamepkg::archive::Package::open(&archive).unwrap();
    gamepkg::source::scan(&mut package).unwrap();
    println!(
        "PASS: actual GamePKG dependency, portability and source scanner admits NVI1 producer"
    );
    if args.len() == 5 {
        let mut entries = Vec::new();
        for mask in 0..16 {
            for (entry, stage) in [("vertexMain", "vertex"), ("fragmentMain", "fragment")] {
                entries.push(gamepkg::manifest::Shader {
                    source: format!("shaders/template_{mask:02}.slang"),
                    entry: entry.into(),
                    stage: stage.into(),
                });
            }
        }
        let bytes = fs::read(&archive).unwrap();
        for backend in ["wgsl", "spirv"] {
            let settings = gamepkg::settings::Settings {
                slang_directory: args[4].clone(),
                shader_backend: backend.into(),
                ..Default::default()
            };
            let stages = gamepkg::shaders::compile_native_sources(
                &scratch.join("cache"),
                &bytes,
                &mut package,
                &entries,
                &settings,
            )
            .unwrap();
            assert_eq!(stages.len(), 32);
            println!(
                "PASS: 32 {backend} stages through actual confined GamePKG native preparation"
            );
        }
    }
    println!("Component checks only: no Takedawn build, launcher, profile admission or gameplay qualification");
}
