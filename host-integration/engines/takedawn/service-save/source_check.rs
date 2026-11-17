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
    for module in [
        "paths.rs",
        "files.rs",
        "state.rs",
        "adapter.rs",
        "history.rs",
        "recovery.rs",
        "worker.rs",
    ] {
        let staged = if module == "worker_fixture.rs" {
            module.to_owned()
        } else {
            format!("save_{module}")
        };
        fs::copy(here.join(module), source.join("source/src").join(staged)).unwrap();
    }
    let game = String::from(
        r#"
mod save_paths;
mod save_files;
mod save_state;
mod save_adapter;
mod save_history;
mod save_recovery;
mod save_worker;
pub fn save_worker_entry(argument:u64)->u64 { save_worker::run(argument) }
fn accept_finished(guard:&mut save_adapter::SaveGuard, completed:save_adapter::Finished){
    if let Ok(cleanup)=guard.finish(completed){let _=save_adapter::clean_obsolete(1,cleanup);}
}
pub fn game_main(){
    if let Ok(mut connection)=save_worker::Connection::launch_save_actor(0,1,"12345678","20261114-120000") {
        let _=connection.poll(0);
        let _=connection.submit(save_worker::STATUS,&[]);
        let _=connection.poll(0);
        let _=connection.cancel_save_actor();
        let _=connection.finish();
    }
    let _=gamepkg::directory_list(1,"");
    let mut guard=save_adapter::SaveGuard::new(1);
    if let Ok(mut startup)=save_recovery::Recovery::begin(&mut guard,1,"12345678"){
        if let Ok(save_recovery::RecoveryStep::History{real,pending,newer,error})=startup.step(){
            let _=newer;let _=error;
            match save_history::archive_pending(1,"12345678","20261114-120000",&real,&pending){
                Ok(proof)=>{let _=startup.history_done(&proof);},
                Err(code)=>{let _=startup.history_failed(code);},
            }
        }
    }
    guard.set_history(true);
    if let Ok(handle)=guard.open_write("save") {
        let _=guard.resize(handle,1);
        let _=guard.write_at(handle,0,b"x");
        let _=guard.save_guard_close(handle);
    }
    if let Ok(Some(mut job))=guard.begin_job(){
        match job.step(){
            Ok(save_adapter::Step::History{real})=>{
                let _=real;
                let _=job.history_finished(Ok(()));
                if let Ok(save_adapter::Step::Complete{finished})=job.step(){accept_finished(&mut guard,finished);}
            },
            Ok(save_adapter::Step::Complete{finished})=>accept_finished(&mut guard,finished),
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
        let plan = Plan::new(
            target,
            "game_main",
            vec!["save_worker_entry".into()],
            LocalLayout::None,
            true,
        )
        .unwrap();
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
