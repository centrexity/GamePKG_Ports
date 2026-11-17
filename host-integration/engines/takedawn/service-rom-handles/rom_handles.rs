// Typed ROM files in the checked guest handle table. No device or OS handles.
use crate::disc;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;

const ROM_KIND: u32 = 4;
const RECORD_MAGIC: u64 = 0x31464d52;
const RECORD_BYTES: usize = 48;
const READ_LIMIT: usize = 65536;

pub struct RomFiles {
    root: u64,
    table: u64,
    task: u64,
    pending_guard: Option<kernel_pool::HandleGuard>,
    pending_file: u64,
    pending_record: u64,
}

pub struct RomRead {
    root: u64,
    services: u64,
    task: u64,
    record: u64,
    version: u64,
    bytes: std::vec::Vec<u8>,
    ready: bool,
    attempted: bool,
    next_cursor: Option<u64>,
    publish_cursor: Option<bool>,
    closed: bool,
}

fn word(bytes: &[u8], index: usize) -> Result<u64, i32> {
    let part = bytes.get(index..index + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        part[0], part[1], part[2], part[3], part[4], part[5], part[6], part[7],
    ]))
}

fn retire(id: u64) -> Result<(), i32> {
    match gamepkg::file_close(id) {
        Ok(()) | Err(-4) => Ok(()),
        Err(error) => Err(error),
    }
}

impl RomFiles {
    pub fn rom_files_new(ram: &memory::Memory, table: &kernel_pool::Handles) -> Result<Self, i32> {
        kernel_pool::kernel_service_values(ram)?;
        Ok(Self {
            root: ram.call_handle(),
            table: table.handles_id(),
            task: gamepkg::local_task_status()?.0,
            pending_guard: None,
            pending_file: 0,
            pending_record: 0,
        })
    }

    fn rom_binding(&self, ram: &memory::Memory, table: &kernel_pool::Handles) -> Result<(), i32> {
        if self.root != ram.call_handle()
            || self.table == 0
            || self.table != table.handles_id()
            || self.task != gamepkg::local_task_status()?.0
        {
            return Err(-4);
        }
        Ok(())
    }

    fn rom_release_guard(
        &mut self,
        ram: &memory::Memory,
        mut guard: kernel_pool::HandleGuard,
    ) -> Result<(), i32> {
        if let Err(code) = guard.handles_guard_close(ram) {
            self.pending_guard = Some(guard);
            return Err(code);
        }
        Ok(())
    }

    // Retry only pending retirements. An open is never automatically replayed.
    pub fn rom_cleanup(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.rom_binding(ram, table)?;
        if let Some(guard) = self.pending_guard.as_mut() {
            guard.handles_guard_close(ram)?;
            self.pending_guard = None;
        }
        if self.pending_file != 0 {
            retire(self.pending_file)?;
            self.pending_file = 0;
        }
        if self.pending_record != 0 {
            match gamepkg::local_record_release(self.pending_record) {
                Ok(()) | Err(-4) => self.pending_record = 0,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    // A nonzero output reports publication even if releasing the table guard
    // subsequently fails. The caller must retain that close obligation.
    pub fn rom_open(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        image: &disc::Disc,
        path: &str,
        output: &mut u32,
    ) -> Result<(), i32> {
        if *output != 0 {
            return Err(-1);
        }
        self.rom_cleanup(ram, table)?;
        let stream = image.disc_open_file(path)?;
        self.pending_file = stream.disc_file_open_handle()?;
        let bytes = [
            u64::to_le_bytes(RECORD_MAGIC).as_slice(),
            &u64::to_le_bytes(self.table),
            &u64::to_le_bytes(self.pending_file),
            &u64::to_le_bytes(u64::from(stream.disc_file_size())),
            &u64::to_le_bytes(0),
            &u64::to_le_bytes(0),
        ]
        .concat();
        let created = self.rom_publish(ram, table, &bytes, output);
        if *output == 0 {
            // Preserve pending IDs on cleanup failure, even if publication
            // already failed for a different reason.
            self.rom_cleanup(ram, table)?;
        }
        created
    }

    fn rom_publish(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        bytes: &[u8],
        output: &mut u32,
    ) -> Result<(), i32> {
        self.pending_record = gamepkg::local_record_create(bytes)?;
        let guard = table.handles_lock(ram)?;
        let result = guard.handle_guard_insert(ram, self.pending_record, ROM_KIND, true);
        if let Ok(token) = result {
            *output = token;
            self.pending_file = 0;
            self.pending_record = 0;
        }
        let released = self.rom_release_guard(ram, guard);
        result?;
        released
    }

    fn rom_lookup(
        &self,
        ram: &memory::Memory,
        guard: &kernel_pool::HandleGuard,
        token: u32,
    ) -> Result<(u64, u64, std::vec::Vec<u8>), i32> {
        let entry = guard.handle_guard_lookup(ram, token)?.ok_or(-4)?;
        if entry[1] != u64::from(ROM_KIND) {
            return Err(-4);
        }
        let (version, bytes) = gamepkg::local_record_snapshot(entry[0], RECORD_BYTES)?;
        if bytes.len() != RECORD_BYTES
            || word(&bytes, 0)? != RECORD_MAGIC
            || word(&bytes, 8)? != self.table
            || word(&bytes, 16)? == 0
            || word(&bytes, 24)? > u64::from(u32::MAX)
            || word(&bytes, 32)? > word(&bytes, 24)?
            || word(&bytes, 40)? > 1
        {
            return Err(-4);
        }
        if word(&bytes, 40)? != 0 {
            return Err(-2);
        }
        Ok((entry[0], version, bytes))
    }

    // Reserve one operation on this file before dropping the shared table
    // lock. Neither close nor another read can consume the same cursor.
    fn rom_reserve(
        &self,
        ram: &memory::Memory,
        guard: &kernel_pool::HandleGuard,
        token: u32,
    ) -> Result<(u64, u64, std::vec::Vec<u8>), i32> {
        let (record, version, mut bytes) = self.rom_lookup(ram, guard, token)?;
        bytes[40] = 1;
        let version = gamepkg::local_record_compare_exchange(record, version, &bytes)?;
        Ok((record, version, bytes))
    }

    pub fn rom_read_begin(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        token: u32,
        output: &mut Option<RomRead>,
    ) -> Result<(), i32> {
        if output.is_some() {
            return Err(-1);
        }
        self.rom_cleanup(ram, table)?;
        let task = gamepkg::local_task_status()?.0;
        let services = ram.services_handle()?;
        let guard = table.handles_lock(ram)?;
        let reserved = self.rom_reserve(ram, &guard, token);
        let released = self.rom_release_guard(ram, guard);
        let (record, version, bytes) = reserved?;
        *output = Some(RomRead {
            root: self.root,
            services,
            task,
            record,
            version,
            bytes,
            ready: released.is_ok(),
            attempted: false,
            next_cursor: None,
            publish_cursor: None,
            closed: false,
        });
        released
    }

    pub fn rom_close(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        token: u32,
    ) -> Result<(), i32> {
        self.rom_cleanup(ram, table)?;
        let guard = table.handles_lock(ram)?;
        let result = self.rom_close_locked(ram, &guard, token);
        let released = self.rom_release_guard(ram, guard);
        result?;
        released
    }

    fn rom_close_locked(
        &mut self,
        ram: &memory::Memory,
        guard: &kernel_pool::HandleGuard,
        token: u32,
    ) -> Result<(), i32> {
        let (record, _, bytes) = self.rom_lookup(ram, guard, token)?;
        // Keep the entry until the host confirms retirement. If removing the
        // entry fails, a later close can confirm that the range is now gone.
        retire(word(&bytes, 16)?)?;
        if guard.handle_guard_remove(ram, token)? != Some([record, u64::from(ROM_KIND)]) {
            return Err(-3);
        }
        self.pending_record = record;
        gamepkg::local_record_release(record)?;
        self.pending_record = 0;
        Ok(())
    }

    // Run after producers/workers join and before destroying the shared
    // table. Preserves other object kinds for their respective owners.
    pub fn rom_drain(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.rom_cleanup(ram, table)?;
        let guard = table.handles_cleanup_lock(ram)?;
        let result = self.rom_drain_locked(ram, &guard);
        let released = self.rom_release_guard(ram, guard);
        result?;
        released
    }

    fn rom_drain_locked(
        &mut self,
        ram: &memory::Memory,
        guard: &kernel_pool::HandleGuard,
    ) -> Result<(), i32> {
        for (slot, entry) in guard.handle_guard_slots(ram)?.iter().enumerate() {
            if entry[1] == u64::from(ROM_KIND) {
                self.rom_close_locked(ram, guard, 0x48000001 + slot as u32)?;
            }
        }
        Ok(())
    }
}

impl RomRead {
    fn rom_read_owner(&self, ram: &memory::Memory) -> Result<(), i32> {
        if self.root != ram.call_handle() || self.task != gamepkg::local_task_status()?.0 {
            return Err(-4);
        }
        Ok(())
    }

    pub fn rom_read_into(
        &mut self,
        ram: &memory::Memory,
        offset: Option<u64>,
        output: &mut [u8],
    ) -> Result<usize, i32> {
        self.rom_read_owner(ram)?;
        if !self.ready || self.closed || self.attempted || output.len() > READ_LIMIT {
            return Err(-1);
        }
        self.attempted = true;
        kernel_pool::kernel_service_values(ram)?;
        if ram.services_handle()? != self.services || gamepkg::local_task_status()?.1 {
            return Err(-4);
        }
        let start = offset.unwrap_or(word(&self.bytes, 32)?);
        let size = word(&self.bytes, 24)?;
        let id = word(&self.bytes, 16)?;
        let count = rom_transfer(id, start, size, output)?;
        kernel_pool::kernel_service_values(ram)?;
        if ram.services_handle()? != self.services || gamepkg::local_task_status()?.1 {
            return Err(-4);
        }
        if offset.is_none() {
            self.next_cursor = Some(start + count as u64);
        }
        Ok(count)
    }

    pub fn rom_read_finish(&mut self, ram: &memory::Memory, publish: bool) -> Result<(), i32> {
        self.rom_read_owner(ram)?;
        if let Some(choice) = self.publish_cursor {
            if choice != publish {
                return Err(-1);
            }
        }
        if !self.closed {
            self.publish_cursor = Some(publish);
            if publish {
                if let Some(next) = self.next_cursor {
                    self.bytes[32..40].copy_from_slice(&u64::to_le_bytes(next));
                }
            }
            self.bytes[40] = 0;
            gamepkg::local_record_compare_exchange(self.record, self.version, &self.bytes)?;
            self.closed = true;
        }
        Ok(())
    }
}

fn rom_transfer(id: u64, start: u64, size: u64, output: &mut [u8]) -> Result<usize, i32> {
    let count = size.saturating_sub(start).min(output.len() as u64) as usize;
    if count == 0 {
        if gamepkg::file_read(id, start, &mut [])? != 0 {
            return Err(-3);
        }
        return Ok(0);
    }
    let mut done = 0usize;
    while done < count {
        let received = gamepkg::file_read(id, start + done as u64, &mut output[done..count])?;
        if received == 0 || received > count - done {
            return Err(-3);
        }
        done += received;
    }
    Ok(done)
}
