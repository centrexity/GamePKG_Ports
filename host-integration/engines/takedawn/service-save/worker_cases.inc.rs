// Maintainer harness only. Actual production worker source is checked separately.
fn worker_setup(root:&std::path::Path) {
    std::fs::create_dir_all(root.join("UDATA/12345678")).unwrap();
    std::fs::create_dir_all(root.join("TDATA/12345678")).unwrap();
    std::fs::write(root.join("UDATA/12345678/profile"),b"old-save").unwrap();
    std::fs::write(root.join("TDATA/12345678/settings"),b"settings").unwrap();
}
fn worker_open(actor:&mut worker::SaveWorker)->u64 {
    let bytes=actor.command(worker::OPEN_WRITE,b"UDATA/12345678/profile").unwrap();
    u64::from_le_bytes(bytes.try_into().unwrap())
}
fn worker_write(actor:&mut worker::SaveWorker,handle:u64) {
    let mut payload=[handle.to_le_bytes(),0u64.to_le_bytes()].concat();
    payload.extend(b"new-save");
    actor.command(worker::WRITE,&payload).unwrap();
}
fn worker_drain(actor:&mut worker::SaveWorker) {
    for _ in 0..64 {if !actor.tick().unwrap(){return;}}
    panic!("worker did not quiesce");
}
#[test]
fn save_worker_close_ack_precedes_flush_history_and_publication() {
    let root=tempfile::tempdir().unwrap();worker_setup(root.path());
    let (grant,_harness)=connect(root.path(),1048576,false,false);
    let mut actor=worker::SaveWorker::new(grant,"12345678","20261114-120000").unwrap();
    let handle=worker_open(&mut actor); worker_write(&mut actor,handle);
    assert_eq!(actor.command(worker::CLOSE,&handle.to_le_bytes()).unwrap(),[0u8;0]);
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"old-save");
    assert!(!root.path().join("save-history").exists());
    assert!(actor.tick().unwrap());
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"old-save");
    assert_eq!(actor.command(worker::STAMP,b"20261114-120001"),Err(-2));
    worker_drain(&mut actor);
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"new-save");
    let exported=root.path().join("save-history/20261114-120000");
    assert_eq!(std::fs::read(exported.join("UDATA/12345678/profile")).unwrap(),b"old-save");
    assert_eq!(std::fs::read(exported.join("TDATA/12345678/settings")).unwrap(),b"settings");
    assert!(history::archive_complete(grant,"save-history/20261114-120000","12345678").unwrap());
    assert_eq!(actor.command(worker::STAMP,b"20261114-120001").unwrap(),[0u8;0]);
    assert_eq!(actor.status_bytes().len(),68);
    assert_eq!(&actor.status_bytes()[..16],&[0;16]);
}
#[test]
fn save_worker_retains_exact_job_on_flush_failure_without_reopening_or_publishing() {
    let root=tempfile::tempdir().unwrap();worker_setup(root.path());
    let (grant,harness)=connect(root.path(),1048576,false,false);
    let mut actor=worker::SaveWorker::new(grant,"12345678","20261114-120000").unwrap();
    let handle=worker_open(&mut actor);worker_write(&mut actor,handle);
    actor.command(worker::CLOSE,&handle.to_le_bytes()).unwrap();
    harness.fail_sync.store(1,Ordering::SeqCst);
    assert_eq!(actor.tick(),Err(-3));
    assert!(actor.has_work());
    assert_eq!(harness.publications.load(Ordering::SeqCst),0);
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"old-save");
    worker_drain(&mut actor);
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"new-save");
}
#[test]
fn save_worker_rejects_foreign_paths_bad_messages_handles_and_limits_before_mutation() {
    let root=tempfile::tempdir().unwrap();worker_setup(root.path());
    let (grant,_harness)=connect(root.path(),1048576,false,false);
    assert!(matches!(worker::SaveWorker::new(grant+1,"12345678","20261114-120000"),Err(-4)));
    assert!(matches!(worker::SaveWorker::new(grant,"12345678","bad"),Err(-1)));
    let mut actor=worker::SaveWorker::new(grant,"12345678","20261114-120000").unwrap();
    for path in ["UDATA/87654321/profile","UDATA/12345678/../escape","../UDATA/12345678/profile","root-settings","TDATA/12345678/profile.~pend"] {
        assert!(actor.command(worker::OPEN_WRITE,path.as_bytes()).is_err(),"{path}");
    }
    for (operation,payload) in [(worker::WRITE,&[0u8;15][..]),(worker::CLOSE,&[0u8;9][..]),(worker::RESIZE,&[0u8;17][..]),(worker::DELETE_ON_CLOSE,&[0u8;7][..]),(worker::OPEN_READ,&[255u8][..]),(worker::STATUS,&[1u8][..]),(worker::STAMP,b"20261114/120000"),(0,&[])] {
        assert!(actor.command(operation,payload).is_err());
    }
    assert_eq!(actor.command(worker::CLOSE,&999u64.to_le_bytes()),Err(-4));
    let mut readers=Vec::new();
    for _ in 0..64 { readers.push(actor.command(worker::OPEN_READ,b"UDATA/12345678/profile").unwrap()); }
    assert_eq!(actor.command(worker::OPEN_READ,b"UDATA/12345678/profile"),Err(-2));
    for bytes in readers { actor.command(worker::CLOSE,&bytes).unwrap(); }
    let mut writers=Vec::new();
    for _ in 0..4 {writers.push(worker_open(&mut actor));}
    assert_eq!(actor.command(worker::OPEN_WRITE,b"UDATA/12345678/profile"),Err(-2));
    for handle in writers {actor.command(worker::CLOSE,&handle.to_le_bytes()).unwrap();}
    worker_drain(&mut actor);
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"old-save");
}
#[test]
fn save_worker_history_failure_is_reported_without_silently_preventing_title_publication() {
    let root=tempfile::tempdir().unwrap();worker_setup(root.path());
    // Existing 16 bytes plus an 8-byte shadow fit; a title snapshot does not.
    let (grant,_harness)=connect(root.path(),24,false,false);
    let mut actor=worker::SaveWorker::new(grant,"12345678","20261114-120000").unwrap();
    let handle=worker_open(&mut actor);worker_write(&mut actor,handle);
    actor.command(worker::CLOSE,&handle.to_le_bytes()).unwrap();
    worker_drain(&mut actor);
    assert_ne!(actor.history_error,0);
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"new-save");
    assert!(!root.path().join("save-history/20261114-120000/save-export.txt").exists());
}
#[test]
fn save_worker_failed_publication_keeps_pending_read_redirection_and_warns() {
    let root=tempfile::tempdir().unwrap();worker_setup(root.path());
    let (grant,harness)=connect(root.path(),1048576,false,false);
    let mut actor=worker::SaveWorker::new(grant,"12345678","20261114-120000").unwrap();
    let handle=worker_open(&mut actor);worker_write(&mut actor,handle);
    actor.command(worker::CLOSE,&handle.to_le_bytes()).unwrap();
    harness.fail_real.store(20,Ordering::SeqCst);
    worker_drain(&mut actor);
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"old-save");
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile.~pend")).unwrap(),b"new-save");
    let reader=actor.command(worker::OPEN_READ,b"UDATA/12345678/profile").unwrap();
    let id=u64::from_le_bytes(reader.clone().try_into().unwrap());
    let mut contents=[0;8]; assert_eq!(gamepkg::file_read(id,0,&mut contents),Ok(8));
    assert_eq!(&contents,b"new-save");actor.command(worker::CLOSE,&reader).unwrap();
    assert_ne!(&actor.status_bytes()[4..8],&[0;4]);
    // The original guard warns at the fourth failed attempt, not on the first
    // pending record. Additional title close events trigger that retry policy.
    assert_eq!(&actor.status_bytes()[16..24],&[0;8]);
    for _ in 0..2 {
        let handle=worker_open(&mut actor);worker_write(&mut actor,handle);
        actor.command(worker::CLOSE,&handle.to_le_bytes()).unwrap();worker_drain(&mut actor);
    }
    assert_ne!(&actor.status_bytes()[16..24],&[0;8]);
}
#[test]
fn save_worker_recovers_startup_before_accepting_title_io() {
    let root=tempfile::tempdir().unwrap();worker_setup(root.path());
    std::fs::write(root.path().join("UDATA/12345678/profile.~new"),b"unfinished").unwrap();
    let (grant,_harness)=connect(root.path(),1048576,false,false);
    let mut actor=worker::SaveWorker::new(grant,"12345678","20261114-120000").unwrap();
    assert_eq!(actor.recovery_report.unfinished_removed,1);
    assert!(!root.path().join("UDATA/12345678/profile.~new").exists());
    let reader=actor.command(worker::OPEN_READ,b"UDATA/12345678/profile").unwrap();
    actor.command(worker::CLOSE,&reader).unwrap();
    assert_eq!(actor.status_bytes().len(),68);
}
#[test]
fn save_worker_durability_uncertainty_is_reported_without_retrying_consumed_shadow() {
    let root=tempfile::tempdir().unwrap();worker_setup(root.path());
    let (grant,harness)=connect(root.path(),1048576,false,false);
    let mut actor=worker::SaveWorker::new(grant,"12345678","20261114-120000").unwrap();
    let handle=worker_open(&mut actor);worker_write(&mut actor,handle);
    actor.command(worker::CLOSE,&handle.to_le_bytes()).unwrap();
    assert!(actor.tick().unwrap()); // History marker has been published normally.
    harness.published_uncertain.store(1,Ordering::SeqCst);
    worker_drain(&mut actor);
    assert_eq!(&actor.status_bytes()[..8],&[0;8]);
    assert_eq!(&actor.status_bytes()[28..32],&[1,0,0,0]);
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),b"new-save");
    assert!(!root.path().join("UDATA/12345678/profile.~new").exists());
    assert!(!root.path().join("UDATA/12345678/profile.~pend").exists());
    assert_eq!(harness.publications.load(Ordering::SeqCst),2);
}

#[test]
fn guarded_regular_open_reports_disposition_and_write_through_errors() {
    let root = tempfile::tempdir().unwrap();
    worker_setup(root.path());
    let (grant, harness) = connect(root.path(), 1048576, false, false);
    let mut actor = worker::SaveWorker::new(grant, "12345678", "20261114-120000").unwrap();
    let profile = "UDATA/12345678/profile";
    let missing = "UDATA/12345678/absent";
    let open = |actor: &mut worker::SaveWorker, flags: u32, disposition: u32, name: &str| {
        let payload = [flags.to_le_bytes().as_slice(), &disposition.to_le_bytes(), name.as_bytes()].concat();
        actor.command(worker::OPEN_FILE, &payload).map(|reply| {
            assert_eq!(reply.len(), 12);
            (u64::from_le_bytes(reply[..8].try_into().unwrap()), u32::from_le_bytes(reply[8..].try_into().unwrap()))
        })
    };
    for (flags, disposition) in [(4, 1), (2, 1), (1, 4), (0, 3), (3, 0)] {
        assert_eq!(open(&mut actor, flags, disposition, profile), Err(-1));
    }
    assert_eq!(open(&mut actor, 3, 1, missing), Ok((0, 5)));
    assert_eq!(open(&mut actor, 3, 2, profile), Ok((0, 4)));
    assert!(!root.path().join("UDATA/12345678/profile.~new").exists());

    harness.fail_sync.store(1, Ordering::SeqCst);
    assert_eq!(open(&mut actor, 3, 3, profile), Err(-3));
    assert!(!root.path().join("UDATA/12345678/profile.~new").exists());
    let (handle, information) = open(&mut actor, 3, 3, profile).unwrap();
    assert_eq!(information, 1);
    harness.fail_sync.store(1, Ordering::SeqCst);
    let written = [handle.to_le_bytes().as_slice(), &0u64.to_le_bytes(), b"written!"].concat();
    assert_eq!(actor.command(worker::WRITE, &written), Err(-3));
    assert_eq!(actor.io_error, -3);
    assert_eq!(std::fs::read(root.path().join(profile)).unwrap(), b"old-save");
    assert_eq!(std::fs::read(root.path().join("UDATA/12345678/profile.~new")).unwrap(), b"written!");
    actor.command(worker::CLOSE, &handle.to_le_bytes()).unwrap();
    worker_drain(&mut actor);
    assert_eq!(std::fs::read(root.path().join(profile)).unwrap(), b"written!");
    assert_eq!(actor.io_error, -3);
}

#[test]
fn worker_queries_exact_opened_file_and_preserves_created_attributes_across_replacement() {
    let root = tempfile::tempdir().unwrap();
    worker_setup(root.path());
    let (grant, _harness) = connect(root.path(), 1048576, false, false);
    let mut actor = worker::SaveWorker::new(grant, "12345678", "20261114-120000").unwrap();
    let path = "UDATA/12345678/TitleMeta.xbx";
    let open_file = |actor: &mut worker::SaveWorker, flags: u32, disposition: u32| {
        let body = [flags.to_le_bytes().as_slice(), &disposition.to_le_bytes(), path.as_bytes()].concat();
        let reply = actor.command(worker::OPEN_FILE, &body).unwrap();
        (u64::from_le_bytes(reply[..8].try_into().unwrap()), u32::from_le_bytes(reply[8..].try_into().unwrap()))
    };
    let verify_info = |actor: &mut worker::SaveWorker, id: u64, length: u64| {
        let body = actor.command(worker::QUERY_FILE, &id.to_le_bytes()).unwrap();
        assert_eq!(body.len(), 52);
        for timestamp in body[..32].chunks_exact(8) {
            assert!(u64::from_le_bytes(timestamp.try_into().unwrap()) > 116_444_736_000_000_000);
        }
        assert_eq!(&body[32..40], &if length == 0 { 0u64 } else { 16384u64 }.to_le_bytes());
        assert_eq!(&body[40..48], &length.to_le_bytes());
        assert_eq!(&body[48..], &0x24u32.to_le_bytes());
    };

    let escaped = gamepkg::file_create_new(grant, "unregistered").unwrap();
    assert_eq!(actor.command(worker::QUERY_FILE, &escaped.to_le_bytes()), Err(-4));
    assert_eq!(actor.command(worker::QUERY_FILE, &[0; 9]), Err(-1));
    gamepkg::file_close(escaped).unwrap();

    let (file, information) = open_file(&mut actor, 0x403, 3);
    assert_eq!(information, 2);
    verify_info(&mut actor, file, 0);
    assert!(!root.path().join(path).exists());
    let body = [file.to_le_bytes().as_slice(), &0u64.to_le_bytes(), b"new"].concat();
    actor.command(worker::WRITE, &body).unwrap();
    actor.command(worker::CLOSE, &file.to_le_bytes()).unwrap();
    worker_drain(&mut actor);
    assert_eq!(actor.command(worker::QUERY_FILE, &file.to_le_bytes()), Err(-4));
    let (reader, result) = open_file(&mut actor, 0, 1);
    assert_eq!(result, 1);
    verify_info(&mut actor, reader, 3);
    actor.command(worker::CLOSE, &reader.to_le_bytes()).unwrap();

    let (existing, result) = open_file(&mut actor, 0x203, 3);
    assert_eq!(result, 1);
    verify_info(&mut actor, existing, 3);
    let update = [existing.to_le_bytes().as_slice(), &3u64.to_le_bytes(), b"!"].concat();
    actor.command(worker::WRITE, &update).unwrap();
    actor.command(worker::CLOSE, &existing.to_le_bytes()).unwrap();
    worker_drain(&mut actor);
    let (final_reader, _) = open_file(&mut actor, 0, 1);
    verify_info(&mut actor, final_reader, 4);
    actor.command(worker::CLOSE, &final_reader.to_le_bytes()).unwrap();
    assert_eq!(std::fs::read(root.path().join(path)).unwrap(), b"new!");
}
