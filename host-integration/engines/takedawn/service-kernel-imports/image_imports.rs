// Production coordinator adapter: derive imports from the authenticated image.
use crate::gamepkg;
use crate::imports;
use crate::kernel_data;
use crate::memory;
use crate::xbe;
fn image_import_word(bytes: &[u8], at: usize) -> Result<u32, i32> {
    let encoded_word = bytes.get(at..at + 4).ok_or(-1)?;
    Ok(u32::from_le_bytes([
        encoded_word[0],
        encoded_word[1],
        encoded_word[2],
        encoded_word[3],
    ]))
}
pub fn imports_image(
    ram: &memory::Memory,
    image: &xbe::Image,
    data: &kernel_data::Data,
    remap: &[u16],
) -> Result<imports::Imports, i32> {
    ram.call_snapshot()?;
    let (_, record) = gamepkg::local_record_snapshot(ram.call_handle(), 280)?;
    let bounds = image.image_bounds();
    if image_import_word(&record, 0)? != bounds[2] || image_import_word(&record, 4)? != bounds[3] {
        return Err(-1);
    }
    let region = (bounds[1].checked_add(65535).ok_or(-1)? & !65535u32).max(0x700000);
    if data.data_base(ram)? != region.checked_add(0x40000).ok_or(-1)? {
        return Err(-1);
    }
    let plan = imports::Imports::imports_scan(ram, image.image_thunk(), data, remap)?;
    let shape = plan.imports_layout();
    if shape[0] < bounds[0] || u64::from(shape[0]) + u64::from(shape[1]) * 4 > u64::from(bounds[1])
    {
        return Err(-1);
    }
    Ok(plan)
}
