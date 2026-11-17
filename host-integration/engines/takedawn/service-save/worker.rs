// Port-owned save actor. The only synchronization and I/O are GamePKG calls.
// The launcher must register run as a reviewed static task before source build.
use crate::gamepkg;
use crate::save_adapter;
use crate::save_history;
use crate::save_paths;
use crate::save_recovery;

pub const OPEN_WRITE: u32 = 1;
pub const OPEN_READ: u32 = 2;
pub const WRITE: u32 = 3;
pub const RESIZE: u32 = 4;
pub const CLOSE: u32 = 5;
pub const DELETE_ON_CLOSE: u32 = 6;
pub const STATUS: u32 = 7;
pub const STAMP: u32 = 8;
pub const SHUTDOWN: u32 = 9;
pub const OPEN_FILE: u32 = 10;
pub const QUERY_FILE: u32 = 11;
pub const FLUSH_FILE: u32 = 12;

pub struct Reply {
    pub sequence: u64,
    pub status: i32,
    pub bytes: std::vec::Vec<u8>,
}
pub struct Connection {
    inbox: u64,
    replies: u64,
    task: u64,
    next: u64,
    waiting: Option<u64>,
    closing: bool,
    joined: Option<u64>,
}
pub struct SaveWorker {
    grant: u64,
    title: std::string::String,
    stamp: std::string::String,
    guard: save_adapter::SaveGuard,
    active: Option<save_adapter::Job>,
    obsolete: Option<std::string::String>,
    reads: std::vec::Vec<u64>,
    writes: std::vec::Vec<[u64; 2]>,
    pub history_error: i32,
    pub io_error: i32,
    pub recovery_report: save_recovery::Report,
    closing: bool,
}
fn worker_text(text: &str) -> std::string::String {
    let mut owned = std::string::String::new();
    owned.push_str(text);
    owned
}
fn worker_stamp_valid(text: &str) -> bool {
    if text.len() != 15 {
        return false;
    }
    for (index, byte) in text.bytes().enumerate() {
        if index == 8 {
            if byte != b'-' {
                return false;
            }
        } else if !byte.is_ascii_digit() {
            return false;
        }
    }
    true
}
fn worker_u64(bytes: &[u8]) -> Result<u64, i32> {
    if bytes.len() < 8 {
        return Err(-1);
    }
    Ok(u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ]))
}
fn worker_string(bytes: &[u8]) -> Result<&str, i32> {
    gamepkg::text_from_utf8(bytes)
}

fn worker_remove_handle<T>(items: &mut std::vec::Vec<T>, index: usize) -> Result<(), i32> {
    if index >= items.len() {
        return Err(-1);
    }
    let last = items.pop().ok_or(-1)?;
    if let Some(slot) = items.get_mut(index) {
        *slot = last;
    }
    Ok(())
}
fn worker_alive() -> Result<(), i32> {
    let (_, cancelled) = gamepkg::local_task_status()?;
    if cancelled {
        Err(-4)
    } else {
        Ok(())
    }
}
fn worker_pause(event: u64) -> Result<(), i32> {
    worker_alive()?;
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(event));
    request.extend(u32::to_le_bytes(20));
    match gamepkg::local_call(23, &request, &mut []) {
        Ok(0) | Err(-2) => worker_alive(),
        Ok(_) => Err(-3),
        Err(error) => Err(error),
    }
}
fn worker_send(save_reply_queue: u64, bytes: &[u8], event: u64) -> Result<(), i32> {
    let (now, _) = gamepkg::monotonic()?;
    let end = now.saturating_add(1000000000);
    loop {
        worker_alive()?;
        match gamepkg::local_mailbox_send(save_reply_queue, bytes) {
            Ok(()) => return Ok(()),
            Err(-2) => {
                if gamepkg::monotonic()?.0 >= end {
                    return Err(-2);
                }
                worker_pause(event)?;
            }
            Err(error) => return Err(error),
        }
    }
}
fn worker_reply(sequence: u64, status: i32, payload: &[u8]) -> std::vec::Vec<u8> {
    let mut encoded = std::vec::Vec::new();
    encoded.extend(u64::to_le_bytes(sequence));
    encoded.extend(i32::to_le_bytes(status));
    encoded.extend(payload);
    encoded
}
impl SaveWorker {
    pub fn new(grant: u64, title: &str, stamp: &str) -> Result<Self, i32> {
        if !worker_stamp_valid(stamp) || title.len() != 8 {
            return Err(-1);
        }
        for byte in title.bytes() {
            if !byte.is_ascii_hexdigit() {
                return Err(-1);
            }
        }
        let mut save_granted = false;
        for item in gamepkg::grants()? {
            if item.id == grant && item.kind == 2 {
                save_granted = true;
            }
        }
        if !save_granted {
            return Err(-4);
        }
        let mut guard = save_adapter::SaveGuard::new(grant);
        let startup_report;
        {
            let mut startup = save_recovery::Recovery::begin(&mut guard, grant, title)?;
            loop {
                match startup.step()? {
                    save_recovery::RecoveryStep::Progress {} => {}
                    save_recovery::RecoveryStep::Done { report: completed } => {
                        startup_report = completed;
                        break;
                    }
                    save_recovery::RecoveryStep::History { real, pending, .. } => {
                        match save_history::archive_pending(grant, title, stamp, &real, &pending) {
                            Ok(proof) => startup.history_done(&proof)?,
                            Err(error) => startup.history_failed(error)?,
                        }
                    }
                }
            }
        }
        guard.set_history(true);
        Ok(Self {
            grant,
            title: worker_text(title),
            stamp: worker_text(stamp),
            guard,
            active: None,
            obsolete: None,
            reads: std::vec::Vec::new(),
            writes: std::vec::Vec::new(),
            history_error: 0,
            io_error: 0,
            recovery_report: startup_report,
            closing: false,
        })
    }
    fn check_path(&self, text: &str) -> Result<(), i32> {
        let parts = save_paths::save_path_segments(text)?;
        if parts.len() < 3 || (parts[0] != "UDATA" && parts[0] != "TDATA") || parts[1] != self.title
        {
            return Err(-4);
        }
        Ok(())
    }
    pub fn status_bytes(&self) -> std::vec::Vec<u8> {
        let mut bytes = std::vec::Vec::new();
        bytes.extend(u32::to_le_bytes(self.guard.busy() as u32));
        bytes.extend(u32::to_le_bytes(self.guard.pending_count() as u32));
        bytes.extend(i32::to_le_bytes(self.io_error));
        bytes.extend(i32::to_le_bytes(self.history_error));
        let mut problem_sequence = 0u64;
        let mut problem_code = 0i32;
        let mut uncertain = false;
        if let Some(observed_problem) = self.guard.problem() {
            problem_sequence = observed_problem.sequence;
            problem_code = observed_problem.code;
            uncertain = observed_problem.durability;
        }
        bytes.extend(u64::to_le_bytes(problem_sequence));
        bytes.extend(i32::to_le_bytes(problem_code));
        bytes.extend(u32::to_le_bytes(u32::from(uncertain)));
        bytes.extend(u32::to_le_bytes(self.reads.len() as u32));
        bytes.extend(u32::to_le_bytes(self.writes.len() as u32));
        for count in [
            self.recovery_report.unfinished_removed,
            self.recovery_report.recovered,
            self.recovery_report.archived,
            self.recovery_report.pending,
            self.recovery_report.older_kept,
            self.recovery_report.archive_errors,
            self.recovery_report.uncertain,
        ] {
            bytes.extend(u32::to_le_bytes(count));
        }
        bytes
    }
    // Executes one exact job step. On failure the same Job/cleanup is retained.
    pub fn save_worker_tick(&mut self) -> Result<bool, i32> {
        if let Some(path) = &self.obsolete {
            if gamepkg::file_exists(self.grant, path)? {
                gamepkg::file_remove(self.grant, path)?;
            }
            self.obsolete = None;
            return Ok(true);
        }
        if self.active.is_none() {
            self.active = self.guard.begin_job()?;
        }
        let job = match self.active.as_mut() {
            Some(job) => job,
            None => return Ok(false),
        };
        match job.step()? {
            save_adapter::Step::History { .. } => {
                let archived =
                    save_history::archive_before_replace(self.grant, &self.title, &self.stamp);
                let result = match archived {
                    Ok(_) => Ok(()),
                    Err(error) => Err(error),
                };
                job.history_finished(result)?;
            }
            save_adapter::Step::Complete { finished } => {
                if let Some(error) = finished.history_error {
                    self.history_error = error;
                }
                if !finished.preservation_durable {
                    self.io_error = -6;
                }
                self.obsolete = self.guard.finish(finished)?.obsolete;
                self.active = None;
            }
        }
        Ok(true)
    }
    pub fn has_work(&self) -> bool {
        self.active.is_some() || self.obsolete.is_some() || self.guard.busy() != 0
    }
    fn flush_write_through(&mut self, file: u64) -> Result<(), i32> {
        for written_handle in &self.writes {
            if written_handle[0] == file && written_handle[1] == 1 {
                return self.sync_save_writer(file);
            }
        }
        Ok(())
    }

    fn sync_save_writer(&mut self, file: u64) -> Result<(), i32> {
        let flushed = gamepkg::file_sync(file);
        if let Err(error) = flushed {
            self.io_error = error;
        }
        flushed
    }

    fn flush_save_handle(&mut self, payload: &[u8]) -> Result<std::vec::Vec<u8>, i32> {
        if payload.len() != 8 {
            return Err(-1);
        }
        let file = worker_u64(payload)?;
        for registered in &self.writes {
            if registered[0] == file {
                self.sync_save_writer(file)?;
                return Ok(std::vec::Vec::new());
            }
        }
        Err(-4)
    }

    fn open_file_message(&mut self, payload: &[u8]) -> Result<std::vec::Vec<u8>, i32> {
        if payload.len() < 9 {
            return Err(-1);
        }
        let flags = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let disposition = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
        if flags & !0x2603 != 0 {
            return Err(-1);
        }
        let writable = flags & 1 != 0;
        let write_through = flags & 2 != 0;
        let real = worker_string(&payload[8..])?;
        self.check_path(real)?;
        if !writable && self.reads.len() >= 64 {
            return Err(-2);
        }
        let (file, information) =
            self.guard
                .open_regular(real, disposition, writable, write_through, flags >> 8)?;
        if file != 0 {
            if writable {
                self.writes.push([file, u64::from(write_through)]);
            } else {
                self.reads.push(file);
            }
        }
        Ok([
            u64::to_le_bytes(file).as_slice(),
            &u32::to_le_bytes(information),
        ]
        .concat())
    }

    fn regular_file_information(&self, payload: &[u8]) -> Result<std::vec::Vec<u8>, i32> {
        if payload.len() != 8 {
            return Err(-1);
        }
        let observed_file = worker_u64(payload)?;
        let mut owned_file = self.reads.contains(&observed_file);
        for write_slot in &self.writes {
            owned_file |= write_slot[0] == observed_file;
        }
        if !owned_file {
            return Err(-4);
        }
        let info = gamepkg::save_file_info(observed_file)?;
        let allocation = info.length.checked_add(16383).ok_or(-1)? & !16383u64;
        if allocation > i64::MAX as u64 {
            return Err(-1);
        }
        let mut reported = std::vec::Vec::new();
        for stamp in info.times {
            reported.extend(u64::to_le_bytes(stamp));
        }
        reported.extend(u64::to_le_bytes(allocation));
        reported.extend(u64::to_le_bytes(info.length));
        reported.extend(u32::to_le_bytes(info.attributes));
        Ok(reported)
    }

    pub fn save_actor_command(
        &mut self,
        operation: u32,
        payload: &[u8],
    ) -> Result<std::vec::Vec<u8>, i32> {
        if self.closing && operation != STATUS {
            return Err(-4);
        }
        match operation {
            OPEN_WRITE | OPEN_READ => {
                let path = worker_string(payload)?;
                self.check_path(path)?;
                let handle;
                if operation == OPEN_WRITE {
                    handle = self.guard.open_write(path)?;
                    self.writes.push([handle, 0]);
                } else {
                    if self.reads.len() >= 64 {
                        return Err(-2);
                    }
                    handle = self.guard.open_read(path)?;
                    self.reads.push(handle);
                }
                Ok(u64::to_le_bytes(handle).to_vec())
            }
            OPEN_FILE => self.open_file_message(payload),
            QUERY_FILE => self.regular_file_information(payload),
            FLUSH_FILE => self.flush_save_handle(payload),
            WRITE => {
                if payload.len() < 16 {
                    return Err(-1);
                }
                let handle = worker_u64(payload)?;
                let offset = worker_u64(&payload[8..])?;
                self.guard.write_at(handle, offset, &payload[16..])?;
                self.flush_write_through(handle)?;
                Ok(std::vec::Vec::new())
            }
            RESIZE => {
                if payload.len() != 16 {
                    return Err(-1);
                }
                let target = worker_u64(payload)?;
                self.guard.resize(target, worker_u64(&payload[8..])?)?;
                self.flush_write_through(target)?;
                Ok(std::vec::Vec::new())
            }
            CLOSE => {
                if payload.len() != 8 {
                    return Err(-1);
                }
                let handle = worker_u64(payload)?;
                let mut found = None;
                for (index, id) in self.reads.iter().enumerate() {
                    if *id == handle {
                        found = Some(index);
                        break;
                    }
                }
                if let Some(index) = found {
                    gamepkg::file_close(handle)?;
                    worker_remove_handle(&mut self.reads, index)?;
                } else {
                    self.guard.save_guard_close(handle)?;
                    let mut position = 0usize;
                    while position < self.writes.len() {
                        if self.writes[position][0] == handle {
                            worker_remove_handle(&mut self.writes, position)?;
                            break;
                        }
                        position += 1;
                    }
                }
                Ok(std::vec::Vec::new())
            }
            DELETE_ON_CLOSE => {
                if payload.len() != 8 {
                    return Err(-1);
                }
                self.guard.delete_on_close(worker_u64(payload)?)?;
                Ok(std::vec::Vec::new())
            }
            STATUS => {
                if !payload.is_empty() {
                    return Err(-1);
                }
                Ok(self.status_bytes())
            }
            STAMP => {
                let text = worker_string(payload)?;
                if !worker_stamp_valid(text) {
                    return Err(-1);
                }
                // Never change the label in the middle of an extracted job.
                if self.active.is_some() {
                    return Err(-2);
                }
                self.stamp = worker_text(text);
                Ok(std::vec::Vec::new())
            }
            _ => Err(-1),
        }
    }
    fn drain_until(&mut self, event: u64, end: u64) -> Result<(), i32> {
        loop {
            worker_alive()?;
            match self.save_worker_tick() {
                Ok(false) => return Ok(()),
                Ok(true) => {}
                Err(error) => {
                    self.io_error = error;
                    worker_pause(event)?;
                }
            }
            if gamepkg::monotonic()?.0 >= end {
                return Ok(());
            }
        }
    }
    fn shutdown(&mut self, event: u64) -> Result<bool, i32> {
        self.closing = true;
        let (now, _) = gamepkg::monotonic()?;
        self.drain_until(event, now.saturating_add(2000000000))?;
        // Always schedule the final pending batch, including when the initial
        // drain was blocked. This does not discard an incomplete active Job.
        self.guard.exit_retry();
        let (now, _) = gamepkg::monotonic()?;
        self.drain_until(event, now.saturating_add(2000000000))?;
        Ok(!self.has_work()
            && self.guard.pending_count() == 0
            && self.reads.is_empty()
            && self.writes.is_empty()
            && self.io_error == 0
            && self.history_error == 0
            && self.guard.problem().is_none()
            && self.recovery_report.older_kept == 0
            && self.recovery_report.archive_errors == 0
            && self.recovery_report.uncertain == 0)
    }
}
fn worker_loop(inbox: u64, event: u64) -> Result<u64, i32> {
    let mut buffer = std::vec::Vec::new();
    buffer.resize(65536, 0u8);
    let length = gamepkg::local_mailbox_receive(inbox, 1000, &mut buffer)?;
    if length != 51 || buffer[0..4] != [0, 0, 0, 0] || worker_u64(&buffer[4..12])? != 1 {
        return Err(-1);
    }
    let replies = worker_u64(&buffer[12..20])?;
    let grant = worker_u64(&buffer[20..28])?;
    let title = worker_string(&buffer[28..36])?;
    let stamp = worker_string(&buffer[36..51])?;
    let mut owner = match SaveWorker::new(grant, title, stamp) {
        Ok(owner) => owner,
        Err(error) => {
            worker_send(replies, &worker_reply(1, error, &[]), event)?;
            return Ok(1);
        }
    };
    worker_send(replies, &worker_reply(1, 0, &owner.status_bytes()), event)?;
    let mut expected = 2u64;
    let mut work_available = false;
    loop {
        worker_alive()?;
        let timeout = if work_available { 0 } else { 20 };
        match gamepkg::local_mailbox_receive(inbox, timeout, &mut buffer) {
            Ok(size) => {
                if size < 12 {
                    return Err(-1);
                }
                let operation = u32::from_le_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]);
                let sequence = worker_u64(&buffer[4..12])?;
                if sequence != expected {
                    return Err(-1);
                }
                expected = expected.checked_add(1).ok_or(-1)?;
                let payload = &buffer[12..size];
                if operation == SHUTDOWN && payload.is_empty() {
                    let settled = owner.shutdown(event)?;
                    let status = if settled { 0 } else { -2 };
                    worker_send(
                        replies,
                        &worker_reply(sequence, status, &owner.status_bytes()),
                        event,
                    )?;
                    return Ok(u64::from(!settled));
                }
                let packet = match owner.save_actor_command(operation, payload) {
                    Ok(bytes) => worker_reply(sequence, 0, &bytes),
                    Err(error) => worker_reply(sequence, error, &[]),
                };
                // In particular CLOSE acknowledgement precedes any job flush,
                // history or publication. It only acknowledges title close.
                worker_send(replies, &packet, event)?;
            }
            Err(-2) => {}
            Err(error) => return Err(error),
        }
        match owner.save_worker_tick() {
            Ok(advanced) => work_available = advanced,
            Err(error) => {
                owner.io_error = error;
                work_available = false;
                worker_pause(event)?;
            }
        }
    }
}
pub fn run(inbox: u64) -> u64 {
    let mut reply = [0u8; 8];
    if gamepkg::local_call(20, &[0u8; 8], &mut reply) != Ok(8) {
        return 2;
    }
    let event = u64::from_le_bytes(reply);
    let result = match worker_loop(inbox, event) {
        Ok(value) => value,
        Err(_) => 2,
    };
    let _ = gamepkg::local_call(25, &u64::to_le_bytes(event), &mut []);
    result
}
impl Connection {
    // entry is a host-reviewed static task index, never a function pointer.
    // The initial recovery result is asynchronous: poll sequence 1 before use.
    pub fn launch_save_actor(
        entry: u32,
        grant: u64,
        title: &str,
        stamp: &str,
    ) -> Result<Self, i32> {
        if title.len() != 8 || !worker_stamp_valid(stamp) {
            return Err(-1);
        }
        for byte in title.bytes() {
            if !byte.is_ascii_hexdigit() {
                return Err(-1);
            }
        }
        let inbox = gamepkg::local_mailbox_create(65536, 8)?;
        let replies = match gamepkg::local_mailbox_create(65536, 4) {
            Ok(id) => id,
            Err(error) => {
                let _ = gamepkg::local_mailbox_release(inbox);
                return Err(error);
            }
        };
        let mut config = std::vec::Vec::new();
        config.extend(u32::to_le_bytes(0));
        config.extend(u64::to_le_bytes(1));
        config.extend(u64::to_le_bytes(replies));
        config.extend(u64::to_le_bytes(grant));
        config.extend(title.as_bytes());
        config.extend(stamp.as_bytes());
        if let Err(error) = gamepkg::local_mailbox_send(inbox, &config) {
            let _ = gamepkg::local_mailbox_release(inbox);
            let _ = gamepkg::local_mailbox_release(replies);
            return Err(error);
        }
        let mut request = std::vec::Vec::new();
        request.extend(u32::to_le_bytes(entry));
        request.extend(u64::to_le_bytes(inbox));
        request.extend(u32::to_le_bytes(0));
        let mut id = [0u8; 8];
        match gamepkg::local_call(10, &request, &mut id) {
            Ok(8) => Ok(Self {
                inbox,
                replies,
                task: u64::from_le_bytes(id),
                next: 2,
                waiting: Some(1),
                closing: false,
                joined: None,
            }),
            result => {
                let _ = gamepkg::local_mailbox_release(inbox);
                let _ = gamepkg::local_mailbox_release(replies);
                match result {
                    Err(error) => Err(error),
                    _ => Err(-3),
                }
            }
        }
    }
    pub fn submit(&mut self, operation: u32, payload: &[u8]) -> Result<u64, i32> {
        if self.closing || self.task == 0 {
            return Err(-4);
        }
        if self.waiting.is_some() {
            return Err(-2);
        }
        if payload.len() > 65524 || !(1..=FLUSH_FILE).contains(&operation) {
            return Err(-1);
        }
        if operation == SHUTDOWN && !payload.is_empty() {
            return Err(-1);
        }
        let next = self.next.checked_add(1).ok_or(-1)?;
        let mut packet = std::vec::Vec::new();
        packet.extend(u32::to_le_bytes(operation));
        packet.extend(u64::to_le_bytes(self.next));
        packet.extend(payload);
        gamepkg::local_mailbox_send(self.inbox, &packet)?;
        let sequence = self.next;
        self.next = next;
        self.waiting = Some(sequence);
        if operation == SHUTDOWN {
            self.closing = true;
        }
        Ok(sequence)
    }
    pub fn poll(&mut self, milliseconds: u32) -> Result<Option<Reply>, i32> {
        let sequence = match self.waiting {
            Some(value) => value,
            None => return Err(-4),
        };
        let mut packet = std::vec::Vec::new();
        packet.resize(65536, 0u8);
        let length = match gamepkg::local_mailbox_receive(self.replies, milliseconds, &mut packet) {
            Ok(length) => length,
            Err(-2) => return Ok(None),
            Err(error) => return Err(error),
        };
        if length < 12 || worker_u64(&packet)? != sequence {
            return Err(-3);
        }
        let status = i32::from_le_bytes([packet[8], packet[9], packet[10], packet[11]]);
        if !(-6..=0).contains(&status) {
            return Err(-3);
        }
        if sequence == 1 && status != 0 {
            self.closing = true;
        }
        self.waiting = None;
        Ok(Some(Reply {
            sequence,
            status,
            bytes: packet[12..length].to_vec(),
        }))
    }
    pub fn cancel_save_actor(&mut self) -> Result<(), i32> {
        if self.task == 0 {
            return Err(-4);
        }
        gamepkg::local_call(12, &u64::to_le_bytes(self.task), &mut [])?;
        self.closing = true;
        Ok(())
    }
    pub fn task_id(&self) -> u64 {
        self.task
    }
    pub fn accept_join(&mut self, joined_task: &gamepkg::LocalTaskJoin) -> Result<(), i32> {
        if !self.closing || self.task == 0 || joined_task.task_id() != self.task {
            return Err(-4);
        }
        self.joined = Some(joined_task.result());
        self.task = 0;
        Ok(())
    }
    // Nonblocking join; release queues only after the worker has really exited.
    // Cancellation/timeout never constitute a successful durable shutdown.
    pub fn finish(&mut self) -> Result<Option<u64>, i32> {
        if !self.closing {
            return Err(-4);
        }
        if self.task != 0 {
            match gamepkg::local_task_join(self.task) {
                Ok(joined_task) => self.accept_join(&joined_task)?,
                Err(-2) => return Ok(None),
                Err(error) => return Err(error),
            }
        }
        if self.inbox != 0 {
            gamepkg::local_mailbox_release(self.inbox)?;
            self.inbox = 0;
        }
        if self.replies != 0 {
            gamepkg::local_mailbox_release(self.replies)?;
            self.replies = 0;
        }
        Ok(self.joined)
    }
}
