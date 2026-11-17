// Scoped synchronous optical-file ABI. Other handle domains and asynchronous
// requests remain the responsibility of the surrounding kernel router.
use crate::disc;
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::registers;
use crate::rom_handles;

pub struct RomKernel {
    files: rom_handles::RomFiles,
    orphan: u32,
    orphan_attempted: bool,
    reading: Option<rom_handles::RomRead>,
    publish_cursor: bool,
}

impl RomKernel {
    pub fn rom_kernel_new(ram: &memory::Memory, table: &kernel_pool::Handles) -> Result<Self, i32> {
        Ok(Self {
            files: rom_handles::RomFiles::rom_files_new(ram, table)?,
            orphan: 0,
            orphan_attempted: false,
            reading: None,
            publish_cursor: false,
        })
    }

    // Called by a checked ordinal router. Do not use this subset as an
    // unconditional classification of the whole kernel file interface.
    pub fn rom_kernel_invoke(
        &mut self,
        image: &disc::Disc,
        table: &kernel_pool::Handles,
        ram: &mut memory::Memory,
        cpu: &mut registers::Registers,
        ordinal: u32,
    ) -> Result<(), i32> {
        let length = match ordinal {
            187 => 4u32,
            202 => 24,
            219 => 32,
            _ => return Err(-5),
        };
        if !cpu.valid() || cpu.esp == 0 {
            return Err(-1);
        }
        if self.orphan != 0 || self.reading.is_some() {
            return Err(-2);
        }
        let end = cpu.esp.checked_add(length + 4).ok_or(-1)?;
        let mut stack = [0u8; 36];
        ram.read_bytes(cpu.esp, &mut stack[..length as usize + 4])?;
        let result = match ordinal {
            187 => {
                self.files
                    .rom_close(ram, table, rom_kernel_word(&stack, 4)?)?;
                0
            }
            202 => self.rom_kernel_open(image, table, ram, &stack)?,
            219 => self.rom_kernel_read(table, ram, &stack)?,
            _ => return Err(-5),
        };
        ram.kernel_set_caller(rom_kernel_word(&stack, 0)?);
        cpu.eax = result;
        cpu.esp = end;
        Ok(())
    }

    fn rom_kernel_open(
        &mut self,
        image: &disc::Disc,
        table: &kernel_pool::Handles,
        ram: &mut memory::Memory,
        stack: &[u8],
    ) -> Result<u32, i32> {
        let output_va = rom_kernel_word(stack, 4)?;
        let access = rom_kernel_word(stack, 8)?;
        let attributes = rom_kernel_word(stack, 12)?;
        let status_va = rom_kernel_word(stack, 16)?;
        let share = rom_kernel_word(stack, 20)?;
        let options = rom_kernel_word(stack, 24)?;
        // Read-only rights and shared readers. Adopt explicit synchronous
        // opens, with non-directory, sequential and no-buffering hints.
        if access & !0xa01200a9 != 0
            || share != 1
            || options & !0x7c != 0
            || (options & 0x30 != 0x10 && options & 0x30 != 0x20)
        {
            return Err(-5);
        }
        if output_va == 0 || status_va == 0 || attributes == 0 {
            return Err(-1);
        }
        let first = u64::from(output_va);
        let last = u64::from(status_va);
        if first < last + 8 && last < first + 4 {
            return Err(-1);
        }
        let raw = rom_kernel_name(ram, attributes)?;
        let path = rom_kernel_path(gamepkg::text_from_utf8(&raw)?)?;
        ram.memory_check_write(output_va, 4)?;
        ram.memory_check_write(status_va, 8)?;
        if image.disc_find(path) == Err(-5) {
            ram.write_u64(status_va, 0xc0000034)?;
            return Ok(0xc0000034);
        }
        let opened = self
            .files
            .rom_open(ram, table, image, path, &mut self.orphan);
        if let Err(code) = opened {
            self.rom_kernel_settle(ram, table)?;
            return Err(code);
        }
        let io_block = u64::to_le_bytes(0x100000000);
        let written = ram.memory_write_pair(
            output_va,
            &u32::to_le_bytes(self.orphan),
            status_va,
            &io_block,
        );
        if let Err(code) = written {
            self.rom_kernel_settle(ram, table)?;
            return Err(code);
        }
        self.orphan = 0;
        Ok(0)
    }

    fn rom_kernel_read(
        &mut self,
        table: &kernel_pool::Handles,
        ram: &mut memory::Memory,
        stack: &[u8],
    ) -> Result<u32, i32> {
        if rom_kernel_word(stack, 8)? != 0
            || rom_kernel_word(stack, 12)? != 0
            || rom_kernel_word(stack, 16)? != 0
        {
            return Err(-5);
        }
        let token = rom_kernel_word(stack, 4)?;
        let status_va = rom_kernel_word(stack, 20)?;
        let output_va = rom_kernel_word(stack, 24)?;
        let length = rom_kernel_word(stack, 28)?;
        let offset_va = rom_kernel_word(stack, 32)?;
        let offset = if offset_va == 0 {
            None
        } else {
            let signed = ram.read_i64(offset_va)?;
            if signed < 0 {
                None
            } else {
                Some(signed as u64)
            }
        };
        self.publish_cursor = false;
        let reserved = self
            .files
            .rom_read_begin(ram, table, token, &mut self.reading);
        let result = match reserved {
            Ok(()) => self
                .reading
                .as_mut()
                .ok_or(-3)?
                .rom_read_guest(ram, offset, output_va, length, status_va),
            Err(code) => Err(code),
        };
        self.publish_cursor = result.is_ok();
        self.rom_kernel_settle(ram, table)?;
        result
    }

    // Teardown retries retirement, never the previous disk or guest write.
    pub fn rom_kernel_settle(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.rom_kernel_finish_read(ram, table)?;
        if self.orphan != 0 {
            if self.orphan_attempted {
                return Err(-3);
            }
            self.orphan_attempted = true;
            self.files.rom_close(ram, table, self.orphan)?;
            self.orphan = 0;
            self.orphan_attempted = false;
        }
        Ok(())
    }

    fn rom_kernel_finish_read(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.files.rom_cleanup(ram, table)?;
        if let Some(active) = self.reading.as_mut() {
            active.rom_read_finish(ram, self.publish_cursor)?;
            self.reading = None;
        }
        Ok(())
    }

    pub fn rom_kernel_drain(
        &mut self,
        ram: &memory::Memory,
        table: &kernel_pool::Handles,
    ) -> Result<(), i32> {
        self.rom_kernel_finish_read(ram, table)?;
        // Workers have joined. Drain by owned entry, without reusing a token
        // from a close that might already have consumed its table slot.
        self.files.rom_drain(ram, table)?;
        self.orphan = 0;
        self.orphan_attempted = false;
        Ok(())
    }
}

fn rom_kernel_word(raw: &[u8], at: usize) -> Result<u32, i32> {
    let bytes = raw.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn rom_kernel_name(ram: &memory::Memory, attributes: u32) -> Result<std::vec::Vec<u8>, i32> {
    let mut fields = [0u8; 12];
    ram.read_bytes(attributes, &mut fields)?;
    let namespace = rom_kernel_word(&fields, 0)?;
    if namespace != 0 && namespace != 0xfffffffdu32 {
        return Err(-5);
    }
    let name_va = rom_kernel_word(&fields, 4)?;
    if name_va == 0 {
        return Err(-1);
    }
    let mut name = [0u8; 8];
    ram.read_bytes(name_va, &mut name)?;
    let length = usize::from(u16::from_le_bytes([name[0], name[1]]));
    let maximum = usize::from(u16::from_le_bytes([name[2], name[3]]));
    let buffer = rom_kernel_word(&name, 4)?;
    if length == 0 || length > maximum || length > 528 || buffer == 0 {
        return Err(-1);
    }
    let mut raw = std::vec::Vec::new();
    raw.resize(length, 0u8);
    ram.read_bytes(buffer, &mut raw)?;
    for byte in &raw {
        if *byte < 32 || *byte >= 127 {
            return Err(-5);
        }
    }
    Ok(raw)
}

fn rom_kernel_path(path: &str) -> Result<&str, i32> {
    for prefix in ["D:\\", "\\??\\D:\\", "\\Device\\CdRom0\\"] {
        if let Some(head) = path.get(..prefix.len()) {
            if head.eq_ignore_ascii_case(prefix) {
                return path.get(prefix.len()..).ok_or(-1);
            }
        }
    }
    Err(-5)
}
