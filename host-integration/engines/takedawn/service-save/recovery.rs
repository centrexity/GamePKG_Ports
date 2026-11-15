// Startup-only recovery. The mutable guard borrow prevents title activity until
// this scanner is released. Disk operations use only shared capability services.
use crate::adapter;
use crate::files;
use crate::gamepkg;
use crate::history;
use crate::state;

pub struct Report {
    pub unfinished_removed: u32,
    pub recovered: u32,
    pub archived: u32,
    pub pending: u32,
    pub older_kept: u32,
    pub archive_errors: u32,
    pub uncertain: u32,
}
pub enum RecoveryStep {
    Progress {},
    History {
        real: std::string::String,
        pending: std::string::String,
        newer: bool,
        error: i32,
    },
    Done {
        report: Report,
    },
}
struct Candidate {
    path: std::string::String,
    pending: bool,
}
struct Waiting {
    real: std::string::String,
    path: std::string::String,
    newer: bool,
    error: i32,
}
pub struct Recovery<'a> {
    guard: &'a mut adapter::SaveGuard,
    grant: u64,
    candidates: std::vec::Vec<Candidate>,
    waiting: Option<Waiting>,
    report: Report,
}
fn recovery_text(text: &str) -> std::string::String {
    let mut owned = std::string::String::new();
    owned.push_str(text);
    owned
}
fn recovery_scan(grant: u64, title: &str) -> Result<std::vec::Vec<Candidate>, i32> {
    if title.len() != 8 {
        return Err(-1);
    }
    for byte in title.bytes() {
        if !byte.is_ascii_hexdigit() {
            return Err(-1);
        }
    }
    let mut directories = std::vec::Vec::new();
    for partition in ["TDATA", "UDATA"] {
        let path = history::history_child(partition, title)?;
        if history::directory_present(grant, &path)? {
            directories.push((path, 0u32));
        }
    }
    let mut seen = 0usize;
    let mut candidates = std::vec::Vec::new();
    while let Some((parent, depth)) = directories.pop() {
        if depth > 32 {
            return Err(-2);
        }
        let entries = gamepkg::directory_list(grant, &parent)?;
        seen = seen.checked_add(entries.len()).ok_or(-2)?;
        if seen > 10000 {
            return Err(-2);
        }
        for entry in entries {
            let path = history::history_child(&parent, &entry.name)?;
            if entry.directory {
                directories.push((path, depth + 1));
            } else if state::hidden(&entry.name) {
                candidates.push(Candidate {
                    path,
                    pending: entry.name.ends_with(".~pend"),
                });
            }
        }
    }
    candidates.reverse();
    Ok(candidates)
}
impl<'a> Recovery<'a> {
    pub fn begin(guard: &'a mut adapter::SaveGuard, grant: u64, title: &str) -> Result<Self, i32> {
        if !guard.for_grant(grant) {
            return Err(-4);
        }
        if !guard.startup_idle() || guard.pending_count() != 0 {
            return Err(-2);
        }
        let candidates = recovery_scan(grant, title)?;
        for candidate in &candidates {
            let suffix = if candidate.pending { 6usize } else { 5usize };
            guard.startup_check(
                candidate
                    .path
                    .get(..candidate.path.len() - suffix)
                    .ok_or(-1)?,
            )?;
        }
        Ok(Self {
            guard,
            grant,
            candidates,
            waiting: None,
            report: Report {
                unfinished_removed: 0,
                recovered: 0,
                archived: 0,
                pending: 0,
                older_kept: 0,
                archive_errors: 0,
                uncertain: 0,
            },
        })
    }
    pub fn summary(&self) -> Report {
        Report {
            unfinished_removed: self.report.unfinished_removed,
            recovered: self.report.recovered,
            archived: self.report.archived,
            pending: self.report.pending,
            older_kept: self.report.older_kept,
            archive_errors: self.report.archive_errors,
            uncertain: self.report.uncertain,
        }
    }
    pub fn step(&mut self) -> Result<RecoveryStep, i32> {
        if let Some(waiting) = &self.waiting {
            return Ok(RecoveryStep::History {
                real: recovery_text(&waiting.real),
                pending: recovery_text(&waiting.path),
                newer: waiting.newer,
                error: waiting.error,
            });
        }
        let candidate = match self.candidates.last() {
            Some(candidate) => Candidate {
                path: recovery_text(&candidate.path),
                pending: candidate.pending,
            },
            None => {
                return Ok(RecoveryStep::Done {
                    report: self.summary(),
                })
            }
        };
        if !gamepkg::file_exists(self.grant, &candidate.path)? {
            let _ = self.candidates.pop();
            return Ok(RecoveryStep::Progress {});
        }
        if !candidate.pending {
            gamepkg::file_remove(self.grant, &candidate.path)?;
            let _ = self.candidates.pop();
            self.report.unfinished_removed += 1;
            return Ok(RecoveryStep::Progress {});
        }
        let length = candidate.path.len() - 6;
        let real = recovery_text(candidate.path.get(..length).ok_or(-1)?);
        let newer = if !gamepkg::file_exists(self.grant, &real)? {
            true
        } else {
            files::modified(self.grant, &candidate.path)? >= files::modified(self.grant, &real)?
        };
        let mut code = -3;
        if newer {
            match files::publish(self.grant, &candidate.path, &real) {
                Ok(files::Publication::Durable {}) => {
                    self.guard.startup_published(&real, true)?;
                    let _ = self.candidates.pop();
                    self.report.recovered += 1;
                    return Ok(RecoveryStep::Progress {});
                }
                Ok(files::Publication::CommittedNotDurable {}) => {
                    self.guard.startup_published(&real, false)?;
                    let _ = self.candidates.pop();
                    self.report.recovered += 1;
                    self.report.uncertain += 1;
                    return Ok(RecoveryStep::Progress {});
                }
                Err(error) => code = error,
            }
        }
        self.waiting = Some(Waiting {
            real: recovery_text(&real),
            path: recovery_text(&candidate.path),
            newer,
            error: code,
        });
        Ok(RecoveryStep::History {
            real,
            pending: candidate.path,
            newer,
            error: code,
        })
    }
    pub fn history_done(&mut self, proof: &history::Archived) -> Result<(), i32> {
        let waiting = self.waiting.as_ref().ok_or(-4)?;
        if !proof.valid_for(self.grant, &waiting.real, &waiting.path) {
            return Err(-4);
        }
        proof.verify_pending(self.grant, &waiting.real, &waiting.path)?;
        if waiting.newer {
            self.guard
                .recovered_pending(&waiting.real, &waiting.path, waiting.error)?;
            self.report.pending += 1;
        } else if gamepkg::file_exists(self.grant, &waiting.path)? {
            // The complete snapshot proof must exist before discarding this
            // older pending save. Any deletion failure keeps this request live.
            gamepkg::file_remove(self.grant, &waiting.path)?;
        }
        self.report.archived += 1;
        self.waiting = None;
        let _ = self.candidates.pop();
        Ok(())
    }
    pub fn history_failed(&mut self, error: i32) -> Result<(), i32> {
        if !(-6..=-1).contains(&error) {
            return Err(-1);
        }
        let waiting = self.waiting.as_ref().ok_or(-4)?;
        if waiting.newer {
            self.guard
                .recovered_pending(&waiting.real, &waiting.path, waiting.error)?;
            self.report.pending += 1;
        } else {
            self.report.older_kept += 1;
        }
        self.report.archive_errors += 1;
        self.waiting = None;
        let _ = self.candidates.pop();
        Ok(())
    }
}
