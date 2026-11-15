// Synthetic inputs only; no game image, saved keys or private assets.
use crate::memory;
pub struct Case {
    pub region: u32,
    pub tick: u32,
    pub video: u32,
    pub top: u32,
    pub magic: u32,
    pub certificate: u32,
    pub cert_key: [u8; 16],
    pub hd: Option<[u8; 16]>,
    pub command: std::vec::Vec<u8>,
}
impl Case {
    pub fn data_case(mode: u32) -> Result<Self, i32> {
        if mode >= 24 {
            return Err(-1);
        }
        let region = [0x700000u32, 0x780000, 0x1000000, 0x3e00000][mode as usize % 4];
        let tick = [0u32, 1, u32::MAX, 0x80000000][mode as usize % 4];
        let length = [0usize, 1, 3071, 3072, 65536, 1, 256, 255][mode as usize % 8];
        let mut command = std::vec::Vec::new();
        command.resize(length, 0u8);
        for index in 0..length {
            command[index] = 65 + (index % 26) as u8;
        }
        if mode % 8 == 5 {
            command[0] = 0;
        }
        if mode % 8 == 6 {
            command[17] = 0;
        }
        let certificate = [0x12000u32, 0x12000, 0, 0xffff, 0x7fffff, 0x800000][mode as usize % 6];
        let mut cert_key = [0u8; 16];
        let mut hd_key = [0u8; 16];
        for index in 0..16usize {
            cert_key[index] = (mode as u8).wrapping_mul(11).wrapping_add(index as u8);
            hd_key[index] = 0xa5 ^ (index as u8).wrapping_add(mode as u8);
        }
        Ok(Self {
            region,
            tick,
            video: mode.wrapping_mul(0x1020304),
            top: if mode == 23 {
                0x1000000 + 7167
            } else {
                67108864
            },
            magic: if mode % 6 == 1 { 0 } else { 0x48454258 },
            certificate: if mode >= 12 && mode % 6 == 5 {
                u32::MAX
            } else {
                certificate
            },
            cert_key,
            hd: if mode % 5 == 0 { None } else { Some(hd_key) },
            command,
        })
    }
    pub fn data_seed(&self, ram: &mut memory::Memory) -> Result<(), i32> {
        ram.write_u32(0x10000, self.magic)?;
        ram.write_u32(0x10118, self.certificate)?;
        if self.certificate >= 0x10000 && self.certificate < 0x800000 {
            ram.write_bytes(self.certificate + 0xc0, &self.cert_key)?;
        }
        Ok(())
    }
}
