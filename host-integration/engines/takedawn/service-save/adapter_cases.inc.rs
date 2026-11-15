// Included only by the maintainer harness, never by the signed game source.
fn finish_one(guard: &mut adapter::SaveGuard, grant: u64) -> (Option<i32>, bool) {
    let mut job = guard.begin_job().unwrap().unwrap();
    let adapter::Step::Complete { finished } = job.step().unwrap() else {
        panic!("unexpected history request");
    };
    let warning = (finished.history_error, finished.preservation_durable);
    let cleanup = guard.finish(finished).unwrap();
    adapter::clean_obsolete(grant, cleanup).unwrap();
    assert!(matches!(job.step(), Err(-4)));
    warning
}
fn read_service(handle: u64) -> Vec<u8> {
    let mut bytes = vec![0; gamepkg::file_size(handle).unwrap() as usize];
    let count = gamepkg::file_read(handle, 0, &mut bytes).unwrap();
    assert_eq!(count, bytes.len());
    gamepkg::file_close(handle).unwrap();
    bytes
}

#[test]
fn adapter_keeps_real_save_whole_until_history_and_worker_publication_finish() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"abcdef").unwrap();
    let (grant, _harness) = connect(root.path(), 1024 * 1024, true, false);
    assert_eq!(
        gamepkg::directory_list(grant, "").unwrap()[0].name,
        "profile"
    );
    assert!(!gamepkg::directory_list(grant, "").unwrap()[0].directory);
    gamepkg::directory_create(grant, "folder").unwrap();
    assert!(gamepkg::directory_list(grant, "")
        .unwrap()
        .iter()
        .any(|entry| entry.directory && entry.name == "folder"));
    let mut guard = adapter::SaveGuard::new(grant);
    guard.set_history(true);
    let first = guard.open_write("profile").unwrap();
    let second = guard.open_write("profile").unwrap();
    guard.write_at(first, 2, b"XY").unwrap();
    guard.close(first).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("profile")).unwrap(),
        b"abcdef"
    );
    guard.close(second).unwrap();
    assert_eq!(guard.open_read("profile"), Err(-2));
    let mut job = guard.begin_job().unwrap().unwrap();
    let adapter::Step::History { real } = job.step().unwrap() else {
        panic!("history must precede publication");
    };
    assert_eq!(real, "profile");
    assert_eq!(
        std::fs::read(root.path().join("profile")).unwrap(),
        b"abcdef"
    );
    job.history_finished(Err(-3)).unwrap();
    assert_eq!(job.history_finished(Ok(())), Err(-4));
    let adapter::Step::Complete { finished } = job.step().unwrap() else {
        panic!("expected publication");
    };
    assert_eq!(finished.history_error, Some(-3));
    assert!(finished.preservation_durable);
    adapter::clean_obsolete(grant, guard.finish(finished).unwrap()).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("profile")).unwrap(),
        b"abXYef"
    );
    assert_eq!(guard.busy(), 0);
    assert!(matches!(job.step(), Err(-4)));
}

#[test]
fn adapter_pending_read_and_partial_write_use_the_complete_newer_file() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"old!!!").unwrap();
    let (grant, harness) = connect(root.path(), 1024 * 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 0, b"new!!!").unwrap();
    guard.close(handle).unwrap();
    harness.fail_real.store(1, Ordering::SeqCst);
    finish_one(&mut guard, grant);
    assert_eq!(
        std::fs::read(root.path().join("profile")).unwrap(),
        b"old!!!"
    );
    assert_eq!(
        std::fs::read(root.path().join("profile.~pend")).unwrap(),
        b"new!!!"
    );
    assert_eq!(guard.pending_count(), 1);
    assert_eq!(read_service(guard.open_read("profile").unwrap()), b"new!!!");
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 3, b"??").unwrap();
    guard.close(handle).unwrap();
    finish_one(&mut guard, grant);
    assert_eq!(
        std::fs::read(root.path().join("profile")).unwrap(),
        b"new??!"
    );
    assert!(!root.path().join("profile.~pend").exists());
    assert!(!root.path().join("profile.~new").exists());
    assert_eq!(guard.pending_count(), 0);
}

#[test]
fn adapter_never_turns_probe_errors_into_permission_to_discard_pending() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"old").unwrap();
    let (grant, harness) = connect(root.path(), 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 0, b"new").unwrap();
    guard.close(handle).unwrap();
    harness.fail_real.store(1, Ordering::SeqCst);
    finish_one(&mut guard, grant);
    harness.fail_probe.store(1, Ordering::SeqCst);
    assert_eq!(guard.open_read("profile"), Err(-3));
    assert_eq!(guard.pending_count(), 1);
    assert_eq!(read_service(guard.open_read("profile").unwrap()), b"new");
    gamepkg::file_remove(grant, "profile.~pend").unwrap();
    assert_eq!(read_service(guard.open_read("profile").unwrap()), b"old");
    assert_eq!(guard.pending_count(), 0);
}

#[test]
fn adapter_quota_failure_foreign_handles_and_missing_new_files_preserve_containment() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"existing").unwrap();
    let (grant, _harness) = connect(root.path(), 12, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    assert_eq!(guard.open_write("profile"), Err(-2));
    assert_eq!(guard.busy(), 0);
    assert_eq!(
        std::fs::read(root.path().join("profile")).unwrap(),
        b"existing"
    );
    assert!(!root.path().join("profile.~new").exists());
    let foreign = gamepkg::file_open(grant, true, "other").unwrap();
    assert_eq!(guard.write_at(foreign, 0, b"evil"), Err(-4));
    assert_eq!(guard.resize(foreign, 4), Err(-4));
    assert_eq!(guard.close(foreign), Err(-4));
    assert_eq!(gamepkg::file_size(foreign).unwrap(), 0);
    gamepkg::file_close(foreign).unwrap();
    assert_eq!(guard.open_write("../outside"), Err(-1));
    let created = guard.open_write("new").unwrap();
    guard.resize(created, 2).unwrap();
    guard.write_at(created, 0, b"ok").unwrap();
    guard.close(created).unwrap();
    finish_one(&mut guard, grant);
    assert_eq!(std::fs::read(root.path().join("new")).unwrap(), b"ok");
}

#[test]
fn adapter_flush_errors_retry_the_same_job_without_publishing_or_leaking_handles() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"old").unwrap();
    let (grant, harness) = connect(root.path(), 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 0, b"new").unwrap();
    guard.close(handle).unwrap();
    let mut job = guard.begin_job().unwrap().unwrap();
    harness.fail_sync.store(1, Ordering::SeqCst);
    assert!(matches!(job.step(), Err(-3)));
    assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), b"old");
    assert_eq!(guard.busy(), 1);
    let adapter::Step::Complete { finished } = job.step().unwrap() else {
        panic!("expected retry success");
    };
    adapter::clean_obsolete(grant, guard.finish(finished).unwrap()).unwrap();
    assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), b"new");
    assert_eq!(guard.busy(), 0);
}

#[test]
fn adapter_unchanged_delete_and_missing_shadow_paths_do_not_publish_other_data() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"old").unwrap();
    let (grant, harness) = connect(root.path(), 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    guard.set_history(true);
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 0, b"old").unwrap();
    guard.close(handle).unwrap();
    finish_one(&mut guard, grant);
    assert_eq!(harness.publications.load(Ordering::SeqCst), 0);
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 0, b"new").unwrap();
    guard.close(handle).unwrap();
    gamepkg::file_remove(grant, "profile.~new").unwrap();
    finish_one(&mut guard, grant);
    assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), b"old");
    let handle = guard.open_write("profile").unwrap();
    guard.delete_on_close(handle).unwrap();
    guard.close(handle).unwrap();
    finish_one(&mut guard, grant);
    assert!(!root.path().join("profile").exists());
    assert_eq!(guard.busy(), 0);
}

#[test]
fn adapter_published_but_not_durable_does_not_retry_consumed_source() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"old").unwrap();
    let (grant, harness) = connect(root.path(), 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 0, b"new").unwrap();
    guard.close(handle).unwrap();
    harness.published_uncertain.store(1, Ordering::SeqCst);
    finish_one(&mut guard, grant);
    assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), b"new");
    assert!(!root.path().join("profile.~new").exists());
    assert_eq!(guard.pending_count(), 0);
    assert!(guard.problem().unwrap().durability);
    guard.exit_retry();
    assert!(guard.begin_job().unwrap().is_none());
    assert_eq!(harness.publications.load(Ordering::SeqCst), 1);
    guard.acknowledge_durability();
    assert!(guard.problem().is_none());
}

#[test]
fn adapter_failed_pending_rename_keeps_the_complete_shadow_for_the_next_open() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"old").unwrap();
    let (grant, harness) = connect(root.path(), 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 0, b"new").unwrap();
    guard.close(handle).unwrap();
    harness.fail_real.store(1, Ordering::SeqCst);
    harness.fail_pending.store(1, Ordering::SeqCst);
    finish_one(&mut guard, grant);
    assert_eq!(read_service(guard.open_read("profile").unwrap()), b"new");
    assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), b"old");
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 2, b"!").unwrap();
    guard.close(handle).unwrap();
    finish_one(&mut guard, grant);
    assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), b"ne!");
    assert_eq!(guard.pending_count(), 0);
}

#[test]
fn adapter_tracks_a_pending_rename_that_published_without_proven_durability() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("profile"), b"old").unwrap();
    let (grant, harness) = connect(root.path(), 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let handle = guard.open_write("profile").unwrap();
    guard.write_at(handle, 0, b"new").unwrap();
    guard.close(handle).unwrap();
    harness.fail_real.store(1, Ordering::SeqCst);
    harness.published_uncertain.store(1, Ordering::SeqCst);
    assert_eq!(finish_one(&mut guard, grant), (None, false));
    assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), b"old");
    assert!(!root.path().join("profile.~new").exists());
    assert_eq!(read_service(guard.open_read("profile").unwrap()), b"new");
    guard.exit_retry();
    finish_one(&mut guard, grant);
    assert_eq!(std::fs::read(root.path().join("profile")).unwrap(), b"new");
    assert_eq!(guard.pending_count(), 0);
    assert_eq!(harness.publications.load(Ordering::SeqCst), 3);
}
