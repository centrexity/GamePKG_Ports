//! Maintainer-only: use the real app policy and audited native preparation.
//! A temporary test signature is never a production port credential.
use std::{fs, path::Path};
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert_eq!(args.len(), 4);
    let app = Path::new(&args[1]);
    let here = Path::new(&args[2]);
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
    let mut game = fs::read_to_string(here.join("pad.rs")).unwrap();
    game.push_str("\npub fn game_main(){let mut pads=Pads::new();let _=pads.read(0,false);}\n");
    fs::write(source.join("source/src/lib.rs"), &game).unwrap();
    let archive = scratch.join("pad.gamepkg");
    gamepkg::pack::pack(&source, &archive, &[173; 32]).unwrap();
    let sources = gamepkg::port_host::admission::CheckedSources::verify(
        &mut gamepkg::archive::Package::open(&archive).unwrap(),
    )
    .unwrap();
    use gamepkg::port_host::preparation::{self, LocalLayout, Plan, Target};
    for target in [
        Target::Linux64,
        Target::Linux32,
        Target::Windows64,
        Target::Windows32,
    ] {
        let plan = Plan::new(target, "game_main", vec![], LocalLayout::None, true).unwrap();
        let artifact = preparation::build(scratch, &sources, &plan, vec![], vec![]).unwrap();
        artifact.executable().unwrap();
        println!("PASS {target:?}: exact production pad source, checked before host bindings, optimized and object-audited before trusted link");
    }
    for hostile in [
        "fn unused(){let held=|x:u32|x;held(1);}",
        "#[derive(Clone)] struct Hidden {value:u32}",
        "fn unused(){std::fs::read(\"secret\");}",
        "type Hidden=u32;",
    ] {
        fs::write(
            source.join("source/src/lib.rs"),
            format!("{game}\n{hostile}"),
        )
        .unwrap();
        assert!(
            gamepkg::pack::pack(&source, &scratch.join("rejected.gamepkg"), &[173; 32]).is_err(),
            "unexpected policy bypass: {hostile}"
        );
    }
    println!("PASS source negatives: unused closures, derive macros, filesystem calls and aliases remain rejected");
    println!("Component admission only: no Burnout gameplay, physical controller or rumble qualification");
}
