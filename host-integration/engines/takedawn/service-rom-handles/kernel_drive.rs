// DOS drive aliases are guest namespace entries. Their targets are the same
// disc and private save roots already granted to the checked startup.
use crate::gamepkg;
use crate::kernel_file;
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

pub struct DriveLinks {
    slots: [u8; 26],
}

impl DriveLinks {
    pub fn drive_links_new() -> Self {
        Self { slots: [0; 26] }
    }

    pub fn drive_links_call(
        &mut self,
        ram: &mut memory::Memory,
        cpu: &mut registers::Registers,
        ordinal: u32,
    ) -> Result<(), i32> {
        if gamepkg::local_task_status()?.1 {
            return Err(-4);
        }
        let frame_length = if ordinal == 67 { 12 } else { 8 };
        let next_stack = cpu.esp.checked_add(frame_length).ok_or(-1)?;
        let mut frame = [0u8; 12];
        ram.read_bytes(cpu.esp, &mut frame[..frame_length as usize])?;
        let symbol =
            kernel_file::rom_kernel_counted_name(ram, kernel_file::rom_kernel_word(&frame, 4)?)?;
        let name = drive_links_dos_name(&symbol);
        let slot = drive_letter(name)?;
        if !symbol.starts_with(b"\\??\\") || name.len() != 2 {
            return Err(-5);
        }
        let status = if ordinal == 67 {
            let target = kernel_file::rom_kernel_counted_name(
                ram,
                kernel_file::rom_kernel_word(&frame, 8)?,
            )?;
            let mut root_id = 0;
            for (index, allowed) in DRIVE_ROOTS.iter().enumerate() {
                if target.eq_ignore_ascii_case(allowed.as_bytes()) {
                    root_id = index as u8;
                    break;
                }
            }
            if root_id == 0 {
                return Err(-5);
            }
            if self.slots[slot] == 0 {
                self.slots[slot] = root_id;
                0
            } else {
                0xc0000035
            }
        } else if self.slots[slot] == 0 {
            0xc0000034
        } else {
            self.slots[slot] = 0;
            0
        };
        ram.kernel_set_caller(kernel_file::rom_kernel_word(&frame, 0)?);
        cpu.esp = next_stack;
        cpu.eax = status;
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
        let root_id = self.slots[slot];
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

fn drive_letter(name: &[u8]) -> Result<usize, i32> {
    if let Some(&letter) = name.first() {
        let upper = letter.to_ascii_uppercase();
        if upper.is_ascii_uppercase() && name.get(1) == Some(&b':') {
            return Ok(usize::from(upper - b'A'));
        }
    }
    Err(-5)
}
