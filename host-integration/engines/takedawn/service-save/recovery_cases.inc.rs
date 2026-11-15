const TITLE: &str = "12345678";
const STAMP: &str = "20261114-120000";
fn save_layout(root: &std::path::Path, real: &[u8], pending: Option<&[u8]>, newer: bool) {
    let folder = root.join("UDATA").join(TITLE);
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("profile"), real).unwrap();
    let set = |path: &std::path::Path, seconds| {
        std::fs::File::open(path)
            .unwrap()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
            .unwrap()
    };
    set(&folder.join("profile"), 20);
    if let Some(bytes) = pending {
        std::fs::write(folder.join("profile.~pend"), bytes).unwrap();
        set(&folder.join("profile.~pend"), if newer { 30 } else { 10 });
    }
}
fn until_recovered(startup: &mut recovery::Recovery<'_>) -> recovery::Report {
    loop {
        match startup.step().unwrap() {
            recovery::RecoveryStep::Progress {} => {}
            recovery::RecoveryStep::Done { report } => return report,
            recovery::RecoveryStep::History { .. } => panic!("unexpected history request"),
        }
    }
}

#[test]
fn recovery_discards_unfinished_shadow_and_preserves_real_and_other_titles() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"whole", None, false);
    std::fs::write(root.path().join("UDATA/12345678/profile.~new"), b"partial").unwrap();
    std::fs::create_dir_all(root.path().join("UDATA/87654321")).unwrap();
    std::fs::write(root.path().join("UDATA/87654321/other.~new"), b"unrelated").unwrap();
    let (grant, _h) = connect(root.path(), 1024 * 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let mut startup = recovery::Recovery::begin(&mut guard, grant, TITLE).unwrap();
    let report = until_recovered(&mut startup);
    assert_eq!(report.unfinished_removed, 1);
    assert_eq!(report.recovered, 0);
    assert_eq!(
        std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),
        b"whole"
    );
    assert!(root.path().join("UDATA/87654321/other.~new").exists());
    assert!(!root.path().join("UDATA/12345678/profile.~new").exists());
}

#[test]
fn recovery_publishes_newer_complete_pending_and_reports_consumed_source() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"old", Some(b"new"), true);
    let (grant, h) = connect(root.path(), 1024 * 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    h.published_uncertain.store(1, Ordering::SeqCst);
    let mut startup = recovery::Recovery::begin(&mut guard, grant, TITLE).unwrap();
    let report = until_recovered(&mut startup);
    assert_eq!((report.recovered, report.uncertain), (1, 1));
    assert_eq!(startup.summary().uncertain, 1);
    drop(startup);
    assert!(guard.problem().unwrap().durability);
    assert_eq!(guard.pending_count(), 0);
    assert_eq!(
        std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),
        b"new"
    );
    assert!(!root.path().join("UDATA/12345678/profile.~pend").exists());
    assert_eq!(h.publications.load(Ordering::SeqCst), 1);
}

#[test]
fn older_pending_is_discarded_only_after_complete_title_snapshot_with_pending_override() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"current", Some(b"older"), false);
    std::fs::create_dir_all(root.path().join("TDATA/12345678/nested")).unwrap();
    std::fs::write(root.path().join("TDATA/12345678/nested/config"), b"config").unwrap();
    std::fs::write(root.path().join("UDATA/12345678/another"), b"other slot").unwrap();
    std::fs::write(
        root.path().join("UDATA/12345678/another.~new"),
        b"unfinished",
    )
    .unwrap();
    std::fs::create_dir_all(root.path().join("UDATA/87654321")).unwrap();
    std::fs::write(root.path().join("UDATA/87654321/private"), b"other title").unwrap();
    std::fs::write(root.path().join("settings"), b"not save data").unwrap();
    let (grant, _h) = connect(root.path(), 1024 * 1024, true, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let mut startup = recovery::Recovery::begin(&mut guard, grant, TITLE).unwrap();
    let (real, pending) = loop {
        match startup.step().unwrap() {
            recovery::RecoveryStep::Progress {} => {}
            recovery::RecoveryStep::History {
                real,
                pending,
                newer,
                error,
            } => {
                assert!(!newer);
                assert_eq!(error, -3);
                break (real, pending);
            }
            recovery::RecoveryStep::Done { .. } => panic!("missing history"),
        }
    };
    let proof = history::archive_pending(grant, TITLE, STAMP, &real, &pending).unwrap();
    let folder = root.path().join(proof.archive_root());
    assert!(proof.valid_for(grant, &real, &pending));
    assert!(!proof.valid_for(grant + 1, &real, &pending));
    assert_eq!(
        std::fs::read(folder.join("UDATA/12345678/profile")).unwrap(),
        b"older"
    );
    assert_eq!(
        std::fs::read(folder.join("UDATA/12345678/another")).unwrap(),
        b"other slot"
    );
    assert_eq!(
        std::fs::read(folder.join("TDATA/12345678/nested/config")).unwrap(),
        b"config"
    );
    assert!(!folder.join("UDATA/12345678/profile.~pend").exists());
    assert!(!folder.join("UDATA/12345678/another.~new").exists());
    assert!(!folder.join("UDATA/87654321").exists());
    assert!(!folder.join("settings").exists());
    assert!(history::archive_complete(grant, proof.archive_root(), TITLE).unwrap());
    assert!(!history::archive_complete(grant, proof.archive_root(), "87654321").unwrap());
    startup.history_done(&proof).unwrap();
    let report = until_recovered(&mut startup);
    assert_eq!(report.archived, 1);
    assert_eq!(report.uncertain, 0);
    assert!(!root.path().join(&pending).exists());
    assert_eq!(std::fs::read(root.path().join(&real)).unwrap(), b"current");
}

#[test]
fn failed_history_copy_cleans_partial_export_and_keeps_older_pending() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"current", Some(b"older"), false);
    let (grant, _h) = connect(root.path(), 16, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let mut startup = recovery::Recovery::begin(&mut guard, grant, TITLE).unwrap();
    let recovery::RecoveryStep::History { real, pending, .. } = startup.step().unwrap() else {
        panic!("history required")
    };
    let error = history::archive_pending(grant, TITLE, STAMP, &real, &pending)
        .err()
        .unwrap();
    assert_eq!(error, -2);
    assert!(gamepkg::directory_list(grant, "save-history")
        .unwrap()
        .is_empty());
    startup.history_failed(error).unwrap();
    let report = until_recovered(&mut startup);
    assert_eq!((report.older_kept, report.archive_errors), (1, 1));
    assert_eq!(std::fs::read(root.path().join(&pending)).unwrap(), b"older");
    assert_eq!(std::fs::read(root.path().join(&real)).unwrap(), b"current");
}

#[test]
fn blocked_newer_pending_survives_history_failure_and_retries_from_its_actual_path() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"old", Some(b"new"), true);
    let (grant, h) = connect(root.path(), 1024 * 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    h.fail_real.store(1, Ordering::SeqCst);
    let mut startup = recovery::Recovery::begin(&mut guard, grant, TITLE).unwrap();
    let recovery::RecoveryStep::History { newer, error, .. } = startup.step().unwrap() else {
        panic!("history requested")
    };
    assert!(newer);
    assert_eq!(error, -3);
    startup.history_failed(-2).unwrap();
    let report = until_recovered(&mut startup);
    assert_eq!((report.pending, report.archive_errors), (1, 1));
    drop(startup);
    assert_eq!(guard.pending_count(), 1);
    assert!(guard.problem().is_some());
    assert_eq!(
        read_service(guard.open_read("UDATA/12345678/profile").unwrap()),
        b"new"
    );
    guard.exit_retry();
    finish_one(&mut guard, grant);
    assert_eq!(guard.pending_count(), 0);
    assert_eq!(
        std::fs::read(root.path().join("UDATA/12345678/profile")).unwrap(),
        b"new"
    );
}

#[test]
fn lookup_errors_keep_the_same_recovery_candidate_and_require_a_matching_snapshot_proof() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"current", Some(b"older"), false);
    let (grant, h) = connect(root.path(), 1024 * 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let mut startup = recovery::Recovery::begin(&mut guard, grant, TITLE).unwrap();
    h.fail_probe.store(1, Ordering::SeqCst);
    assert!(matches!(startup.step(), Err(-3)));
    let recovery::RecoveryStep::History { real, pending, .. } = startup.step().unwrap() else {
        panic!("same candidate must remain")
    };
    let unrelated = history::archive_before_replace(grant, TITLE, STAMP).unwrap();
    assert_eq!(startup.history_done(&unrelated), Err(-4));
    assert!(root.path().join(&pending).exists());
    let proof = history::archive_pending(grant, TITLE, STAMP, &real, &pending).unwrap();
    assert!(proof.archive_root().ends_with("-2"));
    startup.history_done(&proof).unwrap();
    assert_eq!(until_recovered(&mut startup).archived, 1);
}

#[test]
fn incomplete_history_has_no_completion_marker_and_existing_exports_are_never_overwritten() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"current", None, false);
    let partial = root.path().join("save-history").join(STAMP);
    std::fs::create_dir_all(partial.join("UDATA/12345678")).unwrap();
    std::fs::write(partial.join("UDATA/12345678/profile"), b"partial").unwrap();
    let (grant, _h) = connect(root.path(), 1024 * 1024, false, false);
    assert!(!history::archive_complete(grant, &format!("save-history/{STAMP}"), TITLE).unwrap());
    let proof = history::archive_before_replace(grant, TITLE, STAMP).unwrap();
    assert!(proof.archive_root().ends_with("-2"));
    assert!(history::archive_complete(grant, proof.archive_root(), TITLE).unwrap());
    assert_eq!(
        std::fs::read(partial.join("UDATA/12345678/profile")).unwrap(),
        b"partial"
    );
    assert!(history::archive_before_replace(grant, "../oops", STAMP).is_err());
    assert!(history::archive_before_replace(grant, TITLE, "../bad").is_err());
}

#[test]
fn recovery_rejects_wrong_grants_and_any_active_title_handles_before_mutation() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"current", Some(b"older"), false);
    let (grant, _h) = connect(root.path(), 1024 * 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    assert!(matches!(
        recovery::Recovery::begin(&mut guard, grant + 1, TITLE),
        Err(-4)
    ));
    let handle = guard.open_write("UDATA/12345678/profile").unwrap();
    assert!(matches!(
        recovery::Recovery::begin(&mut guard, grant, TITLE),
        Err(-2)
    ));
    guard.close(handle).unwrap();
    assert!(root.path().join("UDATA/12345678/profile.~pend").exists());
}

#[test]
fn an_archive_changed_after_creation_cannot_authorize_discarding_pending_data() {
    let root = tempfile::tempdir().unwrap();
    save_layout(root.path(), b"current", Some(b"older"), false);
    let (grant, _h) = connect(root.path(), 1024 * 1024, false, false);
    let mut guard = adapter::SaveGuard::new(grant);
    let mut startup = recovery::Recovery::begin(&mut guard, grant, TITLE).unwrap();
    let recovery::RecoveryStep::History { real, pending, .. } = startup.step().unwrap() else {
        panic!("history required")
    };
    let proof = history::archive_pending(grant, TITLE, STAMP, &real, &pending).unwrap();
    let target = history::history_child(proof.archive_root(), &real).unwrap();
    let handle = gamepkg::file_open(grant, true, &target).unwrap();
    gamepkg::file_write(handle, 0, b"break").unwrap();
    gamepkg::file_close(handle).unwrap();
    assert_eq!(startup.history_done(&proof), Err(-3));
    assert_eq!(std::fs::read(root.path().join(&pending)).unwrap(), b"older");
    startup.history_failed(-3).unwrap();
    assert_eq!(until_recovered(&mut startup).older_kept, 1);
}
