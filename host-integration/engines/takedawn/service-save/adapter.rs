// Port-owned disk adapter. Work is extracted under the caller's serialization,
// executed off the window owner thread, then acknowledged by its exact token.
use crate::gamepkg;
use crate::save_files;
use crate::save_state;

pub struct SaveGuard {
    grant: u64,
    core: save_state::Guard,
}
pub struct Job {
    grant: u64,
    work: save_state::Work,
    flushed: bool,
    waiting_history: bool,
    history_done: bool,
    history_error: Option<i32>,
    done: bool,
}
pub struct Finished {
    token: u64,
    result: save_state::Completion,
    pub history_error: Option<i32>,
    pub preservation_durable: bool,
}
pub enum Step {
    Complete { finished: Finished },
    History { real: std::string::String },
}
fn owned_path(path: &str) -> std::string::String {
    let mut text = std::string::String::new();
    text.push_str(path);
    text
}
fn remove_present(grant: u64, path: &str) -> Result<(), i32> {
    if gamepkg::file_exists(grant, path)? {
        gamepkg::file_remove(grant, path)?;
    }
    Ok(())
}
pub fn clean_obsolete(grant: u64, cleanup: save_state::Cleanup) -> Result<(), i32> {
    if let Some(path) = cleanup.obsolete {
        remove_present(grant, &path)?;
    }
    Ok(())
}
fn regular_open_prepare(handle: u64, attributes: Option<u32>, through: bool) -> Result<(), i32> {
    if let Some(flag_bits) = attributes {
        gamepkg::save_file_set_attributes(handle, flag_bits)?;
    }
    if through {
        gamepkg::file_sync(handle)?;
    }
    Ok(())
}
impl SaveGuard {
    pub fn new(grant: u64) -> Self {
        Self {
            grant,
            core: save_state::Guard::new(),
        }
    }
    pub fn set_history(&mut self, enabled: bool) {
        self.core.set_history(enabled);
    }
    pub fn for_grant(&self, id: u64) -> bool {
        self.grant == id
    }
    pub fn startup_idle(&self) -> bool {
        self.core.startup_idle()
    }
    pub fn startup_published(&mut self, real: &str, durable: bool) -> Result<(), i32> {
        self.core.startup_published(real, durable)
    }
    pub fn startup_check(&self, real: &str) -> Result<(), i32> {
        self.core.startup_check(real)
    }
    pub fn busy(&self) -> usize {
        self.core.busy()
    }
    pub fn pending_count(&self) -> usize {
        self.core.pending_count()
    }
    pub fn problem(&self) -> Option<save_state::Problem> {
        self.core.problem()
    }
    pub fn acknowledge_durability(&mut self) {
        self.core.acknowledge_durability();
    }
    pub fn exit_retry(&mut self) {
        self.core.exit_retry();
    }
    pub fn recovered_pending(&mut self, real: &str, pending: &str, code: i32) -> Result<(), i32> {
        self.core.recovered_pending(real, pending, code)
    }
    fn seed_shadow(
        &mut self,
        real: &str,
        plan: &save_state::OpenPlan,
        preserve_attributes: bool,
    ) -> Result<(), i32> {
        let seed = match &plan.seed {
            Some(seed) => seed,
            None => return Ok(()),
        };
        if gamepkg::file_exists(self.grant, seed)? {
            if seed != &plan.save_shadow_path {
                let _ = save_files::copy_seed_with_attributes(
                    self.grant,
                    seed,
                    real,
                    preserve_attributes,
                )?;
            }
            return Ok(());
        }
        if seed != real {
            self.core.pending_missing(real)?;
        }
        if gamepkg::file_exists(self.grant, real)? {
            if preserve_attributes {
                save_files::copy_seed_with_attributes(self.grant, real, real, true)?;
            } else {
                save_files::copy_shadow(self.grant, real)?;
            }
            return Ok(());
        }
        let file = gamepkg::file_open(self.grant, true, &plan.save_shadow_path)?;
        let resized = gamepkg::file_resize(file, 0);
        let close_status = gamepkg::file_close(file);
        resized?;
        close_status
    }
    fn failed_open(&mut self, ticket: &save_state::OpenTicket) -> Result<(), i32> {
        let cleanup = self.core.save_handle_opened(ticket, None)?;
        clean_obsolete(self.grant, cleanup)
    }
    pub fn open_write(&mut self, real: &str) -> Result<u64, i32> {
        self.open_guarded_write(real, false, false, None)
    }
    fn open_guarded_write(
        &mut self,
        real: &str,
        write_through: bool,
        preserve_attributes: bool,
        new_attributes: Option<u32>,
    ) -> Result<u64, i32> {
        let plan = self.core.prepare_write(real)?;
        if let Err(error) = self.seed_shadow(real, &plan, preserve_attributes) {
            let _ = self.failed_open(&plan.ticket);
            return Err(error);
        }
        let file = match gamepkg::file_open(self.grant, true, &plan.save_shadow_path) {
            Ok(file) => file,
            Err(error) => {
                let _ = self.failed_open(&plan.ticket);
                return Err(error);
            }
        };
        if let Err(error) = regular_open_prepare(file, new_attributes, write_through) {
            let sync_open_close_result = gamepkg::file_close(file);
            let cleanup = self.failed_open(&plan.ticket);
            sync_open_close_result?;
            cleanup?;
            return Err(error);
        }
        if let Err(error) = self.core.save_handle_opened(&plan.ticket, Some(file)) {
            let _ = gamepkg::file_close(file);
            let _ = self.failed_open(&plan.ticket);
            return Err(error);
        }
        Ok(file)
    }
    fn guarded_read_name(&mut self, real: &str) -> Result<std::string::String, i32> {
        let path = self.core.read_path(real)?;
        if path != real && !gamepkg::file_exists(self.grant, &path)? {
            self.core.pending_missing(real)?;
            return Ok(owned_path(real));
        }
        Ok(path)
    }
    pub fn open_read(&mut self, real: &str) -> Result<u64, i32> {
        let path = self.guarded_read_name(real)?;
        gamepkg::file_open(self.grant, false, &path)
    }

    // Return the Xbox disposition information with a handle, or a zero
    // handle and the exact refused disposition. Only the worker calls this
    // protocol under the host's exclusive per-title save grant.
    pub fn open_regular(
        &mut self,
        real: &str,
        disposition: u32,
        writable: bool,
        write_through: bool,
        creation_flags: u32,
    ) -> Result<(u64, u32), i32> {
        if !(1..=3).contains(&disposition)
            || (!writable && (disposition != 1 || write_through))
            || creation_flags & !0x26 != 0
            || (creation_flags != 0 && disposition == 1)
        {
            return Err(-1);
        }
        let save_shadow_path = if writable {
            self.core.opened_write_path(real)?
        } else {
            None
        };
        let (present, key) = if let Some(open_shadow) = save_shadow_path {
            if !gamepkg::file_exists(self.grant, &open_shadow)? {
                return Err(-3);
            }
            (true, open_shadow)
        } else {
            let selected = self.guarded_read_name(real)?;
            (gamepkg::file_exists(self.grant, &selected)?, selected)
        };
        if present && disposition == 2 {
            return Ok((0, 4));
        }
        if !present && disposition == 1 {
            return Ok((0, 5));
        }
        let file = if writable {
            let initial_flags = if present || creation_flags == 0 {
                None
            } else {
                Some(creation_flags | 0x20)
            };
            self.open_guarded_write(real, write_through, true, initial_flags)?
        } else {
            gamepkg::file_open(self.grant, false, &key)?
        };
        Ok((file, if present { 1 } else { 2 }))
    }
    pub fn write_at(&mut self, handle: u64, offset: u64, bytes: &[u8]) -> Result<(), i32> {
        if !self.core.owns_handle(handle) {
            return Err(-4);
        }
        gamepkg::file_write(handle, offset, bytes)?;
        self.core.wrote(handle, bytes.len() as u64)
    }
    pub fn resize(&mut self, handle: u64, length: u64) -> Result<(), i32> {
        if !self.core.owns_handle(handle) {
            return Err(-4);
        }
        gamepkg::file_resize(handle, length)?;
        self.core.wrote(handle, 0)
    }
    pub fn delete_on_close(&mut self, handle: u64) -> Result<(), i32> {
        self.core.delete_on_close(handle)
    }
    pub fn save_guard_close(&mut self, handle: u64) -> Result<(), i32> {
        let flush = if self.core.last_handle(handle)? {
            Some(save_files::retain_for_flush(handle)?)
        } else {
            None
        };
        if let Err(error) = gamepkg::file_close(handle) {
            if let Some(retained) = flush {
                let _ = gamepkg::file_close(retained);
            }
            return Err(error);
        }
        if let Err(error) = self.core.save_handle_closed(handle, flush) {
            if let Some(retained) = flush {
                let _ = gamepkg::file_close(retained);
            }
            return Err(error);
        }
        Ok(())
    }
    pub fn begin_job(&mut self) -> Result<Option<Job>, i32> {
        match self.core.next_work()? {
            Some(work) => Ok(Some(Job {
                grant: self.grant,
                work,
                flushed: false,
                waiting_history: false,
                history_done: false,
                history_error: None,
                done: false,
            })),
            None => Ok(None),
        }
    }
    // State-only acknowledgement; run returned cleanup on the worker too.
    pub fn finish(&mut self, finished: Finished) -> Result<save_state::Cleanup, i32> {
        self.core.complete(finished.token, finished.result)
    }
}
impl Job {
    fn make_completion(
        &mut self,
        result: save_state::Completion,
        preservation_durable: bool,
    ) -> Step {
        self.done = true;
        Step::Complete {
            finished: Finished {
                token: self.work.token,
                result,
                history_error: self.history_error,
                preservation_durable,
            },
        }
    }
    pub fn history_finished(&mut self, result: Result<(), i32>) -> Result<(), i32> {
        if self.done || !self.waiting_history || self.history_done {
            return Err(-4);
        }
        // The original before_replace hook returns void: history failure must
        // be surfaced, but does not silently prevent the title save publishing.
        self.history_error = result.err();
        self.history_done = true;
        self.waiting_history = false;
        Ok(())
    }
    fn flush_source(&mut self) -> Result<(), i32> {
        if self.flushed || self.work.retry {
            return Ok(());
        }
        if let Some(handle) = self.work.flush.take() {
            save_files::sync_close(handle)?;
        } else if !self.work.deleted
            && gamepkg::file_exists(self.grant, &self.work.save_shadow_path)?
        {
            let handle = gamepkg::file_open(self.grant, true, &self.work.save_shadow_path)?;
            save_files::sync_close(handle)?;
        }
        self.flushed = true;
        Ok(())
    }
    fn replace(&mut self) -> Result<Step, i32> {
        match save_files::publish_save(self.grant, &self.work.save_shadow_path, &self.work.real) {
            Ok(save_files::Publication::Durable {}) => {
                Ok(self.make_completion(save_state::Completion::Published { durable: true }, true))
            }
            Ok(save_files::Publication::CommittedNotDurable {}) => Ok(
                self.make_completion(save_state::Completion::Published { durable: false }, true)
            ),
            Err(error) => {
                let mut preserved = owned_path(&self.work.save_shadow_path);
                let mut durable = true;
                if !self.work.retry {
                    let mut pending = owned_path(&self.work.real);
                    pending.push_str(".~pend");
                    match save_files::publish_save(
                        self.grant,
                        &self.work.save_shadow_path,
                        &pending,
                    ) {
                        Ok(save_files::Publication::Durable {}) => preserved = pending,
                        Ok(save_files::Publication::CommittedNotDurable {}) => {
                            preserved = pending;
                            durable = false;
                        }
                        Err(_) => {}
                    }
                }
                Ok(self.make_completion(
                    save_state::Completion::Failed {
                        code: error,
                        preserved,
                    },
                    durable,
                ))
            }
        }
    }
    // On Err retain this same Job and wait before retrying it. No busy loop,
    // drop/restart, in-place fallback or automatic conversion to "missing".
    pub fn step(&mut self) -> Result<Step, i32> {
        if self.done {
            return Err(-4);
        }
        self.flush_source()?;
        if self.work.retry {
            if !gamepkg::file_exists(self.grant, &self.work.save_shadow_path)? {
                return Ok(self.make_completion(save_state::Completion::Missing {}, true));
            }
            return self.replace();
        }
        if self.work.deleted {
            remove_present(self.grant, &self.work.save_shadow_path)?;
            remove_present(self.grant, &self.work.real)?;
            return Ok(self.make_completion(save_state::Completion::Deleted {}, true));
        }
        if !gamepkg::file_exists(self.grant, &self.work.save_shadow_path)? {
            return Ok(self.make_completion(save_state::Completion::Abandoned {}, true));
        }
        if gamepkg::file_exists(self.grant, &self.work.real)?
            && save_files::same_save_bytes(
                self.grant,
                &self.work.save_shadow_path,
                &self.work.real,
            )?
        {
            remove_present(self.grant, &self.work.save_shadow_path)?;
            return Ok(self.make_completion(save_state::Completion::Unchanged {}, true));
        }
        if self.work.history && !self.history_done {
            self.waiting_history = true;
            return Ok(Step::History {
                real: owned_path(&self.work.real),
            });
        }
        self.replace()
    }
}
