// Port-owned serialized heap task. GamePKG owns the tasks, queues and RAM.
use crate::gamepkg;
use crate::heap;
use crate::memory;
fn service_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let part = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([part[0], part[1], part[2], part[3]]))
}
fn service_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let part = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        part[0], part[1], part[2], part[3], part[4], part[5], part[6], part[7],
    ]))
}
fn service_alive() -> Result<(), i32> {
    if gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
fn service_reply(
    reply: u64,
    sequence: u64,
    result: Result<u32, i32>,
    stats: [u32; 3],
) -> Result<(), i32> {
    let (status, value) = match result {
        Ok(value) => (0i32, value),
        Err(status) => (status, 0),
    };
    let mut bytes = [0u8; 32];
    bytes[..4].copy_from_slice(&u32::to_le_bytes(1));
    bytes[4..8].copy_from_slice(&i32::to_le_bytes(status));
    bytes[8..16].copy_from_slice(&u64::to_le_bytes(sequence));
    bytes[16..20].copy_from_slice(&u32::to_le_bytes(value));
    for number in 0..3usize {
        bytes[20 + number * 4..24 + number * 4].copy_from_slice(&u32::to_le_bytes(stats[number]));
    }
    gamepkg::local_mailbox_send(reply, &bytes)
}
fn service_receive(inbox: u64, bytes: &mut [u8]) -> Result<usize, i32> {
    loop {
        service_alive()?;
        match gamepkg::local_mailbox_receive(inbox, 1000, bytes) {
            Err(-2) => {}
            result => return result,
        }
    }
}
fn service_run(inbox: u64, clients: &mut [u64; 12]) -> Result<(), i32> {
    let mut bytes = [0u8; 48];
    let length = service_receive(inbox, &mut bytes)?;
    if length != 48
        || service_word(&bytes, 0)? != 1
        || service_word(&bytes, 4)? != 0
        || service_long(&bytes, 16)? != 1
    {
        return Err(-1);
    }
    let owner = service_long(&bytes, 8)?;
    if owner == 0 {
        return Err(-1);
    }
    let configured = service_configure(&bytes);
    let (mut ram, mut state) = match configured {
        Ok(values) => values,
        Err(code) => {
            service_reply(owner, 1, Err(code), [0; 3])?;
            return Err(code);
        }
    };
    service_reply(owner, 1, Ok(0), state.heap_stats())?;
    let mut sequences = [0u64; 12];
    let mut tasks = [0u64; 12];
    clients[0] = owner;
    sequences[0] = 1;
    loop {
        let length = service_receive(inbox, &mut bytes)?;
        // A malformed frame without a correlatable header terminates the task;
        // the input queue is revoked by heap_serve, waking bounded waiters.
        if length < 24 {
            return Err(-1);
        }
        let reply = service_long(&bytes, 8)?;
        let sequence = service_long(&bytes, 16)?;
        if reply == 0 || sequence == 0 {
            return Err(-1);
        }
        let mut slot = 12usize;
        for number in 0..12usize {
            if clients[number] == reply {
                slot = number;
                break;
            }
        }
        if length != 32 || service_word(&bytes, 0)? != 1 {
            service_reply(reply, sequence, Err(-1), state.heap_stats())?;
            continue;
        }
        let operation = service_word(&bytes, 4)?;
        let first = service_word(&bytes, 24)?;
        let second = service_word(&bytes, 28)?;
        if operation == 7 {
            if slot != 12 || sequence != 1 {
                service_reply(reply, sequence, Err(-1), state.heap_stats())?;
                continue;
            }
            let task = u64::from(first) | (u64::from(second) << 32);
            if task != 0 {
                match gamepkg::local_task_finished(task) {
                    Ok(false) => {}
                    Ok(true) => {
                        service_reply(reply, sequence, Err(-4), state.heap_stats())?;
                        continue;
                    }
                    Err(code) => {
                        service_reply(reply, sequence, Err(code), state.heap_stats())?;
                        continue;
                    }
                }
            }
            // Reap only completed/joined reviewed client tasks. Registration
            // is the bounded slow path; allocation does not scan task states.
            for number in 1..12usize {
                if clients[number] != 0 && tasks[number] != 0 {
                    let finished = match gamepkg::local_task_finished(tasks[number]) {
                        Ok(value) => value,
                        Err(-4) => true,
                        Err(code) => return Err(code),
                    };
                    if finished {
                        match gamepkg::local_mailbox_release(clients[number]) {
                            Ok(()) | Err(-4) => {}
                            Err(code) => return Err(code),
                        }
                        clients[number] = 0;
                        sequences[number] = 0;
                        tasks[number] = 0;
                    }
                }
            }
            for number in 1..12usize {
                if clients[number] == 0 {
                    slot = number;
                    break;
                }
            }
            if slot == 12 {
                service_reply(reply, sequence, Err(-2), state.heap_stats())?;
                continue;
            }
            // Registration touches no heap. A stale response queue fails before
            // membership publication; its ID is never silently treated as live.
            service_reply(reply, sequence, Ok(0), state.heap_stats())?;
            clients[slot] = reply;
            sequences[slot] = sequence;
            tasks[slot] = task;
            continue;
        }
        if slot == 12 {
            service_reply(reply, sequence, Err(-4), state.heap_stats())?;
            continue;
        }
        if sequences[slot].checked_add(1) != Some(sequence) {
            service_reply(reply, sequence, Err(-1), state.heap_stats())?;
            continue;
        }
        sequences[slot] = sequence;
        let result = match operation {
            1 => state.heap_allocate(&mut ram, first, second),
            2 => {
                if second != 0 {
                    Err(-1)
                } else {
                    match state.heap_free(&ram, first) {
                        Ok(()) => Ok(0),
                        Err(code) => Err(code),
                    }
                }
            }
            3 => {
                if second != 0 {
                    Err(-1)
                } else {
                    state.heap_size(&ram, first)
                }
            }
            4 | 5 | 6 => {
                if first != 0 || second != 0 {
                    Err(-1)
                } else if (operation == 4 && slot != 0) || (operation == 6 && slot == 0) {
                    Err(-4)
                } else if operation == 5 {
                    match ram.call_snapshot() {
                        Ok(_) => Ok(0),
                        Err(code) => Err(code),
                    }
                } else {
                    Ok(0)
                }
            }
            _ => Err(-1),
        };
        let accepted = result.is_ok();
        service_reply(reply, sequence, result, state.heap_stats())?;
        if accepted && operation == 4 {
            return Ok(());
        }
        if accepted && operation == 6 {
            clients[slot] = 0;
            sequences[slot] = 0;
            tasks[slot] = 0;
        }
    }
}
fn service_configure(bytes: &[u8]) -> Result<(memory::Memory, heap::Heap), i32> {
    let mut ram = memory::Memory::with_call_handle(service_long(bytes, 32)?)?;
    ram.attach(0, 67108864, service_long(bytes, 40)?, 0, true)?;
    let state = heap::Heap::heap_new(&ram, service_word(bytes, 24)?, service_word(bytes, 28)?)?;
    Ok((ram, state))
}
// Reviewed game task entry, called through a fixed host-generated task table.
pub fn heap_serve(inbox: u64) -> u64 {
    let mut clients = [0u64; 12];
    let result = service_run(inbox, &mut clients);
    let _ = gamepkg::local_mailbox_release(inbox);
    for number in 0..12usize {
        // A normal stop retains the owner's final acknowledgement. All other
        // client queues are revoked; a fatal exit also revokes the owner.
        if clients[number] != 0 && (number != 0 || result.is_err()) {
            let _ = gamepkg::local_mailbox_release(clients[number]);
        }
    }
    match result {
        Ok(()) => 0,
        Err(code) => code as i64 as u64,
    }
}
pub fn heap_inbox() -> Result<u64, i32> {
    gamepkg::local_mailbox_create(48, 32)
}
pub struct HeapClient {
    inbox: u64,
    job: u64,
    reply: u64,
    sequence: u64,
    active: bool,
    poisoned: bool,
}
impl HeapClient {
    pub fn heap_client(inbox: u64, job: u64) -> Result<Self, i32> {
        if inbox == 0 || job == 0 {
            return Err(-1);
        }
        Ok(Self {
            inbox,
            job,
            reply: gamepkg::local_mailbox_create(32, 1)?,
            sequence: 0,
            active: false,
            poisoned: false,
        })
    }
    fn service_call(
        &mut self,
        operation: u32,
        first: u32,
        second: u32,
        extra: &[u8],
    ) -> Result<(u32, [u32; 3]), i32> {
        if self.reply == 0 || self.poisoned {
            return Err(-4);
        }
        service_alive()?;
        let sequence = self.sequence.checked_add(1).ok_or(-3)?;
        let mut request = std::vec::Vec::new();
        request.extend(u32::to_le_bytes(1));
        request.extend(u32::to_le_bytes(operation));
        request.extend(u64::to_le_bytes(self.reply));
        request.extend(u64::to_le_bytes(sequence));
        request.extend(u32::to_le_bytes(first));
        request.extend(u32::to_le_bytes(second));
        request.extend(extra);
        // Busy here means this request was never accepted; sequence is retained.
        gamepkg::local_mailbox_send(self.inbox, &request)?;
        self.sequence = sequence;
        let reply = self.service_response(sequence);
        if reply.is_err() {
            self.poisoned = true;
        }
        let bytes = reply?;
        let status = service_word(&bytes, 4)? as i32;
        if !(-5..=0).contains(&status) {
            self.poisoned = true;
            return Err(-3);
        }
        if status != 0 {
            return Err(status);
        }
        Ok((
            service_word(&bytes, 16)?,
            [
                service_word(&bytes, 20)?,
                service_word(&bytes, 24)?,
                service_word(&bytes, 28)?,
            ],
        ))
    }
    fn service_response(&self, sequence: u64) -> Result<[u8; 32], i32> {
        let mut bytes = [0u8; 32];
        loop {
            service_alive()?;
            match gamepkg::local_mailbox_receive(self.reply, 1000, &mut bytes) {
                Ok(32) => {
                    if service_word(&bytes, 0)? != 1 || service_long(&bytes, 8)? != sequence {
                        return Err(-3);
                    }
                    return Ok(bytes);
                }
                Ok(_) => return Err(-3),
                Err(-2) => {
                    // The supervisor retains the join result. This snapshot
                    // neither consumes the task nor resumes a startup gate.
                    match gamepkg::local_task_finished(self.job) {
                        Ok(false) => {}
                        Ok(true) => return Err(-3),
                        Err(code) => return Err(code),
                    }
                }
                Err(code) => return Err(code),
            }
        }
    }
    pub fn heap_initialize(
        &mut self,
        record: u64,
        window: u64,
        base: u32,
        top: u32,
    ) -> Result<(), i32> {
        if self.sequence != 0 || self.active {
            return Err(-1);
        }
        let mut extra = std::vec::Vec::new();
        extra.extend(u64::to_le_bytes(record));
        extra.extend(u64::to_le_bytes(window));
        let (result, _) = self.service_call(0, base, top, &extra)?;
        if result != 0 {
            self.poisoned = true;
            return Err(-3);
        }
        self.active = true;
        Ok(())
    }
    pub fn heap_register(&mut self) -> Result<(), i32> {
        if self.sequence != 0 || self.active {
            return Err(-1);
        }
        let task = gamepkg::local_task_status()?.0;
        let (result, _) = self.service_call(7, task as u32, (task >> 32) as u32, &[])?;
        if result != 0 {
            self.poisoned = true;
            return Err(-3);
        }
        self.active = true;
        Ok(())
    }
    pub fn heap_alloc(&mut self, size: u32, alignment: u32) -> Result<u32, i32> {
        if !self.active {
            return Err(-4);
        }
        Ok(self.service_call(1, size, alignment, &[])?.0)
    }
    pub fn heap_release_block(&mut self, address: u32) -> Result<(), i32> {
        if !self.active {
            return Err(-4);
        }
        let (result, _) = self.service_call(2, address, 0, &[])?;
        if result != 0 {
            self.poisoned = true;
            return Err(-3);
        }
        Ok(())
    }
    pub fn heap_query(&mut self, address: u32) -> Result<u32, i32> {
        if !self.active {
            return Err(-4);
        }
        Ok(self.service_call(3, address, 0, &[])?.0)
    }
    pub fn heap_status(&mut self) -> Result<[u32; 3], i32> {
        if !self.active {
            return Err(-4);
        }
        let (result, stats) = self.service_call(5, 0, 0, &[])?;
        if result != 0 {
            self.poisoned = true;
            return Err(-3);
        }
        Ok(stats)
    }
    pub fn heap_unregister(&mut self) -> Result<(), i32> {
        if !self.active {
            return Err(-4);
        }
        let (result, _) = self.service_call(6, 0, 0, &[])?;
        if result != 0 {
            self.poisoned = true;
            return Err(-3);
        }
        self.active = false;
        Ok(())
    }
    pub fn heap_stop(&mut self) -> Result<(), i32> {
        if !self.active {
            return Err(-4);
        }
        let (result, _) = self.service_call(4, 0, 0, &[])?;
        if result != 0 {
            self.poisoned = true;
            return Err(-3);
        }
        self.active = false;
        Ok(())
    }
    // Explicit queue lifecycle. After an accepted but failed/abandoned request,
    // its mutation result may be unknown: abort; never replay that mutation.
    pub fn heap_abandon(&mut self) -> Result<(), i32> {
        if self.reply == 0 {
            return Ok(());
        }
        let reply = self.reply;
        self.reply = 0;
        self.active = false;
        self.poisoned = true;
        match gamepkg::local_mailbox_release(reply) {
            Ok(()) | Err(-4) => Ok(()),
            Err(code) => Err(code),
        }
    }
}
