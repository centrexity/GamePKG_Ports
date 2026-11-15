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
    for module in ["files.rs", "state.rs", "adapter.rs"] {
        fs::copy(here.join(module), source.join("source/src").join(module)).unwrap();
    }
    let game = String::from(
        r#"
mod files;
mod state;
mod adapter;
fn accept_finished(guard:&mut adapter::SaveGuard, completed:adapter::Finished){
    if let Ok(cleanup)=guard.finish(completed){let _=adapter::clean_obsolete(1,cleanup);}
}
pub fn game_main(){
    let _=gamepkg::directory_list(1,"");
    let mut guard=adapter::SaveGuard::new(1);
    guard.set_history(true);
    if let Ok(handle)=guard.open_write("save") {
        let _=guard.resize(handle,1);
        let _=guard.write_at(handle,0,b"x");
        let _=guard.close(handle);
    }
    if let Ok(Some(mut job))=guard.begin_job(){
        match job.step(){
            Ok(adapter::Step::History{real})=>{
                let _=real;
                let _=job.history_finished(Ok(()));
                if let Ok(adapter::Step::Complete{finished})=job.step(){accept_finished(&mut guard,finished);}
            },
            Ok(adapter::Step::Complete{finished})=>accept_finished(&mut guard,finished),
            Err(_)=>{},
        }
    }
}
"#,
    );
    fs::write(source.join("source/src/lib.rs"), &game).unwrap();
    let archive = scratch.join("save.gamepkg");
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
        println!("PASS {target:?}: exact production save I/O and transition source, checked before host bindings, optimized and object-audited before trusted link");
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
    println!("Component admission only: no complete saveguard, Burnout gameplay or physical-device qualification");
}
