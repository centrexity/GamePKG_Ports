// DOS drive aliases are guest namespace entries. Their targets are the same
// disc and private save roots already granted to the checked startup.
use crate::gamepkg;
use crate::kernel_file;
use crate::kernel_pool;
use crate::memory;
use crate::registers;

const DRIVE_ROOTS: [&str; 7] = [
    "",
    "\\Device\\CdRom0",
    "\\Device\\Harddisk0\\partition1",
    "\\Device\\Harddisk0\\partition1\\TDATA",
    "\\Device\\Harddisk0\\partition1\\UDATA",
    "\\Device\\Harddisk0\\partition1\\TDATA\\4541005b",
    "\\Device\\Harddisk0\\partition1\\UDATA\\4541005b",
];

const LINK_KIND: u32 = 7;
const LINK_LIMIT: usize = 32;
const LINK_NAME_BYTES: usize = 48;

pub struct DriveLinks {
    slots: std::vec::Vec<(u8, [u8; LINK_NAME_BYTES])>,
    opened_links: std::vec::Vec<(u32, u8, [u8; LINK_NAME_BYTES])>,
    drive_guard: Option<kernel_pool::HandleGuard>,
    pending_link: u32,
}

impl DriveLinks {
    pub fn drive_links_new() -> Self {
        let mut slots = std::vec::Vec::new();
        slots.resize(26, (0, [0; LINK_NAME_BYTES]));
        let mut opened_links = std::vec::Vec::new();
        opened_links.resize(LINK_LIMIT, (0, 0, [0; LINK_NAME_BYTES]));
        Self {
            slots,
            opened_links,
            drive_guard: None,
            pending_link: 0,
        }
    }

    pub fn drive_links_call(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        cpu: &mut registers::Registers,
        ordinal: u32,
    ) -> Result<(), i32> {
        if gamepkg::local_task_status()?.1 {
            return Err(-4);
        }
        self.drive_links_settle(ram, table)?;
        let frame_length = match ordinal {
            67 | 203 => 12,
            69 | 187 => 8,
            215 => 16,
            _ => return Err(-5),
        };
        let next_stack = cpu.esp.checked_add(frame_length).ok_or(-1)?;
        let mut frame = [0u8; 16];
        ram.read_bytes(cpu.esp, &mut frame[..frame_length as usize])?;
        let first = kernel_file::rom_kernel_word(&frame, 4)?;
        let status = match ordinal {
            67 | 69 => self.drive_links_update(ram, &frame, first, ordinal == 67)?,
            203 => {
                self.drive_link_open(ram, table, first, kernel_file::rom_kernel_word(&frame, 8)?)?
            }
            215 => self.drive_link_query(ram, table, first, &frame)?,
            187 => {
                self.drive_link_remove(ram, table, first)?;
                0
            }
            _ => return Err(-5),
        };
        ram.kernel_set_caller(kernel_file::rom_kernel_word(&frame, 0)?);
        cpu.esp = next_stack;
        cpu.eax = status;
        Ok(())
    }

    fn drive_links_update(
        &mut self,
        ram: &memory::Memory,
        frame: &[u8],
        symbol_at: u32,
        create: bool,
    ) -> Result<u32, i32> {
        let symbol = kernel_file::rom_kernel_counted_name(ram, symbol_at)?;
        let slot = drive_object_slot(&symbol)?;
        if !create {
            if self.slots[slot].0 == 0 {
                return Ok(0xc0000034);
            }
            self.slots[slot] = (0, [0; LINK_NAME_BYTES]);
            return Ok(0);
        }
        let target =
            kernel_file::rom_kernel_counted_name(ram, kernel_file::rom_kernel_word(frame, 8)?)?;
        for (index, allowed) in DRIVE_ROOTS.iter().enumerate() {
            if target.eq_ignore_ascii_case(allowed.as_bytes()) {
                if self.slots[slot].0 != 0 {
                    return Ok(0xc0000035);
                }
                let mut stored = [0; LINK_NAME_BYTES];
                for (byte, source) in stored.iter_mut().zip(&target) {
                    *byte = *source;
                }
                self.slots[slot] = (index as u8, stored);
                return Ok(0);
            }
        }
        Err(-5)
    }

    pub fn drive_links_own(&self, token: u32) -> bool {
        token != 0 && self.drive_open_index(token).is_ok()
    }

    fn drive_open_index(&self, token: u32) -> Result<usize, i32> {
        for (index, entry) in self.opened_links.iter().enumerate() {
            if entry.0 == token {
                return Ok(index);
            }
        }
        Err(-4)
    }

    fn drive_links_unlock(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if let Some(held_link_guard) = self.drive_guard.as_mut() {
            held_link_guard.handles_guard_close(ram)?;
            self.drive_guard = None;
        }
        Ok(())
    }

    fn drive_links_settle(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.drive_links_unlock(ram)?;
        if self.pending_link != 0 {
            self.drive_link_remove(ram, table, self.pending_link)?;
        }
        Ok(())
    }

    fn drive_link_open(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        output_at: u32,
        object_at: u32,
    ) -> Result<u32, i32> {
        if output_at == 0 || object_at == 0 {
            return Err(-1);
        }
        let flags = ram.read_u32(object_at.checked_add(8).ok_or(-1)?)?;
        if flags & !0x40 != 0 {
            return Err(-5);
        }
        let symbol = kernel_file::rom_kernel_name(ram, object_at)?;
        let (root, spelling) = self.slots[drive_object_slot(&symbol)?];
        if root == 0 {
            return Ok(0xc0000034);
        }
        ram.memory_check_write(output_at, 4)?;
        let entry = match self.drive_open_index(0) {
            Ok(free) => free,
            Err(_) => return Err(-2),
        };
        self.drive_guard = Some(table.handles_lock(ram)?);
        let reservation = self.drive_guard.as_ref().ok_or(-3)?.handle_guard_insert(
            ram,
            u64::from(root),
            LINK_KIND,
            true,
        );
        if let Ok(token) = reservation {
            self.opened_links[entry] = (token, root, spelling);
            self.pending_link = token;
        }
        self.drive_links_unlock(ram)?;
        let token = reservation?;
        let written = ram.write_u32(output_at, token);
        if written.is_ok() {
            self.pending_link = 0;
        }
        self.drive_links_settle(ram, table)?;
        written?;
        Ok(0)
    }

    fn drive_link_query(
        &mut self,
        ram: &mut memory::Memory,
        table: &kernel_pool::Handles,
        token: u32,
        frame: &[u8],
    ) -> Result<u32, i32> {
        if token == 0 {
            return Err(-4);
        }
        let (_, root, spelling) = self.opened_links[self.drive_open_index(token)?];
        self.drive_guard = Some(table.handles_lock(ram)?);
        let selected = self
            .drive_guard
            .as_ref()
            .ok_or(-3)?
            .handle_guard_lookup(ram, token);
        self.drive_links_unlock(ram)?;
        if selected? != Some([u64::from(root), u64::from(LINK_KIND)]) {
            return Err(-4);
        }
        let descriptor = kernel_file::rom_kernel_word(frame, 8)?;
        let returned_at = kernel_file::rom_kernel_word(frame, 12)?;
        let length = DRIVE_ROOTS[root as usize].len();
        let returned = u32::to_le_bytes(length as u32);
        let length_output = if returned_at == 0 {
            &returned[..0]
        } else {
            &returned[..]
        };
        let mut fields = [0u8; 8];
        if descriptor != 0 {
            ram.read_bytes(descriptor, &mut fields)?;
        }
        let capacity = usize::from(u16::from_le_bytes([fields[2], fields[3]]));
        let buffer = kernel_file::rom_kernel_word(&fields, 4)?;
        if buffer == 0 || capacity <= length {
            if returned_at != 0 {
                ram.write_u32(returned_at, length as u32)?;
            }
            return Ok(0xc0000023);
        }
        let mut buffer_descriptor = fields;
        let encoded_length = u16::to_le_bytes(length as u16);
        buffer_descriptor[0] = encoded_length[0];
        buffer_descriptor[1] = encoded_length[1];
        ram.memory_write_disjoint([
            (descriptor, &buffer_descriptor),
            (buffer, &spelling[..length + 1]),
            (returned_at, length_output),
        ])?;
        Ok(0)
    }

    fn drive_link_remove(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
        token: u32,
    ) -> Result<(), i32> {
        if token == 0 {
            return Err(-4);
        }
        let index = self.drive_open_index(token)?;
        let value = [u64::from(self.opened_links[index].1), u64::from(LINK_KIND)];
        self.drive_guard = Some(table.handles_cleanup_lock(ram)?);
        let held_link = self.drive_guard.as_ref().ok_or(-3)?;
        if held_link.handle_guard_lookup(ram, token)? != Some(value) {
            return Err(-4);
        }
        if held_link.handle_guard_remove(ram, token)? != Some(value) {
            return Err(-3);
        }
        self.opened_links[index] = (0, 0, [0; LINK_NAME_BYTES]);
        if self.pending_link == token {
            self.pending_link = 0;
        }
        self.drive_links_unlock(ram)
    }

    pub fn drive_links_drain(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.drive_links_settle(ram, table)?;
        for index in 0..LINK_LIMIT {
            let token = self.opened_links[index].0;
            if token != 0 {
                self.drive_link_remove(ram, table, token)?;
            }
        }
        for alias in &mut self.slots {
            *alias = (0, [0; LINK_NAME_BYTES]);
        }
        Ok(())
    }

    pub fn drive_links_resolve(
        &self,
        path: &[u8],
        result: &mut [u8; 528],
    ) -> Result<Option<usize>, i32> {
        let operand = drive_links_dos_name(path);
        if operand.get(1) != Some(&b':') {
            return Ok(None);
        }
        let slot = drive_letter(operand)?;
        let tail = operand.get(2..).ok_or(-5)?;
        if tail.first() != Some(&b'\\') {
            return Err(-5);
        }
        let root_id = self.slots[slot].0;
        if root_id == 0 {
            return Err(-5);
        }
        let root = DRIVE_ROOTS[root_id as usize].as_bytes();
        let length = root.len() + tail.len();
        if length > result.len() {
            return Err(-5);
        }
        for (byte, from) in result[..root.len()].iter_mut().zip(root) {
            *byte = *from;
        }
        for (byte, from) in result[root.len()..length].iter_mut().zip(tail) {
            *byte = *from;
        }
        Ok(Some(length))
    }
}

fn drive_links_dos_name(path: &[u8]) -> &[u8] {
    if path.starts_with(b"\\??\\") {
        &path[4..]
    } else {
        path
    }
}

fn drive_object_slot(symbol: &[u8]) -> Result<usize, i32> {
    let name = drive_links_dos_name(symbol);
    if !symbol.starts_with(b"\\??\\") || name.len() != 2 {
        return Err(-5);
    }
    drive_letter(name)
}

fn drive_letter(name: &[u8]) -> Result<usize, i32> {
    if let Some(&letter) = name.first() {
        let upper = letter.to_ascii_uppercase();
        if upper.is_ascii_uppercase() && name.get(1) == Some(&b':') {
            return Ok(usize::from(upper - b'A'));
        }
    }
    Err(-5)
}
