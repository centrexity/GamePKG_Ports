// Port-owned CPU startup data. No host thread, register, heap or device API.
use crate::gamepkg;
use crate::memory;
use crate::registers;
use crate::xbe;
pub struct Bootstrap {
    record: u64,
    region: u32,
    stack_base: u32,
    stack_bytes: u32,
    stack_top: u32,
    heap_base: u32,
    tls_template: u32,
    tls_total: u32,
    thread_data: u32,
    initial: std::vec::Vec<u8>,
}
fn boot_word(bytes: &[u8], at: u32) -> Result<u32, i32> {
    let end = at.checked_add(4).ok_or(-1)?;
    if end as usize > bytes.len() {
        return Err(-1);
    }
    let start = at as usize;
    Ok(u32::from_le_bytes([
        bytes[start],
        bytes[start + 1],
        bytes[start + 2],
        bytes[start + 3],
    ]))
}
fn boot_range(start: u32, bytes: u32, low: u32, high: u32) -> Result<(), i32> {
    if start < low || start > high || bytes > high - start {
        return Err(-1);
    }
    Ok(())
}
impl Bootstrap {
    pub fn boot_plan(image: &xbe::Image, ram: &memory::Memory) -> Result<Self, i32> {
        let _ = ram.call_snapshot()?;
        let (_, record) = gamepkg::local_record_snapshot(ram.call_handle(), 288)?;
        let bounds = image.image_bounds();
        if boot_word(&record, 0)? != bounds[2] || boot_word(&record, 4)? != bounds[3] {
            return Err(-1);
        }
        let region = (bounds[1].checked_add(65535).ok_or(-1)? & !65535u32).max(0x700000);
        let stack_base = region.checked_add(0x80000).ok_or(-1)?;
        let available = 0x4000000u32.checked_sub(stack_base).ok_or(-1)?;
        if available < 2 * 1048576 {
            return Err(-1);
        }
        let mut stack_bytes = 8 * 1048576u32;
        // Preserve the original u32 decision, then reject an impossible layout
        // rather than publishing an out-of-window stack after subtraction wrap.
        if available.wrapping_sub(stack_bytes) < 16 * 1048576 {
            stack_bytes = if available > 17 * 1048576 {
                (available - 16 * 1048576) & !65535u32
            } else {
                1048576
            };
            stack_bytes = stack_bytes.max(1048576);
        }
        let heap_base = stack_base.checked_add(stack_bytes).ok_or(-1)?;
        if heap_base > 0x4000000 - 1048576 {
            return Err(-1);
        }
        let stack_top = heap_base - 16;
        let mut initial = std::vec::Vec::new();
        let mut tls_template = 0u32;
        let mut tls_total = 0u32;
        let mut thread_data = 0u32;
        let directory = image.image_tls();
        if directory != 0 {
            boot_range(directory, 24, bounds[0], bounds[1])?;
            let data_start = ram.read_u32(directory)?;
            let data_end = ram.read_u32(directory + 4)?;
            let zero_fill = ram.read_u32(directory + 16)?;
            let init_bytes = data_end.checked_sub(data_start).ok_or(-1)?;
            let rounded = init_bytes
                .checked_add(zero_fill)
                .ok_or(-1)?
                .checked_add(15)
                .ok_or(-1)?
                & !15u32;
            tls_total = rounded.checked_add(4).ok_or(-1)?;
            if tls_total > 65536 {
                return Err(-1);
            }
            tls_template = region.checked_add(0x70000).ok_or(-1)?;
            boot_range(tls_template, tls_total, region, stack_base)?;
            thread_data = tls_template.checked_add(0x200).ok_or(-1)?;
            boot_range(thread_data, 64, region, stack_base)?;
            if init_bytes != 0 {
                boot_range(data_start, init_bytes, bounds[0], bounds[1])?;
                initial.resize(init_bytes as usize, 0u8);
                ram.read_bytes(data_start, &mut initial)?;
            }
        }
        Ok(Self {
            record: ram.call_handle(),
            region,
            stack_base,
            stack_bytes,
            stack_top,
            heap_base,
            tls_template,
            tls_total,
            thread_data,
            initial,
        })
    }
    pub fn boot_layout(&self) -> [u32; 14] {
        [
            self.region,
            self.stack_base,
            self.stack_bytes,
            self.stack_top,
            self.heap_base,
            0x4000000,
            0x4000,
            self.tls_template,
            self.tls_total,
            self.thread_data,
            64,
            self.region + 0x61000,
            self.region + 0x60000,
            self.region + 0x40000,
        ]
    }
    pub fn boot_apply(&self, ram: &mut memory::Memory) -> Result<registers::Registers, i32> {
        if ram.call_handle() != self.record {
            return Err(-4);
        }
        let _ = ram.call_snapshot()?;
        ram.write_u32(0x4000, 0xffffffff)?;
        ram.write_u32(0x4004, self.stack_top)?;
        ram.write_u32(0x4008, self.stack_base)?;
        ram.write_u32(0x4018, 0x4000)?;
        let prcb = self.region + 0x61000;
        ram.fill(prcb, 0, 0x400)?;
        ram.write_u32(0x4020, prcb)?;
        let fake_tls = self.region + 0x60000;
        ram.write_u32(0x4028, fake_tls)?;
        ram.write_u32(fake_tls + 0x28, self.region)?;
        if self.tls_total != 0 {
            ram.fill(self.tls_template, 0, self.tls_total)?;
            ram.fill(self.thread_data, 0, 64)?;
            if !self.initial.is_empty() {
                ram.write_bytes(self.tls_template, &self.initial)?;
            }
            ram.write_u32(self.tls_template, self.thread_data)?;
            ram.write_u32(0x4004, self.tls_template + self.tls_total)?;
        }
        let mut context = registers::Registers::new();
        context.fs_base = 0x4000;
        context.esp = self.stack_top;
        Ok(context)
    }
}
