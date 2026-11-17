#[path = "paths.rs"]
mod save_paths;
// Maintainer transition tests. No test attributes/macros enter game source.
#[path = "state.rs"]
mod save_state;
use save_state::{Completion, Guard, Work};

fn open(guard: &mut Guard, real: &str, value: u64) {
    let plan = guard.prepare_write(real).unwrap();
    guard.save_handle_opened(&plan.ticket, Some(value)).unwrap();
}
fn queue(guard: &mut Guard, real: &str, value: u64) {
    open(guard, real, value);
    guard.wrote(value, 9).unwrap();
    guard.save_handle_closed(value, Some(value + 1)).unwrap();
}
fn work(guard: &mut Guard) -> Work {
    guard.next_work().unwrap().unwrap()
}
fn published(guard: &mut Guard, job: Work) {
    guard
        .complete(job.token, Completion::Published { durable: true })
        .unwrap();
}
fn failed(guard: &mut Guard, job: Work) {
    let preserved = if job.retry {
        job.save_shadow_path
    } else {
        format!("{}.~pend", job.real)
    };
    guard
        .complete(
            job.token,
            Completion::Failed {
                code: -3,
                preserved,
            },
        )
        .unwrap();
}
fn trigger(guard: &mut Guard, value: u64) {
    queue(guard, "other", value);
    let job = work(guard);
    assert!(!job.retry);
    published(guard, job);
}

#[test]
fn shared_shadow_last_close_queue_and_commit_preserve_the_original_transition_order() {
    let mut guard = Guard::new();
    guard.set_history(true);
    assert_eq!(guard.opened_write_path("UDATA/profile"), Ok(None));
    let first = guard.prepare_write("UDATA/profile").unwrap();
    assert_eq!(guard.opened_write_path("UDATA/profile"), Err(-2));
    assert_eq!(first.seed.as_deref(), Some("UDATA/profile"));
    assert_eq!(first.save_shadow_path, "UDATA/profile.~new");
    assert!(!first.preserve_shadow);
    assert_eq!(guard.busy(), 1);
    guard.save_handle_opened(&first.ticket, Some(10)).unwrap();
    assert_eq!(
        guard.opened_write_path("UDATA/profile").unwrap().as_deref(),
        Some("UDATA/profile.~new")
    );
    assert_eq!(guard.busy(), 0);
    let second = guard.prepare_write("UDATA/profile").unwrap();
    assert!(second.seed.is_none());
    guard.save_handle_opened(&second.ticket, Some(11)).unwrap();
    guard.wrote(10, 123).unwrap();
    guard.wrote(11, 4).unwrap();
    assert_eq!(guard.last_handle(10), Ok(false));
    assert_eq!(guard.read_path("UDATA/profile").unwrap(), "UDATA/profile");
    guard.save_handle_closed(10, None).unwrap();
    assert_eq!(guard.last_handle(11), Ok(true));
    guard.save_handle_closed(11, Some(12)).unwrap();
    assert_eq!(guard.read_path("UDATA/profile"), Err(-2));
    assert_eq!(guard.opened_write_path("UDATA/profile"), Err(-2));
    assert!(matches!(guard.prepare_write("UDATA/profile"), Err(-2)));
    let job = work(&mut guard);
    assert!(!job.retry && !job.deleted && job.history);
    assert_eq!((job.flush, job.writes, job.bytes), (Some(12), 2, 127));
    assert!(matches!(guard.next_work(), Err(-2)));
    assert_eq!(guard.busy(), 1);
    published(&mut guard, job);
    assert_eq!(guard.busy(), 0);
    assert_eq!(guard.pending_count(), 0);
    assert!(guard.problem().is_none());
}

#[test]
fn failed_opens_stale_tickets_and_pending_reservations_never_discard_dirty_data() {
    let mut guard = Guard::new();
    let old = guard.prepare_write("save").unwrap();
    assert_eq!(
        guard.save_handle_opened(&old.ticket, None).unwrap().obsolete.as_deref(),
        Some("save.~new")
    );
    let first = guard.prepare_write("save").unwrap();
    assert!(matches!(guard.save_handle_opened(&old.ticket, Some(10)), Err(-4)));
    assert!(matches!(guard.save_handle_opened(&first.ticket, Some(0)), Err(-1)));
    guard.save_handle_opened(&first.ticket, Some(10)).unwrap();
    assert!(matches!(guard.save_handle_opened(&first.ticket, Some(11)), Err(-4)));
    guard.wrote(10, 1).unwrap();
    let reservation = guard.prepare_write("save").unwrap();
    assert_eq!(guard.last_handle(10), Err(-2));
    assert_eq!(guard.save_handle_closed(10, None), Err(-2));
    guard.save_handle_opened(&reservation.ticket, None).unwrap();
    assert_eq!(guard.last_handle(10), Ok(true));
    assert_eq!(guard.save_handle_closed(10, None), Err(-1));
    guard.save_handle_closed(10, Some(12)).unwrap();
    let job = work(&mut guard);
    assert_eq!(job.flush, Some(12));
    published(&mut guard, job);
}

#[test]
fn four_handles_and_sixteen_recoverable_paths_apply_backpressure_before_writing() {
    let mut guard = Guard::new();
    for value in 1..=4 {
        open(&mut guard, "save", value);
    }
    assert!(matches!(guard.prepare_write("save"), Err(-2)));
    for value in 1..=3 {
        guard.save_handle_closed(value, None).unwrap();
    }
    guard.save_handle_closed(4, Some(5)).unwrap();
    let job = work(&mut guard);
    failed(&mut guard, job);
    for index in 1..16 {
        open(&mut guard, &format!("save{index}"), 100 + index);
    }
    assert!(matches!(guard.prepare_write("overflow"), Err(-2)));
    // Reopening an existing pending path does not reserve a seventeenth path.
    let plan = guard.prepare_write("save").unwrap();
    assert_eq!(plan.seed.as_deref(), Some("save.~pend"));
    guard.save_handle_opened(&plan.ticket, None).unwrap();
    assert_eq!(guard.pending_count(), 1);
}

#[test]
fn failed_replacement_redirects_reads_and_seeds_the_next_partial_write() {
    let mut guard = Guard::new();
    queue(&mut guard, "save", 10);
    let job = work(&mut guard);
    failed(&mut guard, job);
    assert_eq!(guard.read_path("save").unwrap(), "save.~pend");
    assert_eq!(guard.busy(), 0);
    assert!(guard.problem().is_none());
    let next = guard.prepare_write("save").unwrap();
    assert_eq!(next.seed.as_deref(), Some("save.~pend"));
    guard.save_handle_opened(&next.ticket, Some(20)).unwrap();
    guard.wrote(20, 1).unwrap();
    guard.save_handle_closed(20, Some(21)).unwrap();
    let job = work(&mut guard);
    assert!(!job.retry);
    let cleanup = guard
        .complete(job.token, Completion::Published { durable: true })
        .unwrap();
    assert_eq!(cleanup.obsolete.as_deref(), Some("save.~pend"));
    assert_eq!(guard.read_path("save").unwrap(), "save");
    assert!(guard.next_work().unwrap().is_none());
}

#[test]
fn four_attempt_problem_threshold_close_trigger_and_final_exit_retry_match_original() {
    let mut guard = Guard::new();
    queue(&mut guard, "save", 10);
    let job = work(&mut guard);
    failed(&mut guard, job);
    for attempt in 2..=4 {
        trigger(&mut guard, 100 + attempt * 10);
        let retry = work(&mut guard);
        assert!(retry.retry);
        assert_eq!(retry.save_shadow_path, "save.~pend");
        failed(&mut guard, retry);
        assert_eq!(guard.problem().is_some(), attempt == 4);
        assert!(guard.next_work().unwrap().is_none());
    }
    let problem = guard.problem().unwrap();
    assert_eq!(problem.real, "save");
    assert_eq!(problem.code, -3);
    assert!(!problem.durability);
    trigger(&mut guard, 200);
    assert!(guard.next_work().unwrap().is_none());
    guard.exit_retry();
    let retry = work(&mut guard);
    failed(&mut guard, retry);
    assert_eq!(guard.problem().unwrap().sequence, problem.sequence);
    guard.exit_retry();
    let retry = work(&mut guard);
    published(&mut guard, retry);
    assert_eq!(guard.pending_count(), 0);
    assert!(guard.problem().is_none());
}

#[test]
fn recovered_pending_warns_immediately_and_gets_one_close_retry_before_shutdown() {
    let mut guard = Guard::new();
    guard.recovered_pending("save", "save.~pend", -3).unwrap();
    let initial = guard.problem().unwrap().sequence;
    assert!(guard.next_work().unwrap().is_none());
    trigger(&mut guard, 10);
    let retry = work(&mut guard);
    failed(&mut guard, retry);
    assert!(guard.problem().unwrap().sequence > initial);
    trigger(&mut guard, 20);
    assert!(guard.next_work().unwrap().is_none());
    guard.exit_retry();
    let retry = work(&mut guard);
    published(&mut guard, retry);
    assert_eq!(guard.pending_count(), 0);
}

#[test]
fn post_publication_durability_failure_is_never_queued_as_a_pending_retry() {
    let mut guard = Guard::new();
    queue(&mut guard, "save", 10);
    let job = work(&mut guard);
    assert!(matches!(
        guard.complete(
            job.token,
            Completion::Failed {
                code: -6,
                preserved: "save.~new".into()
            }
        ),
        Err(-1)
    ));
    guard
        .complete(job.token, Completion::Published { durable: false })
        .unwrap();
    assert_eq!(guard.pending_count(), 0);
    assert_eq!(guard.read_path("save").unwrap(), "save");
    let problem = guard.problem().unwrap();
    assert_eq!(problem.code, -6);
    assert!(problem.durability);
    guard.exit_retry();
    assert!(guard.next_work().unwrap().is_none());
    trigger(&mut guard, 20);
    assert!(guard.problem().unwrap().durability);
    guard.acknowledge_durability();
    assert!(guard.problem().is_none());
}

#[test]
fn stale_completions_invalid_paths_and_foreign_handles_cannot_change_inflight_work() {
    let mut guard = Guard::new();
    queue(&mut guard, "save", 10);
    let job = work(&mut guard);
    assert!(matches!(
        guard.complete(job.token + 1, Completion::Published { durable: true }),
        Err(-4)
    ));
    assert!(matches!(
        guard.complete(
            job.token,
            Completion::Failed {
                code: -3,
                preserved: "other.~pend".into()
            }
        ),
        Err(-1)
    ));
    assert!(matches!(
        guard.complete(job.token, Completion::Missing {}),
        Err(-1)
    ));
    assert_eq!(guard.wrote(10, 1), Err(-4));
    assert_eq!(guard.last_handle(99), Err(-4));
    guard.complete(job.token, Completion::Unchanged {}).unwrap();
    assert!(matches!(
        guard.complete(job.token, Completion::Published { durable: true }),
        Err(-4)
    ));
    assert_eq!(guard.busy(), 0);
}

#[test]
fn retry_blocks_only_its_own_path_and_missing_pending_clears_its_record() {
    let mut guard = Guard::new();
    guard.recovered_pending("save", "save.~pend", -3).unwrap();
    guard.exit_retry();
    let retry = work(&mut guard);
    assert_eq!(guard.read_path("save"), Err(-2));
    assert!(matches!(guard.prepare_write("save"), Err(-2)));
    open(&mut guard, "other", 20);
    assert!(matches!(
        guard.complete(retry.token, Completion::Unchanged {}),
        Err(-1)
    ));
    guard.complete(retry.token, Completion::Missing {}).unwrap();
    assert_eq!(guard.pending_count(), 0);
    assert!(guard.problem().is_none());
    assert_eq!(guard.read_path("save").unwrap(), "save");
}

#[test]
fn delete_and_prepublication_io_failure_remain_busy_until_worker_finishes() {
    let mut guard = Guard::new();
    open(&mut guard, "save", 10);
    guard.delete_on_close(10).unwrap();
    guard.save_handle_closed(10, Some(11)).unwrap();
    let job = work(&mut guard);
    assert!(job.deleted);
    assert!(matches!(
        guard.complete(job.token, Completion::Published { durable: true }),
        Err(-1)
    ));
    guard
        .complete(job.token, Completion::RetryLater { code: -3 })
        .unwrap();
    assert_eq!(guard.busy(), 1);
    let next = work(&mut guard);
    assert!(next.deleted);
    assert!(next.flush.is_none());
    guard.complete(next.token, Completion::Deleted {}).unwrap();
    assert_eq!(guard.busy(), 0);
    assert_eq!(guard.pending_count(), 0);
}

#[test]
fn pending_shadow_is_not_removed_on_failed_reopen_or_retried_while_being_written() {
    let mut guard = Guard::new();
    queue(&mut guard, "save", 10);
    let job = work(&mut guard);
    guard
        .complete(
            job.token,
            Completion::Failed {
                code: -3,
                preserved: "save.~new".into(),
            },
        )
        .unwrap();
    let plan = guard.prepare_write("save").unwrap();
    assert!(plan.preserve_shadow);
    assert_eq!(plan.seed.as_deref(), Some("save.~new"));
    assert!(guard.save_handle_opened(&plan.ticket, None).unwrap().obsolete.is_none());
    assert_eq!(guard.read_path("save").unwrap(), "save.~new");
    open(&mut guard, "save", 20);
    guard.exit_retry();
    assert!(guard.next_work().unwrap().is_none());
    guard.save_handle_closed(20, Some(21)).unwrap();
    let job = work(&mut guard);
    assert!(!job.retry);
    published(&mut guard, job);
    assert!(guard.next_work().unwrap().is_none());
}

#[test]
fn handle_aliases_recovery_during_play_and_reserved_paths_are_rejected() {
    let mut guard = Guard::new();
    for path in [
        "",
        "../save",
        "/save",
        "a//b",
        "a/./b",
        "a/../b",
        "C:/save",
        "a\\b",
        "save.~new",
        "save.~pend",
        "save\0",
    ] {
        assert!(matches!(guard.prepare_write(path), Err(-1)), "{path:?}");
    }
    assert!(save_state::hidden("dir/save.~new"));
    assert!(!save_state::hidden(".~new"));
    open(&mut guard, "first", 10);
    let second = guard.prepare_write("second").unwrap();
    assert!(matches!(guard.save_handle_opened(&second.ticket, Some(10)), Err(-1)));
    guard.save_handle_opened(&second.ticket, Some(20)).unwrap();
    assert_eq!(guard.save_handle_closed(20, Some(10)), Err(-1));
    guard.save_handle_closed(20, Some(21)).unwrap();
    assert_eq!(guard.save_handle_closed(10, Some(21)), Err(-1));
    assert_eq!(guard.recovered_pending("save", "save.~pend", -3), Err(-2));
    guard.save_handle_closed(10, Some(11)).unwrap();
}

#[test]
fn every_reserved_path_can_fail_without_overflowing_the_pending_table() {
    let mut guard = Guard::new();
    for index in 0..16 {
        queue(&mut guard, &format!("save{index}"), 100 + index * 10);
    }
    for index in 0..16 {
        let job = work(&mut guard);
        assert_eq!(job.real, format!("save{index}"));
        failed(&mut guard, job);
    }
    assert_eq!(guard.pending_count(), 16);
    assert_eq!(guard.busy(), 0);
    assert!(matches!(guard.prepare_write("overflow"), Err(-2)));
    for index in 0..16 {
        assert_eq!(
            guard.read_path(&format!("save{index}")).unwrap(),
            format!("save{index}.~pend")
        );
    }
}

#[test]
fn newly_failed_saves_join_the_existing_close_triggered_retry_batch() {
    let mut guard = Guard::new();
    queue(&mut guard, "first", 10);
    let first = work(&mut guard);
    failed(&mut guard, first);
    queue(&mut guard, "second", 20);
    let second = work(&mut guard);
    failed(&mut guard, second);
    for expected in ["first", "second"] {
        let retry = work(&mut guard);
        assert!(retry.retry);
        assert_eq!(retry.real, expected);
        failed(&mut guard, retry);
    }
    assert!(guard.next_work().unwrap().is_none());
}

#[test]
fn missing_shadow_does_not_discard_an_older_complete_pending_save() {
    let mut guard = Guard::new();
    guard.recovered_pending("save", "save.~pend", -3).unwrap();
    open(&mut guard, "save", 10);
    assert!(guard.owns_handle(10));
    guard.save_handle_closed(10, Some(11)).unwrap();
    assert!(!guard.owns_handle(10));
    let job = work(&mut guard);
    guard.complete(job.token, Completion::Abandoned {}).unwrap();
    assert_eq!(guard.pending_count(), 1);
    assert_eq!(guard.read_path("save").unwrap(), "save.~pend");
    guard.pending_missing("save").unwrap();
    assert_eq!(guard.pending_count(), 0);
    assert!(guard.problem().is_none());
}

#[test]
fn confirmed_missing_pending_cannot_shift_an_active_retry_to_another_record() {
    let mut guard = Guard::new();
    guard.recovered_pending("first", "first.~pend", -3).unwrap();
    guard
        .recovered_pending("second", "second.~pend", -3)
        .unwrap();
    guard.exit_retry();
    let job = work(&mut guard);
    assert_eq!(guard.pending_missing("second"), Err(-2));
    assert_eq!(guard.pending_missing("first"), Err(-2));
    published(&mut guard, job);
    guard.pending_missing("second").unwrap();
    assert_eq!(guard.pending_count(), 0);
}

#[test]
fn startup_checks_run_before_publication_and_preserve_uncertain_publication_notice() {
    let mut guard = Guard::new();
    assert!(guard.startup_idle());
    assert_eq!(guard.startup_check("../save"), Err(-1));
    guard.startup_published("save", false).unwrap();
    assert!(guard.problem().unwrap().durability);
    open(&mut guard, "save", 10);
    assert!(!guard.startup_idle());
    assert_eq!(guard.startup_check("save"), Err(-2));
    assert_eq!(guard.startup_published("save", true), Err(-2));
}

#[test]
fn pending_policy_matches_unchanged_pinned_c_across_deterministic_event_traces() {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let oracle = std::env::var_os("TAKEDAWN_SAVE_PENDING_ORACLE")
        .expect("run service-save/check.py to build the pinned C oracle");
    let mut input = String::new();
    let mut expected = String::new();
    for seed in 1..=128u64 {
        input.push_str("Z 0 0\n");
        expected.push_str("0 0 0 0\n");
        let mut guard = Guard::new();
        let mut random = seed;
        for step in 0..32u64 {
            random = random
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let key = (random >> 32) & 3;
            let operation = match (random >> 40) % 10 {
                0 => 'X',
                1..=3 => 'S',
                _ => 'A',
            };
            let retry_failure = (random >> 52) & 1;
            input.push_str(&format!("{operation} {key} {retry_failure}\n"));
            if operation == 'X' {
                guard.exit_retry();
            } else {
                queue(&mut guard, &format!("save{key}"), 100 + step * 10);
                let job = work(&mut guard);
                assert!(!job.retry);
                if operation == 'A' {
                    failed(&mut guard, job);
                } else {
                    published(&mut guard, job);
                }
            }
            while let Some(job) = guard.next_work().unwrap() {
                assert!(job.retry);
                if retry_failure == 1 {
                    failed(&mut guard, job);
                } else {
                    published(&mut guard, job);
                }
            }
            let mut mask = 0u32;
            for index in 0..4 {
                let real = format!("save{index}");
                if guard.read_path(&real).unwrap() != real {
                    mask |= 1 << index;
                }
            }
            let notice = guard.problem();
            expected.push_str(&format!(
                "{} {} {} {}\n",
                guard.pending_count(),
                u32::from(notice.is_some()),
                notice.map_or(0, |p| p.sequence),
                mask
            ));
        }
    }
    let mut child = Command::new(oracle)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut pipe = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || pipe.write_all(input.as_bytes()).unwrap());
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    assert!(output.status.success());
    let actual = String::from_utf8(output.stdout).unwrap();
    let actual_lines: Vec<_> = actual.lines().collect();
    let expected_lines: Vec<_> = expected.lines().collect();
    assert_eq!(actual_lines.len(), expected_lines.len());
    for (index, (actual, expected)) in actual_lines.iter().zip(&expected_lines).enumerate() {
        assert_eq!(
            actual,
            expected,
            "pinned C mismatch at trace {}, event {}",
            index / 33,
            index % 33
        );
    }
}
