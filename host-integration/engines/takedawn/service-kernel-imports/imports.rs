// Port-owned complete import planning. No callbacks, pointers or OS calls.
use crate::gamepkg;
use crate::kernel_data;
use crate::kernel_pool;
use crate::memory;
pub struct Imports {
    root: u64,
    table: u32,
    count: u32,
    slots: u32,
    low: u32,
    original: std::vec::Vec<u8>,
    patched: std::vec::Vec<u8>,
    record: std::vec::Vec<u8>,
    pages: std::vec::Vec<[u64; 4]>,
    physical: bool,
    services: bool,
}
fn imports_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let part = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([part[0], part[1], part[2], part[3]]))
}
fn imports_put(bytes: &mut [u8], at: usize, value: u32) {
    let encoded_word = u32::to_le_bytes(value);
    for index in 0..4usize {
        bytes[at + index] = encoded_word[index];
    }
}
fn imports_page_mode(page: [u64; 4], mode: u32) -> Result<(), i32> {
    let mut request = std::vec::Vec::new();
    request.extend(u64::to_le_bytes(page[1]));
    request.extend(u64::to_le_bytes(page[2]));
    request.extend(u64::to_le_bytes(4096));
    request.extend(u32::to_le_bytes(mode));
    if gamepkg::local_call(3, &request, &mut [])? != 0 {
        return Err(-3);
    }
    Ok(())
}
impl Imports {
    pub fn imports_scan(
        ram: &memory::Memory,
        table: u32,
        data: &kernel_data::Data,
        remap: &[u16],
    ) -> Result<Self, i32> {
        ram.call_snapshot()?;
        if ram.kernel_handle() != 0 || table < 65536 || remap.len() > 65536 {
            return Err(-1);
        }
        let base = data.data_base(ram)?;
        let mut counted = 0u32;
        // Require the complete null-terminated run, including the 378-slot edge.
        loop {
            let address = u64::from(table) + u64::from(counted) * 4;
            if address + 4 > 67108864 {
                return Err(-1);
            }
            if ram.read_u32(address as u32)? == 0 {
                break;
            }
            if counted == 378 {
                return Err(-1);
            }
            counted += 1;
        }
        // The original setter keeps its initial default when passed zero.
        let count = if counted == 0 { 378 } else { counted };
        let length = 64 + count as usize * 4 + if counted == 0 { 0 } else { 4 };
        let low = table - 64;
        let end = u64::from(low) + length as u64;
        if end > 67108864 || (u64::from(low) < u64::from(base) + 4096 && end > u64::from(base)) {
            return Err(-1);
        }
        let mut original = std::vec::Vec::new();
        original.resize(length, 0u8);
        ram.read_bytes(low, &mut original)?;
        if counted == 0 {
            if imports_word(&original, 64)? != 0 {
                return Err(-2);
            }
        } else {
            for slot in 0..count as usize {
                if imports_word(&original, 64 + slot * 4)? == 0 {
                    return Err(-2);
                }
            }
            if imports_word(&original, 64 + count as usize * 4)? != 0 {
                return Err(-2);
            }
        }
        let mut patched = std::vec::Vec::new();
        patched.extend(&original);
        let mut ordinals = [0u32; 378];
        let mut physical = false;
        let mut services = false;
        for slot in 0..count as usize {
            let current = imports_word(&original, 64 + slot * 4)?;
            if current & 0x80000000 == 0 {
                continue;
            }
            let raw = current & 0x7fffffff;
            let ordinal = if (raw as u64) < remap.len() as u64 && remap[raw as usize] != 0 {
                u32::from(remap[raw as usize])
            } else {
                raw
            };
            ordinals[slot] = ordinal;
            let address = data.data_address(ram, ordinal)?;
            if address != 0 {
                imports_put(&mut patched, 64 + slot * 4, address);
                continue;
            }
            kernel_pool::kernel_supported(ordinal)?;
            physical |= kernel_pool::kernel_physical(ordinal);
            services |= kernel_pool::kernel_requires_services(ordinal);
            imports_put(&mut patched, 64 + slot * 4, 0xfe000000 + slot as u32 * 4);
        }
        let mut slots = count;
        for distance in 1..=16u32 {
            let at = 64 - distance as usize * 4;
            let current = imports_word(&original, at)?;
            if current & 0x80000000 == 0 {
                continue;
            }
            let slot = count + distance;
            if slot >= 378 {
                return Err(-1);
            }
            // Original lookbehind deliberately has neither remap nor data routing.
            let ordinal = current & 0x7fffffff;
            kernel_pool::kernel_supported(ordinal)?;
            physical |= kernel_pool::kernel_physical(ordinal);
            services |= kernel_pool::kernel_requires_services(ordinal);
            ordinals[slot as usize] = ordinal;
            slots = slots.max(slot + 1);
            imports_put(&mut patched, at, 0xfe000000 + slot * 4);
        }
        let mut record = std::vec::Vec::new();
        record.resize(28 + slots as usize * 4, 0u8);
        imports_put(&mut record, 0, 0x334c504b);
        imports_put(&mut record, 4, slots);
        let root = u64::to_le_bytes(ram.call_handle());
        for index in 0..8usize {
            record[8 + index] = root[index];
        }
        imports_put(&mut record, 16, table);
        imports_put(&mut record, 20, count);
        for slot in 0..slots as usize {
            imports_put(&mut record, 24 + slot * 4, ordinals[slot]);
        }
        let mut pages = std::vec::Vec::new();
        let mut address = low & !4095u32;
        while u64::from(address) < end {
            let backing = ram.memory_page_backing(address)?;
            let states = gamepkg::local_memory_page_states(backing[0], backing[1], 1)?;
            if states[0] < 2 {
                return Err(-4);
            }
            pages.extend([[
                u64::from(address),
                backing[0],
                backing[1],
                u64::from(states[0]),
            ]]);
            address += 4096;
        }
        if pages.len() > 2 {
            return Err(-1);
        }
        if services {
            kernel_pool::kernel_service_values(ram)?;
        }
        Ok(Self {
            root: ram.call_handle(),
            table,
            count,
            slots,
            low,
            original,
            patched,
            record,
            pages,
            physical,
            services,
        })
    }
    pub fn imports_layout(&self) -> [u32; 3] {
        [self.table, self.count, self.slots]
    }
    fn imports_validate(&self, ram: &memory::Memory) -> Result<(), i32> {
        ram.call_snapshot()?;
        if ram.call_handle() != self.root {
            return Err(-4);
        }
        if ram.kernel_handle() != 0 {
            return Err(-1);
        }
        for page in &self.pages {
            let backing = ram.memory_page_backing(page[0] as u32)?;
            if backing != [page[1], page[2]] {
                return Err(-4);
            }
            let states = gamepkg::local_memory_page_states(backing[0], backing[1], 1)?;
            if u64::from(states[0]) != page[3] {
                return Err(-2);
            }
        }
        let mut current = std::vec::Vec::new();
        current.resize(self.original.len(), 0u8);
        ram.read_bytes(self.low, &mut current)?;
        if current != self.original {
            return Err(-2);
        }
        Ok(())
    }
    fn imports_patch(&self, ram: &mut memory::Memory) -> Result<(), i32> {
        let mut changed = [false; 2];
        let mut result = Ok(());
        for index in 0..self.pages.len() {
            if self.pages[index][3] == 2 {
                match imports_page_mode(self.pages[index], 2) {
                    Ok(()) => changed[index] = true,
                    Err(code) => {
                        result = Err(code);
                        break;
                    }
                }
            }
        }
        if result.is_ok() {
            result = ram.write_bytes(self.low, &self.patched);
        }
        let mut restore = None;
        for index in (0..self.pages.len()).rev() {
            if changed[index] {
                if let Err(code) = imports_page_mode(self.pages[index], 1) {
                    restore = Some(code);
                }
            }
        }
        if let Some(code) = restore {
            return Err(code);
        }
        result
    }
    pub fn imports_publish(&self, ram: &mut memory::Memory) -> Result<(), i32> {
        self.imports_validate(ram)?;
        if self.services {
            kernel_pool::kernel_service_values(ram)?;
        }
        if self.physical {
            let mut client = ram.heap_take()?;
            let ready = client.heap_contiguous_counter(0);
            ram.heap_put(client);
            ready?;
        }
        let id = gamepkg::local_record_create(&self.record)?;
        if let Err(code) = self.imports_patch(ram) {
            let _ = gamepkg::local_record_release(id);
            return Err(code);
        }
        if let Err(code) = ram.kernel_install(id) {
            let _ = gamepkg::local_record_release(id);
            return Err(code);
        }
        Ok(())
    }
}
