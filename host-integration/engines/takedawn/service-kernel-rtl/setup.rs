use crate::gamepkg;
use crate::kernel_pool;
use crate::memory;
use crate::rtl_probe;
pub struct Fixture {
    pub ram: memory::Memory,
    pub window: u64,
}
impl Fixture {
    pub fn fixture_new() -> Result<Self, i32> {
        let mut out = [0u8; 16];
        if gamepkg::local_call(1, &[], &mut out)? != 16 {
            return Err(-3);
        }
        let window = u64::from_le_bytes([
            out[0], out[1], out[2], out[3], out[4], out[5], out[6], out[7],
        ]);
        gamepkg::local_call(
            2,
            &[
                u64::to_le_bytes(window).as_slice(),
                &u64::to_le_bytes(0),
                &u64::to_le_bytes(262144),
                &u32::to_le_bytes(2),
            ]
            .concat(),
            &mut [],
        )?;
        let mut ram = memory::Memory::new()?;
        ram.attach(0, 262144, window, 0, true)?;
        for (index, ordinal) in [269u32, 279, 289, 290, 301].iter().enumerate() {
            ram.write_u32(0x10000 + index as u32 * 4, 0x80000000 | *ordinal)?;
        }
        kernel_pool::kernel_prepare(&mut ram, 0x10000, 5)?;
        Ok(Self { ram, window })
    }
    pub fn fixture_close(&mut self) -> Result<(), i32> {
        self.ram.kernel_clear()?;
        gamepkg::local_record_release(self.ram.call_handle())
    }
}
pub fn rtl_run() -> Result<std::vec::Vec<u8>, i32> {
    let mut fixture = Fixture::fixture_new()?;
    rtl_probe::rtl_probe_controls(&mut fixture.ram, fixture.window)?;
    let bytes = rtl_probe::rtl_probe_vectors(&mut fixture.ram)?;
    fixture.fixture_close()?;
    Ok(bytes)
}
