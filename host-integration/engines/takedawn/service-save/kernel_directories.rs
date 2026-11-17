// Burnout's private hard disk namespace. Directory tokens carry no permission
// for root-relative paths. All path resolution and quotas belong to GamePKG.
use crate::gamepkg;
use crate::kernel_file;
use crate::kernel_pool;
use crate::memory;
use crate::registers;

const SAVE_DIRECTORY_KIND: u32 = 5;
const SAVE_DIRECTORY_MAGIC: u64 = 0x31445253;
const SAVE_DIRECTORY_LIMIT: usize = 64;

pub struct SaveDirectories {
    root: u64,
    table: u64,
    task: u64,
    grant: u64,
    entries: [[u64; 2]; SAVE_DIRECTORY_LIMIT],
    guard: Option<kernel_pool::HandleGuard>,
    retired_record: u64,
    unpublished_token: u32,
}

impl SaveDirectories {
    pub fn save_directories_new(
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        grant: u64,
    ) -> Result<Self, i32> {
        kernel_pool::kernel_service_values(ram)?;
        let (task, cancelled) = gamepkg::local_task_status()?;
        if cancelled || gamepkg::save_directory(grant, "", false)? != 1 {
            return Err(-4);
        }
        Ok(Self {
            root: ram.call_handle(),
            table: table.handles_id(),
            task,
            grant,
            entries: [[0; 2]; SAVE_DIRECTORY_LIMIT],
            guard: None,
            retired_record: 0,
            unpublished_token: 0,
        })
    }

    pub fn save_directory_invoke(
        &mut self,
        table: &kernel_pool::Handles,
        ram: &mut memory::Memory,
        cpu: &mut registers::Registers,
        ordinal: u32,
    ) -> Result<(), i32> {
        let bytes = match ordinal {
            187 => 8u32,
            190 => 40,
            202 => 28,
            218 => 24,
            _ => return Err(-5),
        };
        if !cpu.valid() || cpu.esp == 0 {
            return Err(-1);
        }
        self.save_directory_binding(ram, table)?;
        if gamepkg::local_task_status()?.1 {
            return Err(-4);
        }
        self.save_directory_settle(ram, table)?;
        let next_stack = cpu.esp.checked_add(bytes).ok_or(-1)?;
        let mut frame = [0u8; 40];
        ram.read_bytes(cpu.esp, &mut frame[..bytes as usize])?;
        let status = match ordinal {
            187 => {
                self.save_directory_close(ram, table, kernel_file::rom_kernel_word(&frame, 4)?)?;
                0
            }
            190 | 202 => self.save_directory_open(ram, table, &frame, ordinal == 190)?,
            218 => self.save_directory_volume(ram, table, &frame)?,
            _ => return Err(-5),
        };
        ram.kernel_set_caller(kernel_file::rom_kernel_word(&frame, 0)?);
        cpu.esp = next_stack;
        cpu.eax = status;
        Ok(())
    }

    fn save_directory_binding(
        &self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        if self.root != ram.call_handle()
            || self.table != table.handles_id()
            || self.task != gamepkg::local_task_status()?.0
        {
            return Err(-4);
        }
        Ok(())
    }

    fn save_directory_settle(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.save_directory_unlock(ram)?;
        if self.retired_record != 0 {
            match gamepkg::local_record_release(self.retired_record) {
                Ok(()) | Err(-4) => self.retired_record = 0,
                Err(code) => return Err(code),
            }
        }
        if self.unpublished_token != 0 {
            let abandoned_token = self.unpublished_token;
            self.save_directory_close(ram, table, abandoned_token)?;
        }
        Ok(())
    }

    fn save_directory_unlock(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if let Some(held_directory_guard) = self.guard.as_mut() {
            held_directory_guard.handles_guard_close(ram)?;
            self.guard = None;
        }
        Ok(())
    }

    fn save_directory_open(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        frame: &[u8],
        ensure: bool,
    ) -> Result<u32, i32> {
        let handle_va = kernel_file::rom_kernel_word(frame, 4)?;
        let access = kernel_file::rom_kernel_word(frame, 8)?;
        let object_va = kernel_file::rom_kernel_word(frame, 12)?;
        let status_va = kernel_file::rom_kernel_word(frame, 16)?;
        if access != 0x100001 || object_va == 0 || handle_va == 0 || status_va == 0 {
            return Err(-5);
        }
        let handle_location = u64::from(handle_va);
        let status_location = u64::from(status_va);
        if handle_location < status_location + 8 && status_location < handle_location + 4 {
            return Err(-1);
        }
        let read_field = if ensure { 28 } else { 20 };
        if kernel_file::rom_kernel_word(frame, read_field)? != 3
            || kernel_file::rom_kernel_word(frame, read_field + 4)?
                != if ensure { 3 } else { 0x800021 }
            || (ensure
                && (kernel_file::rom_kernel_word(frame, 20)? != 0
                    || kernel_file::rom_kernel_word(frame, 24)? != 0x80
                    || kernel_file::rom_kernel_word(frame, 36)? != 0x4021))
            || ram.read_u32(object_va)? != 0
            || ram.read_u32(object_va.checked_add(8).ok_or(-1)?)? != 0x40
        {
            return Err(-5);
        }
        let counted_name = kernel_file::rom_kernel_name(ram, object_va)?;
        let portable = save_directory_name(gamepkg::text_from_utf8(&counted_name)?)?;
        if portable.is_empty() && ensure {
            return Err(-5);
        }
        ram.memory_check_write(handle_va, 4)?;
        ram.memory_check_write(status_va, 8)?;
        let directory_slot = self.save_directory_index(0)?;

        // Establish the guest token before creating a persistent directory.
        // It is delivered only after the host operation and guest write succeed.
        if let Err(code) = self.save_directory_reserve(ram, table, directory_slot) {
            self.save_directory_settle(ram, table)?;
            return Err(code);
        }
        let result = match gamepkg::save_directory(self.grant, portable, ensure) {
            Ok(0) => match ram.write_u64(status_va, 0xc0000034) {
                Ok(()) => Ok(0xc0000034),
                Err(code) => Err(code),
            },
            Ok(state) => {
                let io_status = u64::to_le_bytes(u64::from(state) << 32);
                let published = ram.memory_write_pair(
                    handle_va,
                    &u32::to_le_bytes(self.unpublished_token),
                    status_va,
                    &io_status,
                );
                if published.is_ok() {
                    self.unpublished_token = 0;
                }
                match published {
                    Ok(()) => Ok(0),
                    Err(code) => Err(code),
                }
            }
            Err(code) => Err(code),
        };
        self.save_directory_settle(ram, table)?;
        result
    }

    fn save_directory_reserve(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        directory_slot: usize,
    ) -> Result<(), i32> {
        let slot = self.entries.get_mut(directory_slot).ok_or(-3)?;
        let directory_identity = [
            u64::to_le_bytes(SAVE_DIRECTORY_MAGIC),
            u64::to_le_bytes(self.table),
            u64::to_le_bytes(self.grant),
        ]
        .concat();
        self.retired_record = gamepkg::local_record_create(&directory_identity)?;
        self.guard = Some(table.handles_lock(ram)?);
        let token = self.guard.as_ref().ok_or(-3)?.handle_guard_insert(
            ram,
            self.retired_record,
            SAVE_DIRECTORY_KIND,
            true,
        )?;
        *slot = [u64::from(token), self.retired_record];
        self.unpublished_token = token;
        self.retired_record = 0;
        self.save_directory_unlock(ram)
    }

    pub fn save_directory_owns(&self, token: u32) -> bool {
        token != 0 && self.save_directory_index(token).is_ok()
    }

    fn save_directory_index(&self, token: u32) -> Result<usize, i32> {
        for (directory_index, pair) in self.entries.iter().enumerate() {
            if pair[0] == u64::from(token) {
                return Ok(directory_index);
            }
        }
        Err(if token == 0 { -2 } else { -4 })
    }

    fn save_directory_entry(
        &self,
        ram: &memory::Memory,
        token: u32,
        record: u64,
    ) -> Result<(), i32> {
        let owned_directory = self
            .guard
            .as_ref()
            .ok_or(-3)?
            .handle_guard_lookup(ram, token)?;
        if owned_directory != Some([record, u64::from(SAVE_DIRECTORY_KIND)]) {
            return Err(-4);
        }
        Ok(())
    }

    fn save_directory_volume(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        frame: &[u8],
    ) -> Result<u32, i32> {
        let token = kernel_file::rom_kernel_word(frame, 4)?;
        let status_va = kernel_file::rom_kernel_word(frame, 8)?;
        let result_va = kernel_file::rom_kernel_word(frame, 12)?;
        if kernel_file::rom_kernel_word(frame, 16)? < 24
            || kernel_file::rom_kernel_word(frame, 20)? != 3
            || status_va == 0
            || result_va == 0
        {
            return Err(-5);
        }
        ram.memory_check_write(result_va, 24)?;
        ram.memory_check_write(status_va, 8)?;
        if token == 0 {
            return Err(-4);
        }
        let directory_index = self.save_directory_index(token)?;
        self.guard = Some(table.handles_lock(ram)?);
        let validated = self.save_directory_entry(ram, token, self.entries[directory_index][1]);
        self.save_directory_unlock(ram)?;
        validated?;
        // The reference CRT checks the 16 KiB FATX allocation-unit geometry.
        let capacity = gamepkg::save_volume(self.grant)?;
        let geometry = [
            u64::to_le_bytes(capacity[0] / 16384).as_slice(),
            &u64::to_le_bytes(capacity[1] / 16384),
            &u32::to_le_bytes(32),
            &u32::to_le_bytes(512),
        ]
        .concat();
        ram.memory_write_pair(
            result_va,
            &geometry,
            status_va,
            &u64::to_le_bytes(24u64 << 32),
        )?;
        Ok(0)
    }

    fn save_directory_close(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        token: u32,
    ) -> Result<(), i32> {
        if token == 0 {
            return Err(-4);
        }
        let directory_index = self.save_directory_index(token)?;
        let record = self.entries[directory_index][1];
        self.guard = Some(table.handles_cleanup_lock(ram)?);
        self.save_directory_entry(ram, token, record)?;
        let removed = self
            .guard
            .as_ref()
            .ok_or(-3)?
            .handle_guard_remove(ram, token)?;
        if removed != Some([record, u64::from(SAVE_DIRECTORY_KIND)]) {
            return Err(-3);
        }
        self.entries[directory_index] = [0; 2];
        if self.unpublished_token == token {
            self.unpublished_token = 0;
        }
        self.retired_record = record;
        self.save_directory_unlock(ram)?;
        gamepkg::local_record_release(record)?;
        self.retired_record = 0;
        Ok(())
    }

    pub fn save_directories_drain(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.save_directory_binding(ram, table)?;
        self.save_directory_settle(ram, table)?;
        for directory_slot in 0..SAVE_DIRECTORY_LIMIT {
            let token = self.entries[directory_slot][0] as u32;
            if token != 0 {
                self.save_directory_close(ram, table, token)?;
            }
        }
        Ok(())
    }
}

fn save_directory_name(name: &str) -> Result<&'static str, i32> {
    let prefix = "\\Device\\Harddisk0\\partition1\\";
    if !name
        .get(..prefix.len())
        .ok_or(-5)?
        .eq_ignore_ascii_case(prefix)
    {
        return Err(-5);
    }
    let tail = name.get(prefix.len()..).ok_or(-5)?;
    if tail.is_empty() {
        return Ok("");
    }
    for (xbox, portable) in [
        ("UDATA", "UDATA"),
        ("TDATA", "TDATA"),
        ("UDATA\\4541005b", "UDATA/4541005b"),
        ("TDATA\\4541005b", "TDATA/4541005b"),
    ] {
        if tail.eq_ignore_ascii_case(xbox) {
            return Ok(portable);
        }
    }
    Err(-5)
}
