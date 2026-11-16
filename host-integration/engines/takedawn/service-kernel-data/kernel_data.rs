// Port-owned kernel data initialization over checked guest RAM and one owner.
use crate::gamepkg;
use crate::memory;
use crate::signature;
pub struct Data {
    root: u64,
    base: u32,
    keys: [u8; 32],
    launch: u32,
}
fn exports_put32(page: &mut [u8], at: usize, value: u32) {
    let bytes = u32::to_le_bytes(value);
    for index in 0..4usize {
        page[at + index] = bytes[index];
    }
}
fn exports_put16(page: &mut [u8], at: usize, value: u16) {
    let bytes = u16::to_le_bytes(value);
    page[at] = bytes[0];
    page[at + 1] = bytes[1];
}
fn exports_launch(ram: &mut memory::Memory, command: &[u8]) -> Result<u32, i32> {
    let mut length = 0usize;
    while length < command.len() && command[length] != 0 {
        length += 1;
    }
    if length == 0 {
        return Ok(0);
    }
    let mut client = ram.heap_take()?;
    let allocated = client.heap_alloc(7168, 4096);
    ram.heap_put(client);
    let address = allocated?;
    if address == 0 {
        return Ok(0);
    }
    let mut page = std::vec::Vec::new();
    page.resize(4096, 0u8);
    exports_put32(&mut page, 0, 3);
    exports_put32(&mut page, 4, 0x45410091);
    let length = length.min(3071);
    for index in 0..length {
        page[1024 + index] = command[index];
    }
    ram.write_bytes(address, &page)?;
    Ok(address)
}
fn exports_keys(ram: &memory::Memory, hd: Option<[u8; 16]>) -> Result<[u8; 32], i32> {
    let mut keys = [0u8; 32];
    if let Some(key) = hd {
        let mut certificate = 0u32;
        if ram.read_u32(0x10000)? == 0x48454258 {
            certificate = ram.read_u32(0x10118)?;
        }
        if certificate >= 0x10000 && certificate < 0x800000 {
            let mut cert_key = [0u8; 16];
            ram.read_bytes(certificate + 0xc0, &mut cert_key)?;
            let derived = signature::console_signature(&cert_key);
            for index in 0..16usize {
                keys[index] = key[index];
                keys[16 + index] = derived[index];
            }
        }
    }
    Ok(keys)
}
impl Data {
    pub fn data_initialize(
        ram: &mut memory::Memory,
        region: u32,
        tick: u32,
        video: u32,
        command: &[u8],
        hd: Option<[u8; 16]>,
    ) -> Result<Self, i32> {
        Self::data_initialize_sample(ram, region, Some(tick), video, command, hd)
    }
    // Production initialization samples boot uptime once, without guest skew.
    // Explicit-value initialization remains for deterministic original-C oracles.
    pub fn data_initialize_live(
        ram: &mut memory::Memory,
        region: u32,
        video: u32,
        command: &[u8],
        hd: Option<[u8; 16]>,
    ) -> Result<Self, i32> {
        Self::data_initialize_sample(ram, region, None, video, command, hd)
    }
    fn data_initialize_sample(
        ram: &mut memory::Memory,
        region: u32,
        tick: Option<u32>,
        video: u32,
        command: &[u8],
        hd: Option<[u8; 16]>,
    ) -> Result<Self, i32> {
        ram.call_snapshot()?;
        if region < 0x700000
            || region & 65535 != 0
            || u64::from(region) + 0x41000 > 67108864
            || command.len() > 65536
        {
            return Err(-1);
        }
        let base = region + 0x40000;
        let mut page = std::vec::Vec::new();
        page.resize(4096, 0u8);
        ram.read_bytes(base, &mut page)?;
        let launch = exports_launch(ram, command)?;
        let keys = exports_keys(ram, hd)?;
        exports_put32(&mut page, 0, 0);
        page[4] = 0xa1;
        page[5] = 0xb1;
        exports_put16(&mut page, 0x10, 1);
        exports_put16(&mut page, 0x12, 0);
        exports_put16(&mut page, 0x14, 5849);
        exports_put16(&mut page, 0x16, 0);
        let tick = match tick {
            Some(value) => value,
            None => gamepkg::local_performance_sample()?.uptime_ms as u32,
        };
        exports_put32(&mut page, 0x20, tick);
        exports_put32(&mut page, 0x30, launch);
        for offset in [0x40usize, 0x50, 0x90, 0xa0, 0xb0, 0xc0] {
            exports_put32(&mut page, offset, base + offset as u32);
        }
        exports_put32(&mut page, 0xd0, 0x2710);
        exports_put32(&mut page, 0xe0, video);
        for byte in 0x500..0x700usize {
            page[byte] = 0;
        }
        exports_put32(&mut page, 0x528, base + 0x528);
        exports_put32(&mut page, 0x52c, base + 0x528);
        exports_put32(&mut page, 0x4a0, 3);
        exports_put32(&mut page, 0x70, 0);
        exports_put32(&mut page, 0x80, 0);
        for index in 0..32usize {
            page[0x100 + index] = keys[index];
        }
        for index in 0x120..0x230usize {
            page[index] = 0;
        }
        for index in 0x300..0x41cusize {
            page[index] = 0;
        }
        for (structure, buffer, text) in [
            (0x420usize, 0x430usize, b"XBOXRECOMP VIRTUAL HDD".as_slice()),
            (0x460, 0x470, b"XR0000000000".as_slice()),
            (0x60, 0x4b0, b"\\Device\\CdRom0\\default.xbe".as_slice()),
        ] {
            for index in 0..text.len() {
                page[buffer + index] = text[index];
            }
            page[buffer + text.len()] = 0;
            exports_put16(&mut page, structure, text.len() as u16);
            exports_put16(&mut page, structure + 2, text.len() as u16 + 1);
            exports_put32(&mut page, structure + 4, base + buffer as u32);
        }
        ram.write_bytes(base, &page)?;
        Ok(Self {
            root: ram.call_handle(),
            base,
            keys,
            launch,
        })
    }
    fn data_live(&self, ram: &memory::Memory) -> Result<(), i32> {
        if ram.call_handle() != self.root {
            return Err(-4);
        }
        ram.call_snapshot()?;
        Ok(())
    }
    pub fn data_base(&self, ram: &memory::Memory) -> Result<u32, i32> {
        self.data_live(ram)?;
        Ok(self.base)
    }
    pub fn data_launch(&self, ram: &memory::Memory) -> Result<u32, i32> {
        self.data_live(ram)?;
        Ok(self.launch)
    }
    pub fn data_keys(&self, ram: &memory::Memory) -> Result<[u8; 32], i32> {
        self.data_live(ram)?;
        Ok(self.keys)
    }
    pub fn data_tick(&self, ram: &mut memory::Memory, tick: u32) -> Result<(), i32> {
        self.data_live(ram)?;
        ram.write_u32(self.base + 0x20, tick)
    }
    pub fn data_tick_binding(&self, ram: &memory::Memory) -> Result<[u64; 3], i32> {
        self.data_live(ram)?;
        let backing = ram.memory_writable_word_backing(self.base + 0x20)?;
        Ok([self.root, backing[0], backing[1]])
    }
    pub fn data_address(&self, ram: &memory::Memory, ordinal: u32) -> Result<u32, i32> {
        self.data_live(ram)?;
        let offset = match ordinal {
            16 => 0x50,
            22 => 0x90,
            30 => 0xa0,
            31 => 0xb0,
            40 => 0x4a0,
            41 => 0x420,
            42 => 0x460,
            64 => 0x70,
            70 => 0x80,
            71 => 0xc0,
            156 => 0x20,
            157 => 0xd0,
            164 => 0x30,
            259 => 0x40,
            322 => 0,
            323 => 0x100,
            324 => 0x10,
            325 => 0x110,
            326 => 0x60,
            353 => 0x120,
            354 => 0x130,
            355 => 0x300,
            356 => 0xe0,
            357 => 0x500,
            _ => return Ok(0),
        };
        Ok(self.base + offset)
    }
}
