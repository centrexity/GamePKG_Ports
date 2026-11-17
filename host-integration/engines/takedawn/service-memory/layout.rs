// Port-owned guest views. Backing, permissions and lifetime remain host-owned.
use crate::gamepkg;
use crate::memory;
pub struct Layout {
    root: u64,
    banks: [u64; 3],
    scratch: u64,
}
fn layout_long(bytes: &[u8], at: usize) -> Result<u64, i32> {
    let part = bytes.get(at..at + 8).ok_or(-1)?;
    Ok(u64::from_le_bytes([
        part[0], part[1], part[2], part[3], part[4], part[5], part[6], part[7],
    ]))
}
impl Layout {
    pub fn layout_current(ram: &memory::Memory) -> Result<Self, i32> {
        Self::layout_read(ram, false)
    }
    pub fn layout_services(ram: &memory::Memory) -> Result<Self, i32> {
        Self::layout_read(ram, true)
    }
    fn layout_read(ram: &memory::Memory, services: bool) -> Result<Self, i32> {
        ram.call_snapshot()?;
        let mut bytes = [0u8; 64];
        let expected = if services { 64usize } else { 48 };
        if gamepkg::local_call(1, &[], &mut bytes[..expected])? != expected {
            return Err(-5);
        }
        let banks = [
            layout_long(&bytes, 0)?,
            layout_long(&bytes, 16)?,
            layout_long(&bytes, 32)?,
        ];
        if layout_long(&bytes, 8)? != 67108864
            || layout_long(&bytes, 24)? != 67108864
            || layout_long(&bytes, 40)? != 1048576
            || banks[0] == 0
            || banks[1] == 0
            || banks[2] == 0
            || banks[0] == banks[1]
            || banks[0] == banks[2]
            || banks[1] == banks[2]
        {
            return Err(-1);
        }
        let scratch = if services {
            layout_long(&bytes, 48)?
        } else {
            0
        };
        if services
            && (scratch == 0 || layout_long(&bytes, 56)? != 1048576 || banks.contains(&scratch))
        {
            return Err(-1);
        }
        Ok(Self {
            root: ram.call_handle(),
            banks,
            scratch,
        })
    }
    pub fn layout_scratch(&self) -> Result<u64, i32> {
        if self.scratch == 0 {
            Err(-5)
        } else {
            Ok(self.scratch)
        }
    }
    pub fn layout_banks(&self) -> [u64; 3] {
        self.banks
    }
    // Build a fresh view, preserving opaque caller context but never copying an
    // exclusive heap client. Failure drops only this partial view, not its root.
    // This operation commits, zeros and changes permissions on no RAM pages.
    pub fn layout_view(&self, source: &memory::Memory) -> Result<memory::Memory, i32> {
        source.call_snapshot()?;
        if source.call_handle() != self.root {
            return Err(-4);
        }
        let mut view = source.call_view();
        for index in 0..29u32 {
            view.attach(index * 67108864, 67108864, self.banks[0], 0, true)?;
        }
        view.attach(0x80000000, 67108864, self.banks[1], 0, true)?;
        view.attach(0xf0000000, 67108864, self.banks[1], 0, true)?;
        view.attach(0xff000000, 1048576, self.banks[2], 0, true)?;
        Ok(view)
    }
}
