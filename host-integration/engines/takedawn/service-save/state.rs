// Port-owned saveguard transitions. This module performs no I/O or callbacks.
// The adapter executes Work on its reviewed worker task using shared services.
use crate::paths;
const FREE: u32 = 0;
const PREPARING: u32 = 1;
const OPEN: u32 = 2;
const QUEUED: u32 = 3;
const COMMITTING: u32 = 4;
const COMMIT: u32 = 1;
const RETRY: u32 = 2;

pub struct OpenTicket {
    entry: usize,
    generation: u64,
    reservation: u64,
}
pub struct OpenPlan {
    pub ticket: OpenTicket,
    pub shadow: std::string::String,
    // The adapter seeds shadow from this pending/real path if it exists.
    // None means another title handle already owns the prepared shadow.
    pub seed: Option<std::string::String>,
    // A failed open must never remove a pending file reused as the shadow.
    pub preserve_shadow: bool,
}
pub struct Work {
    pub token: u64,
    pub retry: bool,
    pub real: std::string::String,
    pub shadow: std::string::String,
    pub flush: Option<u64>,
    pub deleted: bool,
    pub history: bool,
    pub writes: u64,
    pub bytes: u64,
}
pub enum Completion {
    Published {
        durable: bool,
    },
    Unchanged {},
    Deleted {},
    Missing {},
    // Title moved/removed its shadow without delete-on-close. Leave any older
    // pending save alone, matching the original missing-shadow commit branch.
    Abandoned {},
    // I/O failed before the operation could reach a complete publication or
    // preserved-pending outcome. Keep this entry queued; caller applies a wait.
    RetryLater {
        code: i32,
    },
    // Failed replacement preserved the complete newer file at this exact path.
    Failed {
        code: i32,
        preserved: std::string::String,
    },
}
pub struct Cleanup {
    pub obsolete: Option<std::string::String>,
}
pub struct Problem {
    pub sequence: u64,
    pub real: std::string::String,
    pub code: i32,
    pub durability: bool,
}
struct Handle {
    reservation: u64,
    value: Option<u64>,
}
struct Entry {
    generation: u64,
    phase: u32,
    real: std::string::String,
    shadow: std::string::String,
    handles: std::vec::Vec<Handle>,
    flush: Option<u64>,
    preserve_shadow: bool,
    dirty: bool,
    deleted: bool,
    writes: u64,
    bytes: u64,
}
impl Entry {
    fn empty() -> Self {
        Self {
            generation: 0,
            phase: FREE,
            real: std::string::String::new(),
            shadow: std::string::String::new(),
            handles: std::vec::Vec::new(),
            flush: None,
            preserve_shadow: false,
            dirty: false,
            deleted: false,
            writes: 0,
            bytes: 0,
        }
    }
}
struct Pending {
    real: std::string::String,
    preserved: std::string::String,
    tries: u32,
    code: i32,
    retry: bool,
    problem: bool,
}
struct Active {
    token: u64,
    kind: u32,
    slot: usize,
}
pub struct Guard {
    entries: std::vec::Vec<Entry>,
    pending: std::vec::Vec<Pending>,
    active: Option<Active>,
    sequence: u64,
    problem_sequence: u64,
    problem: Option<Problem>,
    history: bool,
    retry_due: bool,
}
fn copy_path(path: &str) -> std::string::String {
    let mut owned = std::string::String::new();
    owned.push_str(path);
    owned
}
fn shadow_name(real: &str) -> std::string::String {
    let mut path = copy_path(&real);
    path.push_str(".~new");
    path
}
fn pending_name(real: &str) -> std::string::String {
    let mut path = copy_path(&real);
    path.push_str(".~pend");
    path
}
pub fn hidden(path: &str) -> bool {
    (path.len() > 5 && path.ends_with(".~new")) || (path.len() > 6 && path.ends_with(".~pend"))
}
fn title_path(path: &str) -> bool {
    if path.is_empty() || path.len() > 4089 || hidden(path) {
        return false;
    }
    let parts = match paths::segments(path) {
        Ok(data) => data,
        Err(_) => return false,
    };
    for part in parts {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.len() > 249
            || part.ends_with(" ")
            || part.ends_with(".")
        {
            return false;
        }
    }
    for byte in path.bytes() {
        if byte < 32 || byte == 127 || byte == 92 || byte == 58 {
            return false;
        }
    }
    true
}
// Preserve table order without Vec::remove's out-of-line assertion helper.
// Ordinary bounds traps and mem::replace remain inside the reviewed policy.
fn remove_stable<T>(items: &mut std::vec::Vec<T>, index: usize) -> Result<T, i32> {
    if index >= items.len() {
        return Err(-1);
    }
    let mut removed = items.pop().ok_or(-1)?;
    for place in (index..items.len()).rev() {
        removed = std::mem::replace(&mut items[place], removed);
    }
    Ok(removed)
}
impl Guard {
    pub fn new() -> Self {
        let mut entries = std::vec::Vec::new();
        while entries.len() < 16 {
            entries.push(Entry::empty());
        }
        Self {
            entries,
            pending: std::vec::Vec::new(),
            active: None,
            sequence: 0,
            problem_sequence: 0,
            problem: None,
            history: false,
            retry_due: false,
        }
    }
    fn next_id(&mut self) -> Result<u64, i32> {
        // Never recycle a ticket, even after a slot becomes free.
        self.sequence = self.sequence.checked_add(1).ok_or(-2)?;
        Ok(self.sequence)
    }
    fn entry_path(&self, real: &str) -> Option<usize> {
        for index in 0..self.entries.len() {
            if self.entries[index].phase != FREE && self.entries[index].real == real {
                return Some(index);
            }
        }
        None
    }
    fn pending_path(&self, real: &str) -> Option<usize> {
        for index in 0..self.pending.len() {
            if self.pending[index].real == real {
                return Some(index);
            }
        }
        None
    }
    fn locate_handle(&self, value: u64) -> Option<(usize, usize)> {
        for index in 0..self.entries.len() {
            for slot in 0..self.entries[index].handles.len() {
                if self.entries[index].handles[slot].value == Some(value) {
                    return Some((index, slot));
                }
            }
        }
        None
    }
    fn is_retained(&self, value: u64) -> bool {
        for entry in &self.entries {
            if entry.flush == Some(value) {
                return true;
            }
        }
        false
    }
    fn retrying(&self, real: &str) -> bool {
        match &self.active {
            Some(active) => active.kind == RETRY && self.pending[active.slot].real == real,
            None => false,
        }
    }
    fn report(&mut self, real: &str, code: i32, durability: bool) {
        self.problem_sequence = self.problem_sequence.wrapping_add(1).max(1);
        self.problem = Some(Problem {
            sequence: self.problem_sequence,
            real: copy_path(&real),
            code,
            durability,
        });
    }
    fn refresh_problem(&mut self) {
        if let Some(notice) = &self.problem {
            if notice.durability {
                return;
            }
        }
        for pending in &self.pending {
            if pending.problem {
                return;
            }
        }
        self.problem = None;
    }
    pub fn set_history(&mut self, enabled: bool) {
        self.history = enabled;
    }
    pub fn prepare_write(&mut self, real: &str) -> Result<OpenPlan, i32> {
        if !title_path(real) {
            return Err(-1);
        }
        if self.retrying(real) {
            return Err(-2);
        }
        let reservation = self.next_id()?;
        if let Some(index) = self.entry_path(real) {
            let entry = &mut self.entries[index];
            if entry.phase != OPEN || entry.handles.len() >= 4 {
                return Err(-2);
            }
            entry.handles.push(Handle {
                reservation,
                value: None,
            });
            return Ok(OpenPlan {
                ticket: OpenTicket {
                    entry: index,
                    generation: entry.generation,
                    reservation,
                },
                shadow: copy_path(&entry.shadow),
                seed: None,
                preserve_shadow: entry.preserve_shadow,
            });
        }
        // Reserve recovery capacity before any new shadow can be written. A
        // full table returns Busy, never an unguarded in-place-write fallback.
        let mut occupied = self.pending.len();
        for entry in &self.entries {
            if entry.phase != FREE && self.pending_path(&entry.real).is_none() {
                occupied += 1;
            }
        }
        if self.pending_path(real).is_none() && occupied >= 16 {
            return Err(-2);
        }
        let mut free = None;
        for index in 0..self.entries.len() {
            if self.entries[index].phase == FREE {
                free = Some(index);
                break;
            }
        }
        let index = free.ok_or(-2)?;
        let shadow = shadow_name(real);
        let seed = match self.pending_path(real) {
            Some(pending) => copy_path(&self.pending[pending].preserved),
            None => copy_path(&real),
        };
        let preserve = seed == shadow;
        let mut entry = Entry::empty();
        entry.phase = PREPARING;
        entry.generation = reservation;
        entry.real = copy_path(&real);
        entry.shadow = copy_path(&shadow);
        entry.preserve_shadow = preserve;
        entry.handles.push(Handle {
            reservation,
            value: None,
        });
        self.entries[index] = entry;
        Ok(OpenPlan {
            ticket: OpenTicket {
                entry: index,
                generation: reservation,
                reservation,
            },
            shadow,
            seed: Some(seed),
            preserve_shadow: preserve,
        })
    }
    // Actual file open completed. On Err the adapter closes any unaccepted OS-
    // independent service handle; this method never owns an unaccepted value.
    pub fn opened(&mut self, ticket: &OpenTicket, handle: Option<u64>) -> Result<Cleanup, i32> {
        if let Some(value) = handle {
            if value == 0 || self.locate_handle(value).is_some() || self.is_retained(value) {
                return Err(-1);
            }
        }
        let entry = self.entries.get_mut(ticket.entry).ok_or(-4)?;
        if entry.generation != ticket.generation
            || (entry.phase != PREPARING && entry.phase != OPEN)
        {
            return Err(-4);
        }
        let mut position = None;
        for index in 0..entry.handles.len() {
            if entry.handles[index].reservation == ticket.reservation
                && entry.handles[index].value.is_none()
            {
                position = Some(index);
                break;
            }
        }
        let position = position.ok_or(-4)?;
        if let Some(value) = handle {
            entry.handles[position].value = Some(value);
            entry.phase = OPEN;
            return Ok(Cleanup { obsolete: None });
        }
        let _ = remove_stable(&mut entry.handles, position)?;
        if entry.handles.is_empty() {
            let obsolete = if entry.preserve_shadow {
                None
            } else {
                Some(copy_path(&entry.shadow))
            };
            *entry = Entry::empty();
            return Ok(Cleanup { obsolete });
        }
        Ok(Cleanup { obsolete: None })
    }
    pub fn wrote(&mut self, handle: u64, bytes: u64) -> Result<(), i32> {
        let (index, _) = self.locate_handle(handle).ok_or(-4)?;
        let entry = &mut self.entries[index];
        if entry.phase != OPEN {
            return Err(-4);
        }
        entry.dirty = true;
        entry.writes = entry.writes.saturating_add(1);
        entry.bytes = entry.bytes.saturating_add(bytes);
        Ok(())
    }
    pub fn owns_handle(&self, handle: u64) -> bool {
        self.locate_handle(handle).is_some()
    }
    // Call only after the shared file service confirmed actual absence.
    pub fn pending_missing(&mut self, real: &str) -> Result<(), i32> {
        if let Some(active) = &self.active {
            if active.kind == RETRY {
                return Err(-2);
            }
        }
        if let Some(index) = self.pending_path(real) {
            let _ = remove_stable(&mut self.pending, index)?;
            if let Some(index) = self.entry_path(real) {
                self.entries[index].preserve_shadow = false;
            }
            self.refresh_problem();
        }
        Ok(())
    }
    pub fn delete_on_close(&mut self, handle: u64) -> Result<(), i32> {
        let (index, _) = self.locate_handle(handle).ok_or(-4)?;
        self.entries[index].deleted = true;
        self.entries[index].dirty = true;
        Ok(())
    }
    pub fn last_handle(&self, handle: u64) -> Result<bool, i32> {
        let (index, _) = self.locate_handle(handle).ok_or(-4)?;
        for slot in &self.entries[index].handles {
            if slot.value.is_none() {
                return Err(-2);
            }
        }
        Ok(self.entries[index].handles.len() == 1)
    }
    // Call only after a successful service close; the last handle must first
    // acquire its retained flush duplicate. Pending open reservations count.
    pub fn closed(&mut self, handle: u64, flush: Option<u64>) -> Result<(), i32> {
        let (index, slot) = self.locate_handle(handle).ok_or(-4)?;
        for opening in &self.entries[index].handles {
            if opening.value.is_none() {
                return Err(-2);
            }
        }
        let last = self.entries[index].handles.len() == 1;
        if last != flush.is_some() {
            return Err(-1);
        }
        if let Some(value) = flush {
            if value == 0
                || value == handle
                || self.locate_handle(value).is_some()
                || self.is_retained(value)
            {
                return Err(-1);
            }
        }
        let entry = &mut self.entries[index];
        let _ = remove_stable(&mut entry.handles, slot)?;
        if last {
            entry.phase = QUEUED;
            entry.flush = flush;
            for pending in &mut self.pending {
                if pending.tries < 4 {
                    self.retry_due = true;
                }
            }
        }
        Ok(())
    }
    // An already opened write sees its shared shadow, including a newly
    // created file which still has no published name.
    pub fn opened_write_path(&self, real: &str) -> Result<Option<std::string::String>, i32> {
        if !title_path(real) {
            return Err(-1);
        }
        if self.retrying(real) {
            return Err(-2);
        }
        if let Some(index) = self.entry_path(real) {
            let entry = &self.entries[index];
            if entry.phase != OPEN {
                return Err(-2);
            }
            return Ok(Some(copy_path(&entry.shadow)));
        }
        Ok(None)
    }
    pub fn read_path(&self, real: &str) -> Result<std::string::String, i32> {
        if !title_path(real) {
            return Err(-1);
        }
        if self.retrying(real) {
            return Err(-2);
        }
        if let Some(index) = self.entry_path(real) {
            if self.entries[index].phase != OPEN {
                return Err(-2);
            }
        }
        if let Some(index) = self.pending_path(real) {
            return Ok(copy_path(&self.pending[index].preserved));
        }
        Ok(copy_path(&real))
    }
    pub fn busy(&self) -> usize {
        let mut count = 0;
        for entry in &self.entries {
            if entry.phase != FREE && (entry.phase != OPEN || entry.dirty) {
                count += 1;
            }
        }
        count
    }
    pub fn startup_idle(&self) -> bool {
        if self.active.is_some() {
            return false;
        }
        for entry in &self.entries {
            if entry.phase != FREE {
                return false;
            }
        }
        true
    }
    pub fn startup_published(&mut self, real: &str, durable: bool) -> Result<(), i32> {
        self.startup_check(real)?;
        if !durable {
            self.report(real, -6, true);
        }
        Ok(())
    }
    pub fn startup_check(&self, real: &str) -> Result<(), i32> {
        if !self.startup_idle() {
            return Err(-2);
        }
        if !title_path(real) {
            return Err(-1);
        }
        Ok(())
    }
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
    pub fn problem(&self) -> Option<Problem> {
        match &self.problem {
            Some(notice) => Some(Problem {
                sequence: notice.sequence,
                real: copy_path(&notice.real),
                code: notice.code,
                durability: notice.durability,
            }),
            None => None,
        }
    }
    pub fn acknowledge_durability(&mut self) {
        if let Some(notice) = &self.problem {
            if notice.durability {
                self.problem = None;
            }
        }
        for index in 0..self.pending.len() {
            if self.pending[index].problem {
                let real = copy_path(&self.pending[index].real);
                let code = self.pending[index].code;
                self.report(&real, code, false);
                return;
            }
        }
    }
    pub fn exit_retry(&mut self) {
        for pending in &mut self.pending {
            pending.retry = true;
        }
    }
    pub fn next_work(&mut self) -> Result<Option<Work>, i32> {
        if self.active.is_some() {
            return Err(-2);
        }
        for index in 0..self.entries.len() {
            if self.entries[index].phase != QUEUED {
                continue;
            }
            let token = self.next_id()?;
            let entry = &mut self.entries[index];
            entry.phase = COMMITTING;
            self.active = Some(Active {
                token,
                kind: COMMIT,
                slot: index,
            });
            return Ok(Some(Work {
                token,
                retry: false,
                real: copy_path(&entry.real),
                shadow: copy_path(&entry.shadow),
                flush: entry.flush,
                deleted: entry.deleted,
                history: self.history,
                writes: entry.writes,
                bytes: entry.bytes,
            }));
        }
        // Like the original worker's s_retry_due branch, select the retry batch
        // after all queued commits. A newly failed commit in that batch must
        // join the retries too, not wait until another title save closes.
        if self.retry_due {
            for pending in &mut self.pending {
                if pending.tries < 4 {
                    pending.retry = true;
                }
            }
            self.retry_due = false;
        }
        for index in 0..self.pending.len() {
            if !self.pending[index].retry {
                continue;
            }
            if let Some(entry) = self.entry_path(&self.pending[index].real) {
                if self.entries[entry].shadow == self.pending[index].preserved {
                    continue;
                }
            }
            let token = self.next_id()?;
            let pending = &mut self.pending[index];
            pending.retry = false;
            self.active = Some(Active {
                token,
                kind: RETRY,
                slot: index,
            });
            return Ok(Some(Work {
                token,
                retry: true,
                real: copy_path(&pending.real),
                shadow: copy_path(&pending.preserved),
                flush: None,
                deleted: false,
                history: false,
                writes: 0,
                bytes: 0,
            }));
        }
        Ok(None)
    }
    pub fn complete(&mut self, token: u64, result: Completion) -> Result<Cleanup, i32> {
        let active = self.active.as_ref().ok_or(-4)?;
        if active.token != token {
            return Err(-4);
        }
        let kind = active.kind;
        let slot = active.slot;
        let real = if kind == COMMIT {
            copy_path(&self.entries[slot].real)
        } else {
            copy_path(&self.pending[slot].real)
        };
        if let Completion::Failed { code, preserved } = &result {
            if !(-5..=-1).contains(code)
                || (preserved != &shadow_name(&real) && preserved != &pending_name(&real))
            {
                return Err(-1);
            }
            if kind == RETRY && preserved != &self.pending[slot].preserved {
                return Err(-1);
            }
        }
        let disposition = match &result {
            Completion::Published { .. } => 1,
            Completion::Unchanged {} => 2,
            Completion::Deleted {} => 3,
            Completion::Missing {} => 4,
            Completion::Failed { .. } => 5,
            Completion::RetryLater { .. } => 6,
            Completion::Abandoned {} => 7,
        };
        if kind == COMMIT && disposition == 4 {
            return Err(-1);
        }
        if kind == RETRY && (disposition == 2 || disposition == 3 || disposition == 7) {
            return Err(-1);
        }
        if let Completion::RetryLater { code } = &result {
            if kind != COMMIT || !(-5..=-1).contains(code) {
                return Err(-1);
            }
        }
        if kind == COMMIT {
            let deleted = self.entries[slot].deleted;
            if disposition == 3 && !deleted {
                return Err(-1);
            }
            if deleted
                && (disposition == 1 || disposition == 2 || disposition == 5 || disposition == 7)
            {
                return Err(-1);
            }
        }
        self.active = None;
        let mut obsolete = None;
        match result {
            Completion::Failed { code, preserved } => {
                let index = match self.pending_path(&real) {
                    Some(index) => index,
                    None => {
                        self.pending.push(Pending {
                            real: copy_path(&real),
                            preserved: copy_path(&preserved),
                            tries: 0,
                            code,
                            retry: false,
                            problem: false,
                        });
                        self.pending.len() - 1
                    }
                };
                let pending = &mut self.pending[index];
                if pending.preserved != preserved {
                    obsolete = Some(copy_path(&pending.preserved));
                }
                pending.preserved = preserved;
                pending.code = code;
                pending.tries = pending.tries.saturating_add(1);
                // Adding a newly failed save reports each failure >=4; retries
                // report only when crossing the original four-attempt boundary.
                if pending.tries == 4 || (kind == COMMIT && pending.tries > 4) {
                    pending.problem = true;
                    self.report(&real, code, false);
                }
            }
            Completion::Published { durable } => {
                if let Some(index) = self.pending_path(&real) {
                    let old = remove_stable(&mut self.pending, index)?.preserved;
                    if kind == COMMIT && old != self.entries[slot].shadow {
                        obsolete = Some(old);
                    }
                }
                if !durable {
                    self.report(&real, -6, true);
                }
            }
            Completion::Unchanged {} | Completion::Deleted {} => {
                if let Some(index) = self.pending_path(&real) {
                    obsolete = Some(remove_stable(&mut self.pending, index)?.preserved);
                }
            }
            Completion::Missing {} => {
                let _ = remove_stable(&mut self.pending, slot)?;
            }
            Completion::Abandoned {} => {}
            Completion::RetryLater { code } => {
                self.entries[slot].phase = QUEUED;
                self.entries[slot].flush = None;
                self.report(&real, code, false);
                return Ok(Cleanup { obsolete: None });
            }
        }
        if kind == COMMIT {
            self.entries[slot] = Entry::empty();
        }
        self.refresh_problem();
        Ok(Cleanup { obsolete })
    }
    // Startup has already compared timestamps, attempted publication and kept
    // a history snapshot where required. A newer still-blocked file gets one
    // ordinary retry on next close, plus the unconditional shutdown retry.
    pub fn recovered_pending(&mut self, real: &str, preserved: &str, code: i32) -> Result<(), i32> {
        if self.active.is_some() {
            return Err(-2);
        }
        for entry in &self.entries {
            if entry.phase != FREE {
                return Err(-2);
            }
        }
        if !title_path(real) || preserved != pending_name(real) || !(-5..=-1).contains(&code) {
            return Err(-1);
        }
        if self.pending_path(real).is_some() {
            return Err(-1);
        }
        if self.pending.len() >= 16 {
            return Err(-2);
        }
        self.pending.push(Pending {
            real: copy_path(&real),
            preserved: copy_path(&preserved),
            tries: 3,
            code,
            retry: false,
            problem: true,
        });
        self.report(real, code, false);
        Ok(())
    }
}
