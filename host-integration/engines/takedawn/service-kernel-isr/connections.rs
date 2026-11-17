// Original guest interrupt-vector registry, through copied GamePKG records.
use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
pub struct Connections {
    root: u64,
    services: u64,
    record: u64,
    creator: u64,
    owned: bool,
}
fn connection_word(b: &[u8], at: usize) -> Result<u64, i32> {
    let s = b.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7],
    ]))
}
fn connection_live(ram: &memory::Memory, services: u64) -> Result<(), i32> {
    if services == 0 || ram.services_handle()? != services {
        return Err(-4);
    }
    let values = kernel_pool::kernel_service_values(ram);
    if ram.services_handle()? != services || gamepkg::local_task_status()?.1 {
        return Err(-4);
    }
    values?;
    Ok(())
}
impl Connections {
    pub fn connections_new(ram: &memory::Memory) -> Result<Self, i32> {
        let services = ram.services_handle()?;
        connection_live(ram, services)?;
        let creator = gamepkg::local_task_status()?.0;
        let mut b = [0u8; 192];
        for (at, value) in [
            (0usize, 0x31525349u64),
            (8, ram.call_handle()),
            (16, services),
        ] {
            let word = u64::to_le_bytes(value);
            for lane in 0..8usize {
                b[at + lane] = word[lane];
            }
        }
        let record = gamepkg::local_record_create(&b)?;
        if let Err(code) = connection_live(ram, services) {
            if gamepkg::local_record_release(record).is_err() {
                return Err(-3);
            }
            return Err(code);
        }
        Ok(Self {
            root: ram.call_handle(),
            services,
            record,
            creator,
            owned: true,
        })
    }
    pub fn connections_id(&self) -> u64 {
        self.record
    }
    pub fn connections_closed(&self) -> bool {
        self.record == 0
    }
    pub fn connections_borrow(ram: &memory::Memory, record: u64) -> Result<Self, i32> {
        let view = Self {
            root: ram.call_handle(),
            services: ram.services_handle()?,
            record,
            creator: 0,
            owned: false,
        };
        view.connection_snapshot(ram)?;
        Ok(view)
    }
    fn connection_snapshot(&self, ram: &memory::Memory) -> Result<(u64, std::vec::Vec<u8>), i32> {
        if self.record == 0 || ram.call_handle() != self.root {
            return Err(-4);
        }
        connection_live(ram, self.services)?;
        let (version, b) = gamepkg::local_record_snapshot(self.record, 192)?;
        if b.len() != 192 || connection_word(&b, 0)? != 0x31525349 {
            return Err(-1);
        }
        if connection_word(&b, 8)? != self.root || connection_word(&b, 16)? != self.services {
            return Err(-4);
        }
        for at in [24usize, 32, 40, 48, 56] {
            if connection_word(&b, at)? != 0 {
                return Err(-1);
            }
        }
        connection_live(ram, self.services)?;
        Ok((version, b))
    }
    pub fn connection_get(&self, ram: &memory::Memory, vector: u32) -> Result<u32, i32> {
        let (_, b) = self.connection_snapshot(ram)?;
        if vector >= 32 {
            return Ok(0);
        }
        let at = 64 + vector as usize * 4;
        Ok(u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]))
    }
    // The original reports connected even for null/out-of-range requests.
    // [return value, vector or u32::MAX, routine, context] retains its log data.
    pub fn connection_connect(&self, ram: &memory::Memory, guest: u32) -> Result<[u32; 4], i32> {
        self.connection_snapshot(ram)?;
        if guest == 0 {
            return Ok([1, u32::MAX, 0, 0]);
        }
        let vector = ram.read_u32(guest.checked_add(8).ok_or(-1)?)?;
        if vector >= 32 {
            return Ok([1, vector, 0, 0]);
        }
        let at = 64 + vector as usize * 4;
        let encoded = u32::to_le_bytes(guest);
        for _ in 0..64u32 {
            let (version, mut b) = self.connection_snapshot(ram)?;
            for lane in 0..4usize {
                b[at + lane] = encoded[lane];
            }
            match gamepkg::local_record_compare_exchange(self.record, version, &b) {
                Ok(_) => {
                    // Publication preceded these original diagnostic reads.
                    let routine = ram.read_u32(guest)?;
                    let context = ram.read_u32(guest.checked_add(4).ok_or(-1)?)?;
                    connection_live(ram, self.services)?;
                    return Ok([1, vector, routine, context]);
                }
                Err(-2) => memory::Memory::call_retry_pause()?,
                Err(code) => return Err(code),
            }
        }
        Err(-2)
    }
    pub fn connections_close(&mut self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root || gamepkg::local_task_status()?.0 != self.creator {
            return Err(-4);
        }
        if self.record == 0 {
            return Ok(());
        }
        if !self.owned {
            return Err(-4);
        }
        let validation = connection_live(ram, self.services);
        gamepkg::local_record_release(self.record)?;
        self.record = 0;
        self.owned = false;
        validation
    }
}
// The bridge writes exactly 44 guest bytes, not a host-shaped KINTERRUPT.
pub fn interrupt_initialize(
    ram: &mut memory::Memory,
    guest: u32,
    routine: u32,
    context: u32,
    vector: u32,
) -> Result<bool, i32> {
    ram.call_snapshot()?;
    if guest == 0 {
        return Ok(false);
    }
    guest.checked_add(44).ok_or(-1)?;
    ram.fill(guest, 0, 44)?;
    ram.write_u32(guest, routine)?;
    ram.write_u32(guest + 4, context)?;
    ram.write_u32(guest + 8, vector)?;
    Ok(true)
}
pub fn interrupt_vector(ram: &mut memory::Memory, level: u32, irql_guest: u32) -> Result<u32, i32> {
    ram.call_snapshot()?;
    if irql_guest != 0 {
        ram.write_u8(irql_guest, level as u8)?;
    }
    Ok(level)
}
