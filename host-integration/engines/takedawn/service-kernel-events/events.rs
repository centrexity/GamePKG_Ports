// Checked game adapter. All native events remain opaque GamePKG-owned objects.
use crate::gamepkg;
use crate::handles;
use crate::kernel_pool;
use crate::memory;
pub struct Events {
    root: u64,
    services: u64,
    table: u64,
    record: u64,
    owner: u64,
    owning: bool,
    close_error: i32,
}
pub struct EventCreation {
    root: u64,
    services: u64,
    table: u64,
    record: u64,
    owner: u64,
    task: u64,
    row: usize,
    event: u64,
    token: u32,
    phase: u32,
    attempted_create: bool,
    attempted_publish: bool,
    released: bool,
    cleared: bool,
    closed: bool,
    guard: handles::HandleGuard,
}
fn events_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let p = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7],
    ]))
}
fn events_put(b: &mut [u8], at: usize, value: u64) -> Result<(), i32> {
    b.get_mut(at..at + 8)
        .ok_or(-1)?
        .copy_from_slice(&u64::to_le_bytes(value));
    Ok(())
}
fn events_live(ram: &memory::Memory, root: u64, services: u64) -> Result<(), i32> {
    if root == 0 || ram.call_handle() != root || services == 0 || ram.services_handle()? != services
    {
        return Err(-4);
    }
    kernel_pool::kernel_service_values(ram)?;
    if ram.services_handle()? != services || gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    Ok(())
}
fn events_record(
    record: u64,
    root: u64,
    services: u64,
    table: u64,
    owner: u64,
) -> Result<(u64, std::vec::Vec<u8>), i32> {
    let (version, b) = gamepkg::local_record_snapshot(record, 2112)?;
    if b.len() != 2112
        || events_word(&b, 0)? != 0x31445645
        || events_word(&b, 40)? > 1
        || events_word(&b, 48)? != 0
        || events_word(&b, 56)? != 0
    {
        return Err(-1);
    }
    if root == 0
        || services == 0
        || table == 0
        || owner == u64::MAX
        || events_word(&b, 8)? != root
        || events_word(&b, 16)? != services
        || events_word(&b, 24)? != table
        || events_word(&b, 32)? != owner
    {
        return Err(-4);
    }
    for row in 0..128usize {
        let id = events_word(&b, 64 + row * 16)?;
        let state = events_word(&b, 72 + row * 16)?;
        if state > 3 || ((state < 2) != (id == 0)) {
            return Err(-1);
        }
        if id != 0 {
            for before in 0..row {
                if events_word(&b, 64 + before * 16)? == id {
                    return Err(-1);
                }
            }
        }
    }
    Ok((version, b))
}
fn events_commit(record: u64, version: u64, b: &[u8]) -> Result<(), i32> {
    match gamepkg::local_record_compare_exchange(record, version, b) {
        Ok(_) => Ok(()),
        Err(-2) => Err(-3),
        Err(code) => Err(code),
    }
}
fn events_release(id: u64) -> Result<(), i32> {
    if gamepkg::local_call(25, &u64::to_le_bytes(id), &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
fn events_signal(id: u64, value: bool) -> Result<(), i32> {
    gamepkg::local_event_state(id)?;
    if gamepkg::local_call(
        22,
        &[
            u64::to_le_bytes(id).as_slice(),
            &u32::to_le_bytes(u32::from(value)),
        ]
        .concat(),
        &mut [],
    )? != 0
    {
        return Err(-3);
    }
    Ok(())
}
impl Events {
    pub fn events_new(ram: &memory::Memory, map: &handles::Handles) -> Result<Self, i32> {
        let root = ram.call_handle();
        let services = ram.services_handle()?;
        events_live(ram, root, services)?;
        // Only the table creator can own its domain and post-revocation cleanup.
        let mut guard = map.handles_cleanup_lock(ram)?;
        guard.handles_guard_close(ram)?;
        events_live(ram, root, services)?;
        let owner = gamepkg::local_task_status()?.0;
        let mut b = std::vec::Vec::new();
        b.resize(2112, 0);
        for (at, value) in [
            (0usize, 0x31445645u64),
            (8, root),
            (16, services),
            (24, map.handles_id()),
            (32, owner),
        ] {
            events_put(&mut b, at, value)?;
        }
        let record = gamepkg::local_record_create(&b)?;
        Ok(Self {
            root,
            services,
            table: map.handles_id(),
            record,
            owner,
            owning: true,
            close_error: 0,
        })
    }
    pub fn events_id(&self) -> u64 {
        self.record
    }
    pub fn events_closed(&self) -> bool {
        self.record == 0
    }
    pub fn events_borrow(
        ram: &memory::Memory,
        map: &handles::Handles,
        record: u64,
    ) -> Result<Self, i32> {
        let (_, b) = gamepkg::local_record_snapshot(record, 2112)?;
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            table: map.handles_id(),
            record,
            owner: events_word(&b, 32)?,
            owning: false,
            close_error: 0,
        };
        view.events_snapshot(ram, map)?;
        Ok(view)
    }
    fn events_snapshot(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
    ) -> Result<(u64, std::vec::Vec<u8>), i32> {
        events_live(ram, self.root, self.services)?;
        if map.handles_id() != self.table {
            return Err(-4);
        }
        let snapshot = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&snapshot.1, 40)? != 0 {
            return Err(-4);
        }
        Ok(snapshot)
    }
    pub fn events_begin(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
    ) -> Result<EventCreation, i32> {
        let task = gamepkg::local_task_status()?.0;
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_reserve(ram, map);
        let row = match result {
            Ok(row) => row,
            Err(code) => {
                let _ = guard.handles_guard_close(ram);
                return Err(code);
            }
        };
        // No fallible operation follows the accepted reservation before its owner is returned.
        Ok(EventCreation {
            root: self.root,
            services: self.services,
            table: self.table,
            record: self.record,
            owner: self.owner,
            task,
            row,
            event: 0,
            token: 0,
            phase: 0,
            attempted_create: false,
            attempted_publish: false,
            released: false,
            cleared: false,
            closed: false,
            guard,
        })
    }
    fn events_reserve(&self, ram: &memory::Memory, map: &handles::Handles) -> Result<usize, i32> {
        let (version, mut b) = self.events_snapshot(ram, map)?;
        for row in 0..128usize {
            if events_word(&b, 72 + row * 16)? == 0 {
                events_put(&mut b, 72 + row * 16, 1)?;
                events_commit(self.record, version, &b)?;
                return Ok(row);
            }
        }
        Err(-2)
    }
    fn events_lookup_locked(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        guard: &handles::HandleGuard,
        token: u32,
    ) -> Result<(usize, u64), i32> {
        let (_, b) = self.events_snapshot(ram, map)?;
        let value = guard.handle_guard_lookup(ram, token)?.ok_or(-4)?;
        if value[1] != 1 {
            return Err(-1);
        }
        for row in 0..128usize {
            if events_word(&b, 64 + row * 16)? == value[0] {
                if events_word(&b, 72 + row * 16)? != 2 {
                    return Err(-2);
                }
                gamepkg::local_event_state(value[0])?;
                return Ok((row, value[0]));
            }
        }
        Err(-4)
    }
    pub fn events_state(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        token: u32,
    ) -> Result<[bool; 2], i32> {
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_state_locked(ram, map, &guard, token);
        let closed = guard.handles_guard_close(ram);
        let value = result?;
        closed?;
        Ok(value)
    }
    fn events_state_locked(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        guard: &handles::HandleGuard,
        token: u32,
    ) -> Result<[bool; 2], i32> {
        let (_, id) = self.events_lookup_locked(ram, map, guard, token)?;
        let state = gamepkg::local_event_state(id)?;
        Ok([state.manual, state.signaled])
    }
    pub fn events_set(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        token: u32,
        signal: bool,
    ) -> Result<(), i32> {
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_signal_locked(ram, map, &guard, token, signal);
        let closed = guard.handles_guard_close(ram);
        result?;
        closed
    }
    fn events_signal_locked(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        guard: &handles::HandleGuard,
        token: u32,
        signal: bool,
    ) -> Result<(), i32> {
        let (_, id) = self.events_lookup_locked(ram, map, guard, token)?;
        events_signal(id, signal)
    }
    pub fn events_alias(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        token: u32,
        fresh: bool,
    ) -> Result<u32, i32> {
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_alias_locked(ram, map, &guard, token, fresh);
        let closed = guard.handles_guard_close(ram);
        let value = result?;
        closed?;
        Ok(value)
    }
    fn events_alias_locked(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        guard: &handles::HandleGuard,
        token: u32,
        fresh: bool,
    ) -> Result<u32, i32> {
        let (_, id) = self.events_lookup_locked(ram, map, guard, token)?;
        guard.handle_guard_insert(ram, id, 1, fresh)
    }
    pub fn events_remove(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        token: u32,
    ) -> Result<(), i32> {
        let mut guard = map.handles_lock(ram)?;
        let result = self.events_remove_locked(ram, map, &guard, token);
        let closed = guard.handles_guard_close(ram);
        result?;
        closed
    }
    fn events_remove_locked(
        &self,
        ram: &memory::Memory,
        map: &handles::Handles,
        guard: &handles::HandleGuard,
        token: u32,
    ) -> Result<(), i32> {
        let (row, id) = self.events_lookup_locked(ram, map, guard, token)?;
        let references = guard.handle_guard_references(ram, id, 1)?;
        if references == 0 {
            return Err(-3);
        }
        if references > 1 {
            if guard.handle_guard_remove(ram, token)? != Some([id, 1]) {
                return Err(-3);
            }
            return Ok(());
        }
        let (version, mut b) = self.events_snapshot(ram, map)?;
        events_put(&mut b, 72 + row * 16, 3)?;
        events_commit(self.record, version, &b)?;
        self.events_drain_row(ram, guard, row, id)
    }
    fn events_drain_row(
        &self,
        ram: &memory::Memory,
        guard: &handles::HandleGuard,
        row: usize,
        id: u64,
    ) -> Result<(), i32> {
        let (_, b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&b, 64 + row * 16)? != id || events_word(&b, 72 + row * 16)? != 3 {
            return Err(-4);
        }
        match gamepkg::local_event_state(id) {
            Ok(_) => events_release(id)?,
            Err(-4) => {}
            Err(code) => return Err(code),
        }
        for (index, item) in guard.handle_guard_slots(ram)?.iter().enumerate() {
            if *item == [id, 1]
                && guard.handle_guard_remove(ram, 0x48000001 + index as u32)? != Some([id, 1])
            {
                return Err(-3);
            }
        }
        let (version, mut b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&b, 64 + row * 16)? != id || events_word(&b, 72 + row * 16)? != 3 {
            return Err(-4);
        }
        events_put(&mut b, 64 + row * 16, 0)?;
        events_put(&mut b, 72 + row * 16, 0)?;
        events_commit(self.record, version, &b)
    }
    pub fn events_close(
        &mut self,
        ram: &memory::Memory,
        map: &handles::Handles,
    ) -> Result<(), i32> {
        if !self.owning
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.owner
        {
            return Err(-4);
        }
        if self.record == 0 {
            return if self.close_error == 0 {
                Ok(())
            } else {
                Err(self.close_error)
            };
        }
        if map.handles_id() != self.table {
            return Err(-4);
        }
        let validation = events_live(ram, self.root, self.services);
        let mut guard = map.handles_cleanup_lock(ram)?;
        let result = self.events_close_locked(ram, &guard);
        let closed = guard.handles_guard_close(ram);
        result?;
        closed?;
        gamepkg::local_record_release(self.record)?;
        self.record = 0;
        if let Err(code) = validation {
            self.close_error = code;
        }
        if self.close_error == 0 {
            Ok(())
        } else {
            Err(self.close_error)
        }
    }
    fn events_close_locked(
        &mut self,
        ram: &memory::Memory,
        guard: &handles::HandleGuard,
    ) -> Result<(), i32> {
        let (_, table) = gamepkg::local_record_snapshot(self.table, 80)?;
        if events_word(&table, 32)? != 1 {
            return Err(-2);
        }
        let (version, mut b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        for row in 0..128usize {
            if events_word(&b, 72 + row * 16)? == 1 {
                return Err(-2);
            }
        }
        if events_word(&b, 40)? == 0 {
            events_put(&mut b, 40, 1)?;
            events_commit(self.record, version, &b)?;
        }
        for row in 0..128usize {
            let (version, mut b) = events_record(
                self.record,
                self.root,
                self.services,
                self.table,
                self.owner,
            )?;
            let state = events_word(&b, 72 + row * 16)?;
            if state == 0 {
                continue;
            }
            let id = events_word(&b, 64 + row * 16)?;
            if state == 2 {
                if gamepkg::local_event_state(id).err() == Some(-4) {
                    self.close_error = -4;
                }
                events_put(&mut b, 72 + row * 16, 3)?;
                events_commit(self.record, version, &b)?;
            }
            self.events_drain_row(ram, guard, row, id)?;
        }
        Ok(())
    }
}
impl EventCreation {
    fn event_creation_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.closed
            || ram.call_handle() != self.root
            || gamepkg::local_task_status()?.0 != self.task
        {
            return Err(-4);
        }
        Ok(())
    }
    pub fn event_creation_id(&self) -> u64 {
        self.event
    }
    pub fn event_creation_token(&self) -> u32 {
        self.token
    }
    pub fn event_creation_phase(&self) -> u32 {
        self.phase
    }
    pub fn event_creation_closed(&self) -> bool {
        self.closed
    }
    pub fn event_create(
        &mut self,
        ram: &memory::Memory,
        event_type: u32,
        initial: u32,
    ) -> Result<(), i32> {
        self.event_creation_owner(ram)?;
        events_live(ram, self.root, self.services)?;
        if self.attempted_create {
            return Err(-1);
        }
        self.attempted_create = true;
        let mut reply = [0u8; 8];
        if gamepkg::local_call(
            20,
            &[
                u32::to_le_bytes(u32::from(event_type == 0)).as_slice(),
                &u32::to_le_bytes(u32::from(initial & 255 != 0)),
            ]
            .concat(),
            &mut reply,
        )? != 8
        {
            return Err(-3);
        }
        self.event = u64::from_le_bytes(reply);
        self.phase = 1;
        Ok(())
    }
    pub fn event_publish(&mut self, ram: &mut memory::Memory, guest: u32) -> Result<u32, i32> {
        self.event_creation_owner(ram)?;
        events_live(ram, self.root, self.services)?;
        if self.phase != 1 || self.attempted_publish {
            return Err(-1);
        }
        self.attempted_publish = true;
        let (version, mut b) = events_record(
            self.record,
            self.root,
            self.services,
            self.table,
            self.owner,
        )?;
        if events_word(&b, 40)? != 0
            || events_word(&b, 64 + self.row * 16)? != 0
            || events_word(&b, 72 + self.row * 16)? != 1
        {
            return Err(-4);
        }
        gamepkg::local_event_state(self.event)?;
        events_put(&mut b, 64 + self.row * 16, self.event)?;
        events_put(&mut b, 72 + self.row * 16, 2)?;
        events_commit(self.record, version, &b)?;
        // Retain transfer immediately: later tag publication or guest write can fail.
        self.phase = 2;
        if guest != 0 {
            self.token = self.guard.handle_guard_insert(ram, self.event, 1, false)?;
            self.phase = 3;
            ram.write_u32(guest, self.token)?;
        }
        self.phase = 4;
        Ok(self.token)
    }
    pub fn event_creation_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if self.closed {
            return Ok(());
        }
        self.event_creation_owner(ram)?;
        if self.phase < 2 && !self.cleared {
            if self.event != 0 && !self.released {
                gamepkg::local_event_state(self.event)?;
                events_release(self.event)?;
                self.released = true;
            }
            let (version, mut b) = events_record(
                self.record,
                self.root,
                self.services,
                self.table,
                self.owner,
            )?;
            if events_word(&b, 64 + self.row * 16)? != 0
                || events_word(&b, 72 + self.row * 16)? != 1
            {
                return Err(-4);
            }
            events_put(&mut b, 72 + self.row * 16, 0)?;
            events_commit(self.record, version, &b)?;
            self.cleared = true;
        }
        self.guard.handles_guard_close(ram)?;
        self.closed = true;
        Ok(())
    }
}
